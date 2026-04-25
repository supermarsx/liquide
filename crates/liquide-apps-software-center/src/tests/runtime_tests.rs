//! Tests for the software center runtime.

use crate::config::SoftwareCenterConfig;
use crate::install::PackageSource;
use crate::package::{AppCategory, License, PackageInfo, Version};
use crate::runtime::SoftwareCenterRuntime;

fn make_package(id: &str, name: &str, installed: bool) -> PackageInfo {
    PackageInfo {
        id: id.into(),
        name: name.into(),
        summary: format!("{name} summary"),
        description: format!("{name} description"),
        version: Version::new(2, 0, 0),
        category: AppCategory::Productivity,
        license: License::OpenSource,
        developer: "Dev".into(),
        homepage: "https://example.com".into(),
        download_size: 1024,
        installed_size: 4096,
        screenshots: Vec::new(),
        icon: "icon".into(),
        installed,
        installed_version: if installed {
            Some(Version::new(1, 0, 0))
        } else {
            None
        },
        repository_id: "official".into(),
    }
}

#[test]
fn test_runtime_new() {
    let rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    assert_eq!(rt.catalog().total_count(), 0);
    assert_eq!(rt.repos().count(), 3);
}

#[test]
fn test_runtime_load_packages() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![
        make_package("a", "Alpha", false),
        make_package("b", "Beta", true),
    ]);
    assert_eq!(rt.catalog().total_count(), 2);
}

#[test]
fn test_runtime_install() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", false)]);
    rt.install("a").unwrap();
    assert_eq!(rt.queue().count(), 1);
}

#[test]
fn test_runtime_install_already_installed() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", true)]);
    assert!(rt.install("a").is_err());
}

#[test]
fn test_runtime_install_not_found() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    assert!(rt.install("nonexistent").is_err());
}

#[test]
fn test_runtime_remove() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", true)]);
    rt.remove("a").unwrap();
    assert_eq!(rt.queue().count(), 1);
}

#[test]
fn test_runtime_remove_not_installed() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", false)]);
    assert!(rt.remove("a").is_err());
}

#[test]
fn test_runtime_update_package() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", true)]);
    rt.update_package("a").unwrap();
    assert_eq!(rt.queue().count(), 1);
}

#[test]
fn test_runtime_config() {
    let config = SoftwareCenterConfig {
        max_concurrent_downloads: 5,
        ..SoftwareCenterConfig::default()
    };
    let rt = SoftwareCenterRuntime::new(config);
    assert_eq!(rt.config().max_concurrent_downloads, 5);
}

#[test]
fn test_runtime_install_command_uses_platform_backend() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("a", "Alpha", false)]);

    let spec = rt.install_command("a").unwrap();

    #[cfg(target_os = "windows")]
    {
        assert_eq!(rt.package_source("a").unwrap(), PackageSource::Winget);
        assert_eq!(spec.program, "winget");
    }

    #[cfg(target_os = "linux")]
    {
        assert_eq!(rt.package_source("a").unwrap(), PackageSource::Apt);
        assert_eq!(spec.program, "apt-get");
    }
}

#[test]
fn test_runtime_flatpak_repo_uses_flatpak_backend() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    let mut pkg = make_package("org.example.Flatpak", "Flatpak App", false);
    pkg.repository_id = "flatpak".into();
    rt.load_packages(vec![pkg]);

    let spec = rt.install_command("org.example.Flatpak").unwrap();

    assert_eq!(
        rt.package_source("org.example.Flatpak").unwrap(),
        PackageSource::Flatpak
    );
    assert_eq!(spec.program, "flatpak");
}

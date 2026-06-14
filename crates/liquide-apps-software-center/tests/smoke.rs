//! Per-app smoke test for the software center (t57 A7 / t57-e8).
//!
//! Builds the runtime and asserts the root model carries the default
//! repositories, and that loading a package into the catalog makes it
//! discoverable and queues an install operation — real behavior, not
//! bare construction. No network access.

use liquide_apps_software_center::config::SoftwareCenterConfig;
use liquide_apps_software_center::package::{AppCategory, License, PackageInfo, Version};
use liquide_apps_software_center::runtime::SoftwareCenterRuntime;

fn make_package(id: &str, name: &str) -> PackageInfo {
    PackageInfo {
        id: id.into(),
        name: name.into(),
        summary: format!("{name} summary"),
        description: format!("{name} description"),
        version: Version::new(1, 0, 0),
        category: AppCategory::Productivity,
        license: License::OpenSource,
        developer: "Dev".into(),
        homepage: "https://example.com".into(),
        download_size: 1024,
        installed_size: 4096,
        screenshots: Vec::new(),
        icon: "icon".into(),
        installed: false,
        installed_version: None,
        repository_id: "official".into(),
    }
}

#[test]
fn root_model_has_default_repositories() {
    let rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    let repos = rt.repos().repositories();
    assert!(
        !repos.is_empty(),
        "software center must seed default repositories, not be an empty placeholder"
    );
    assert!(
        rt.repos().find("official").is_some(),
        "default 'official' repository should be present, got {:?}",
        repos.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
}

#[test]
fn loaded_package_is_discoverable_and_queues_install() {
    let mut rt = SoftwareCenterRuntime::new(SoftwareCenterConfig::default());
    rt.load_packages(vec![make_package("com.example.app", "Example App")]);

    assert!(
        rt.catalog().find("com.example.app").is_some(),
        "loaded package should be discoverable in the catalog"
    );

    rt.install("com.example.app")
        .expect("installing a not-installed package should enqueue an operation");
    assert_eq!(
        rt.queue().count(),
        1,
        "install should enqueue exactly one operation"
    );
}

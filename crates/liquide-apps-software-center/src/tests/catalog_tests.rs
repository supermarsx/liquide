//! Tests for the catalog.

use crate::catalog::Catalog;
use crate::package::{AppCategory, License, PackageInfo, Version};

fn make_package(id: &str, name: &str, category: AppCategory, installed: bool) -> PackageInfo {
    PackageInfo {
        id: id.into(),
        name: name.into(),
        summary: format!("{name} summary"),
        description: format!("{name} description"),
        version: Version::new(1, 0, 0),
        category,
        license: License::OpenSource,
        developer: "Dev".into(),
        homepage: "https://example.com".into(),
        download_size: 1024,
        installed_size: 4096,
        screenshots: Vec::new(),
        icon: "icon".into(),
        installed,
        installed_version: None,
        repository_id: "official".into(),
    }
}

#[test]
fn test_catalog_empty() {
    let c = Catalog::new();
    assert_eq!(c.total_count(), 0);
}

#[test]
fn test_catalog_load() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Alpha", AppCategory::Productivity, false),
        make_package("b", "Beta", AppCategory::Development, true),
    ]);
    assert_eq!(c.total_count(), 2);
}

#[test]
fn test_catalog_find() {
    let mut c = Catalog::new();
    c.load(vec![make_package("a", "Alpha", AppCategory::Productivity, false)]);
    assert!(c.find("a").is_some());
    assert!(c.find("z").is_none());
}

#[test]
fn test_catalog_by_category() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Alpha", AppCategory::Productivity, false),
        make_package("b", "Beta", AppCategory::Development, false),
        make_package("c", "Gamma", AppCategory::Productivity, false),
    ]);
    let prod = c.by_category(AppCategory::Productivity);
    assert_eq!(prod.len(), 2);
}

#[test]
fn test_catalog_installed() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Alpha", AppCategory::Productivity, true),
        make_package("b", "Beta", AppCategory::Development, false),
    ]);
    assert_eq!(c.installed_count(), 1);
    assert_eq!(c.installed().len(), 1);
}

#[test]
fn test_catalog_featured() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Alpha", AppCategory::Productivity, false),
        make_package("b", "Beta", AppCategory::Development, false),
    ]);
    c.set_featured(vec!["b".into()]);
    let featured = c.featured();
    assert_eq!(featured.len(), 1);
    assert_eq!(featured[0].id, "b");
}

#[test]
fn test_catalog_search() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Firefox", AppCategory::Internet, false),
        make_package("b", "Chromium", AppCategory::Internet, false),
        make_package("c", "GIMP", AppCategory::Graphics, false),
    ]);
    let results = c.search("fire");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].package_id, "a");
}

#[test]
fn test_catalog_search_empty() {
    let c = Catalog::new();
    assert!(c.search("").is_empty());
}

#[test]
fn test_catalog_search_ranking() {
    let mut c = Catalog::new();
    c.load(vec![
        make_package("a", "Firefox Browser", AppCategory::Internet, false),
        make_package("b", "Firefox", AppCategory::Internet, false),
    ]);
    let results = c.search("Firefox");
    assert_eq!(results[0].package_id, "b"); // exact match ranks higher
}

#[test]
fn test_catalog_updatable() {
    let mut c = Catalog::new();
    let mut p = make_package("a", "Alpha", AppCategory::Productivity, true);
    p.version = Version::new(2, 0, 0);
    p.installed_version = Some(Version::new(1, 0, 0));
    c.load(vec![p]);
    assert_eq!(c.update_count(), 1);
}

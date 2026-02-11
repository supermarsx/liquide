//! Tests for package, repository, review, screenshot, and install types.

use crate::package::{AppCategory, License, PackageInfo, Version};
use crate::repository::{RepoManager, RepoType, Repository};
use crate::review::{Review, ReviewStats, ReviewStore};
use crate::screenshot::{Gallery, Screenshot};
use crate::install::{InstallAction, InstallOperation, InstallQueue, InstallState};
use crate::update::{PendingUpdate, UpdateManager};

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
        download_size: 10 * 1024 * 1024,
        installed_size: 30 * 1024 * 1024,
        screenshots: Vec::new(),
        icon: "app-icon".into(),
        installed,
        installed_version: if installed { Some(Version::new(1, 0, 0)) } else { None },
        repository_id: "official".into(),
    }
}

// ===========================================================================
// Version
// ===========================================================================

#[test]
fn test_version_parse() {
    let v = Version::parse("1.2.3").unwrap();
    assert_eq!(v, Version::new(1, 2, 3));
}

#[test]
fn test_version_parse_invalid() {
    assert!(Version::parse("1.2").is_none());
    assert!(Version::parse("abc").is_none());
}

#[test]
fn test_version_ordering() {
    assert!(Version::new(1, 0, 0) < Version::new(2, 0, 0));
    assert!(Version::new(1, 1, 0) < Version::new(1, 2, 0));
    assert!(Version::new(1, 1, 1) < Version::new(1, 1, 2));
}

#[test]
fn test_version_display() {
    assert_eq!(format!("{}", Version::new(1, 2, 3)), "1.2.3");
}

// ===========================================================================
// PackageInfo
// ===========================================================================

#[test]
fn test_package_has_update() {
    let p = make_package("a", "A", true);
    assert!(p.has_update()); // installed 1.0.0, latest 2.0.0
}

#[test]
fn test_package_no_update() {
    let mut p = make_package("a", "A", true);
    p.installed_version = Some(Version::new(2, 0, 0));
    assert!(!p.has_update());
}

#[test]
fn test_package_not_installed() {
    let p = make_package("a", "A", false);
    assert!(!p.has_update());
}

#[test]
fn test_package_human_size() {
    let p = make_package("a", "A", false);
    assert_eq!(p.human_download_size(), "10.0 MB");
}

#[test]
fn test_license_display() {
    assert_eq!(License::OpenSource.to_string(), "open-source");
    assert_eq!(License::Proprietary.to_string(), "proprietary");
}

#[test]
fn test_app_category() {
    assert_eq!(AppCategory::ALL.len(), 12);
    assert_eq!(AppCategory::Development.label(), "Development");
    assert_eq!(AppCategory::Games.to_string(), "Games");
}

// ===========================================================================
// Repository
// ===========================================================================

#[test]
fn test_repo_manager_defaults() {
    let rm = RepoManager::new();
    assert_eq!(rm.count(), 3);
    assert!(rm.find("official").is_some());
    assert!(rm.find("flatpak").is_some());
}

#[test]
fn test_repo_add() {
    let mut rm = RepoManager::new();
    let before = rm.count();
    rm.add(Repository::new("custom", "Custom", "https://custom.com", RepoType::ThirdParty));
    assert_eq!(rm.count(), before + 1);
}

#[test]
fn test_repo_no_duplicate() {
    let mut rm = RepoManager::new();
    let before = rm.count();
    rm.add(Repository::new("official", "Dupe", "url", RepoType::Official));
    assert_eq!(rm.count(), before);
}

#[test]
fn test_repo_remove() {
    let mut rm = RepoManager::new();
    rm.add(Repository::new("custom", "Custom", "url", RepoType::ThirdParty));
    rm.remove("custom").unwrap();
    assert!(rm.find("custom").is_none());
}

#[test]
fn test_repo_remove_nonexistent() {
    let mut rm = RepoManager::new();
    assert!(rm.remove("nonexistent").is_err());
}

#[test]
fn test_repo_toggle() {
    let mut rm = RepoManager::new();
    let was_enabled = rm.find("official").unwrap().enabled;
    let now_enabled = rm.toggle("official").unwrap();
    assert_ne!(was_enabled, now_enabled);
}

#[test]
fn test_repo_enabled() {
    let rm = RepoManager::new();
    let enabled = rm.enabled_repos();
    assert_eq!(enabled.len(), 3);
}

#[test]
fn test_repo_type_display() {
    assert_eq!(RepoType::Official.to_string(), "official");
    assert_eq!(RepoType::Flatpak.to_string(), "flatpak");
}

// ===========================================================================
// Review
// ===========================================================================

#[test]
fn test_review_valid() {
    let r = Review::new("Alice", 5, "Great app!", 1000).unwrap();
    assert_eq!(r.rating, 5);
}

#[test]
fn test_review_invalid_rating() {
    assert!(Review::new("Alice", 0, "Bad", 1000).is_err());
    assert!(Review::new("Alice", 6, "Bad", 1000).is_err());
}

#[test]
fn test_review_stats() {
    let reviews = vec![
        Review::new("A", 5, "", 100).unwrap(),
        Review::new("B", 3, "", 200).unwrap(),
        Review::new("C", 4, "", 300).unwrap(),
    ];
    let stats = ReviewStats::from_reviews(&reviews);
    assert_eq!(stats.total_reviews, 3);
    assert!((stats.average_rating - 4.0).abs() < 0.01);
    assert_eq!(stats.distribution[4], 1); // 5-star
    assert_eq!(stats.distribution[2], 1); // 3-star
}

#[test]
fn test_review_store() {
    let mut store = ReviewStore::new();
    store.add(Review::new("A", 5, "Great", 100).unwrap());
    store.add(Review::new("B", 3, "OK", 200).unwrap());
    assert_eq!(store.count(), 2);
    let recent = store.recent(1);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].author, "B"); // most recent
}

// ===========================================================================
// Screenshot
// ===========================================================================

#[test]
fn test_screenshot_aspect_ratio() {
    let s = Screenshot::new("url", "thumb", "Caption", 1920, 1080);
    assert!((s.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
}

#[test]
fn test_gallery_navigation() {
    let shots = vec![
        Screenshot::new("a", "a", "A", 100, 100),
        Screenshot::new("b", "b", "B", 100, 100),
        Screenshot::new("c", "c", "C", 100, 100),
    ];
    let mut g = Gallery::new(shots);
    assert_eq!(g.count(), 3);
    assert_eq!(g.current_index(), 0);
    g.next();
    assert_eq!(g.current_index(), 1);
    g.next();
    g.next();
    assert_eq!(g.current_index(), 0); // wraps
    g.prev();
    assert_eq!(g.current_index(), 2); // wraps back
    g.goto(1);
    assert_eq!(g.current_index(), 1);
}

// ===========================================================================
// Install
// ===========================================================================

#[test]
fn test_install_operation() {
    let op = InstallOperation::new("pkg", "Package", InstallAction::Install);
    assert_eq!(op.state, InstallState::Queued);
    assert!(!op.is_done());
    assert!((op.overall_progress() - 0.0).abs() < 0.01);
}

#[test]
fn test_install_progress() {
    let mut op = InstallOperation::new("pkg", "Package", InstallAction::Install);
    op.set_downloading(0.5);
    assert_eq!(op.state, InstallState::Downloading);
    op.set_installing(0.5);
    assert_eq!(op.state, InstallState::Installing);
    op.complete();
    assert!(op.is_done());
    assert!((op.overall_progress() - 1.0).abs() < 0.01);
}

#[test]
fn test_install_fail() {
    let mut op = InstallOperation::new("pkg", "Package", InstallAction::Install);
    op.fail("network error");
    assert!(op.is_done());
    assert_eq!(op.error.as_deref(), Some("network error"));
}

#[test]
fn test_install_queue() {
    let mut q = InstallQueue::new();
    q.enqueue(InstallOperation::new("a", "A", InstallAction::Install));
    q.enqueue(InstallOperation::new("b", "B", InstallAction::Remove));
    assert_eq!(q.count(), 2);
    assert_eq!(q.active_count(), 2);

    q.find_mut("a").unwrap().complete();
    assert_eq!(q.active_count(), 1);
    assert_eq!(q.completed().len(), 1);

    q.clear_completed();
    assert_eq!(q.count(), 1);
}

#[test]
fn test_install_action_display() {
    assert_eq!(InstallAction::Install.to_string(), "install");
    assert_eq!(InstallAction::Remove.to_string(), "remove");
    assert_eq!(InstallAction::Update.to_string(), "update");
}

// ===========================================================================
// Update
// ===========================================================================

#[test]
fn test_update_manager() {
    let mut um = UpdateManager::new(true);
    assert!(um.auto_check());
    assert_eq!(um.count(), 0);

    um.set_pending(vec![
        PendingUpdate {
            package_id: "a".into(),
            package_name: "A".into(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
            download_size: 1024,
            changelog: "New stuff".into(),
        },
    ]);
    assert_eq!(um.count(), 1);
    assert_eq!(um.total_download_size(), 1024);

    um.mark_checked(12345);
    assert_eq!(um.last_check(), 12345);

    um.remove("a");
    assert_eq!(um.count(), 0);
}

#[test]
fn test_pending_update_version_change() {
    let pu = PendingUpdate {
        package_id: "a".into(),
        package_name: "A".into(),
        current_version: Version::new(1, 0, 0),
        new_version: Version::new(2, 0, 0),
        download_size: 0,
        changelog: String::new(),
    };
    assert_eq!(pu.version_change(), "1.0.0 -> 2.0.0");
}

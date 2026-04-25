//! Tests for the places module.

use crate::places::{PlaceItem, PlaceType, PlacesModel};

#[test]
fn test_place_type_display() {
    assert_eq!(PlaceType::Bookmark.to_string(), "bookmark");
    assert_eq!(PlaceType::Device.to_string(), "device");
    assert_eq!(PlaceType::Network.to_string(), "network");
    assert_eq!(PlaceType::Trash.to_string(), "trash");
    assert_eq!(PlaceType::Recent.to_string(), "recent");
    assert_eq!(PlaceType::Separator.to_string(), "separator");
}

#[test]
fn test_place_item_bookmark() {
    let p = PlaceItem::bookmark("Home", "folder-home", "file:///home/user");
    assert_eq!(p.label, "Home");
    assert_eq!(p.place_type, PlaceType::Bookmark);
    assert!(!p.is_ejectable);
    assert!(!p.is_separator());
}

#[test]
fn test_place_item_device() {
    let p = PlaceItem::device(
        "USB Drive",
        "drive-removable",
        "file:///media/usb",
        true,
        Some(1_000_000),
    );
    assert_eq!(p.place_type, PlaceType::Device);
    assert!(p.is_ejectable);
    assert_eq!(p.free_space, Some(1_000_000));
}

#[test]
fn test_place_item_network() {
    let p = PlaceItem::network("NAS", "smb://nas/share");
    assert_eq!(p.place_type, PlaceType::Network);
    assert_eq!(p.icon, "network-server");
}

#[test]
fn test_place_item_separator() {
    let p = PlaceItem::separator();
    assert!(p.is_separator());
    assert!(p.label.is_empty());
}

#[test]
fn test_places_model_new_has_items() {
    let model = PlacesModel::new();
    assert!(!model.is_empty());
    // Should have bookmarks + separator + Recent + Trash at minimum.
    assert!(model.visible_count() >= 8); // 6 bookmarks + Recent + Trash
}

#[test]
fn test_places_model_has_trash_and_recent() {
    let model = PlacesModel::new();
    assert!(model.find("trash:///").is_some());
    assert!(model.find("recent:///").is_some());
}

#[test]
fn test_places_model_empty() {
    let model = PlacesModel::empty();
    assert!(model.is_empty());
}

#[test]
fn test_places_model_mount_device() {
    let mut model = PlacesModel::new();
    let before = model.len();
    model.mount_device(
        "USB",
        "drive-removable",
        "file:///media/usb",
        true,
        Some(500_000),
    );
    assert_eq!(model.device_count(), 1);
    // Should have more items now (device + separator).
    assert!(model.len() > before);
    assert!(model.find("file:///media/usb").is_some());
}

#[test]
fn test_places_model_mount_device_duplicate() {
    let mut model = PlacesModel::new();
    model.mount_device("USB", "drive", "file:///media/usb", false, None);
    model.mount_device("USB", "drive", "file:///media/usb", false, None);
    assert_eq!(model.device_count(), 1);
}

#[test]
fn test_places_model_unmount_device() {
    let mut model = PlacesModel::new();
    model.mount_device("USB", "drive", "file:///media/usb", false, None);
    model.unmount_device("file:///media/usb");
    assert_eq!(model.device_count(), 0);
    assert!(model.find("file:///media/usb").is_none());
}

#[test]
fn test_places_model_eject_device() {
    let mut model = PlacesModel::new();
    model.mount_device("USB", "drive", "file:///media/usb", true, None);
    assert!(model.eject_device("file:///media/usb"));
    assert_eq!(model.device_count(), 0);
}

#[test]
fn test_places_model_eject_non_ejectable() {
    let mut model = PlacesModel::new();
    model.mount_device("HDD", "drive", "file:///mnt/data", false, None);
    assert!(!model.eject_device("file:///mnt/data"));
    assert_eq!(model.device_count(), 1); // still mounted
}

#[test]
fn test_places_model_add_network() {
    let mut model = PlacesModel::new();
    model.add_network("NAS", "smb://nas/share");
    assert_eq!(model.network_count(), 1);
    assert!(model.find("smb://nas/share").is_some());
}

#[test]
fn test_places_model_remove_network() {
    let mut model = PlacesModel::new();
    model.add_network("NAS", "smb://nas/share");
    model.remove_network("smb://nas/share");
    assert_eq!(model.network_count(), 0);
}

#[test]
fn test_places_model_hide_trash() {
    let mut model = PlacesModel::new();
    model.set_show_trash(false);
    assert!(model.find("trash:///").is_none());
}

#[test]
fn test_places_model_hide_recent() {
    let mut model = PlacesModel::new();
    model.set_show_recent(false);
    assert!(model.find("recent:///").is_none());
}

#[test]
fn test_places_model_separators_present() {
    let model = PlacesModel::new();
    let sep_count = model.items().iter().filter(|p| p.is_separator()).count();
    // At least one separator (before virtual folders).
    assert!(sep_count >= 1);
}

#[test]
fn test_places_model_visible_count() {
    let model = PlacesModel::new();
    let total = model.len();
    let visible = model.visible_count();
    let seps = model.items().iter().filter(|p| p.is_separator()).count();
    assert_eq!(visible + seps, total);
}

use crate::tray::*;

#[test]
fn test_create_tray() {
    let tray = SystemTray::new();
    assert!(tray.is_empty());
    assert_eq!(tray.len(), 0);
}

#[test]
fn test_add_remove_item() {
    let mut tray = SystemTray::new();
    tray.add_item(TrayItem::new("vol", "Volume"));
    tray.add_item(TrayItem::new("net", "Network"));
    assert_eq!(tray.len(), 2);
    tray.remove_item("vol").unwrap();
    assert_eq!(tray.len(), 1);
    assert!(tray.find("vol").is_none());
}

#[test]
fn test_update_item() {
    let mut tray = SystemTray::new();
    tray.add_item(TrayItem::new("vol", "Volume"));
    tray.update_item("vol", |item| {
        item.title = "Volume: 50%".to_string();
        item.status = TrayItemStatus::NeedsAttention;
    })
    .unwrap();
    let item = tray.find("vol").unwrap();
    assert_eq!(item.title, "Volume: 50%");
    assert_eq!(item.status, TrayItemStatus::NeedsAttention);
}

#[test]
fn test_find() {
    let mut tray = SystemTray::new();
    tray.add_item(TrayItem::new("bt", "Bluetooth"));
    assert!(tray.find("bt").is_some());
    assert!(tray.find("wifi").is_none());
}

#[test]
fn test_status() {
    let mut item = TrayItem::new("test", "Test");
    assert_eq!(item.status, TrayItemStatus::Active);
    item.status = TrayItemStatus::Passive;
    assert_eq!(item.status, TrayItemStatus::Passive);
}

#[test]
fn test_menu_items() {
    let mut item = TrayItem::new("app", "App");
    item.menu.push(TrayMenuItem::new("open", "Open"));
    item.menu.push(TrayMenuItem::separator());
    item.menu.push(TrayMenuItem::new("quit", "Quit"));
    assert_eq!(item.menu.len(), 3);
    assert!(item.menu[1].separator);
    assert!(!item.menu[0].separator);
    assert!(item.menu[0].enabled);
}

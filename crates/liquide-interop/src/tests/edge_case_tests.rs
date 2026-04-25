use crate::desktop_entry::DesktopEntry;
use crate::mime::MimeType;
use crate::notification::*;
use crate::tray::SystemTray;
use crate::xdg::XdgDirs;

#[test]
fn test_empty_desktop_entry() {
    let result = DesktopEntry::parse("");
    assert!(result.is_err());
}

#[test]
fn test_invalid_mime() {
    assert!(MimeType::parse("noslash").is_err());
    assert!(MimeType::parse("").is_err());
}

#[test]
fn test_empty_tray() {
    let tray = SystemTray::new();
    assert!(tray.items().is_empty());
    assert!(tray.find("anything").is_none());
}

#[test]
fn test_notification_overflow() {
    let mut svc = MemoryNotificationService::new();
    for i in 0..100 {
        svc.notify(Notification::new("App", &format!("msg {i}")))
            .unwrap();
    }
    assert_eq!(svc.list().len(), 100);
}

#[test]
fn test_xdg_missing_runtime_dir() {
    let dirs = XdgDirs::new();
    assert!(dirs.runtime_dir.is_none());
}

#[test]
fn test_serde_roundtrip() {
    let mt = MimeType::parse("image/png").unwrap();
    let json = serde_json::to_string(&mt).unwrap();
    let d: MimeType = serde_json::from_str(&json).unwrap();
    assert_eq!(d.type_, "image");
    assert_eq!(d.subtype, "png");
}

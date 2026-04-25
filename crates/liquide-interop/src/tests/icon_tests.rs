use crate::icon::*;

#[test]
fn test_theme_creation() {
    let theme = IconTheme::new("Adwaita", "GNOME default icon theme");
    assert_eq!(theme.name, "Adwaita");
    assert!(theme.directories.is_empty());
}

#[test]
fn test_directory_contexts() {
    let dir = IconDirectory::new("apps/48", 48, IconContext::Applications, IconType::Fixed);
    assert_eq!(dir.size, 48);
    assert_eq!(dir.context, IconContext::Applications);
    assert_eq!(dir.scale, 1);
}

#[test]
fn test_icon_lookup_hit() {
    let mut theme = IconTheme::new("test", "test theme");
    theme.add_directory(IconDirectory::new(
        "apps/48/firefox",
        48,
        IconContext::Applications,
        IconType::Fixed,
    ));
    let mut lookup = IconLookup::new();
    lookup.add_theme(theme);
    let result = lookup.find_icon("firefox", 48, 1);
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.size, 48);
    assert_eq!(m.theme, "test");
}

#[test]
fn test_icon_lookup_miss() {
    let lookup = IconLookup::new();
    assert!(lookup.find_icon("nonexistent", 48, 1).is_none());
}

#[test]
fn test_scalable_preference() {
    let mut theme = IconTheme::new("test", "test theme");
    theme.add_directory(IconDirectory::new(
        "apps/24/edit",
        24,
        IconContext::Actions,
        IconType::Fixed,
    ));
    theme.add_directory(IconDirectory::new(
        "apps/scalable/edit",
        0,
        IconContext::Actions,
        IconType::Scalable,
    ));
    let mut lookup = IconLookup::new();
    lookup.add_theme(theme);
    // Requesting size 32 which doesn't match 24 exactly -> should get scalable
    let result = lookup.find_icon("edit", 32, 1);
    assert!(result.is_some());
    assert_eq!(result.unwrap().icon_type, IconType::Scalable);
}

#[test]
fn test_theme_display() {
    let mut theme = IconTheme::new("Adwaita", "GNOME default");
    theme.add_directory(IconDirectory::new(
        "apps/48",
        48,
        IconContext::Applications,
        IconType::Fixed,
    ));
    let s = format!("{theme}");
    assert!(s.contains("Adwaita"));
    assert!(s.contains("1 dirs"));
}

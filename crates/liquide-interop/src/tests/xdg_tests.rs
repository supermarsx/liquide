use crate::xdg::XdgDirs;

#[test]
fn test_defaults() {
    let dirs = XdgDirs::new();
    assert!(dirs.data_home.contains(".local/share"));
    assert!(dirs.config_home.contains(".config"));
    assert!(dirs.cache_home.contains(".cache"));
}

#[test]
fn test_custom_home() {
    let dirs = XdgDirs::with_home("/home/alice");
    assert_eq!(dirs.data_home, "/home/alice/.local/share");
    assert_eq!(dirs.config_home, "/home/alice/.config");
    assert_eq!(dirs.cache_home, "/home/alice/.cache");
    assert_eq!(dirs.state_home, "/home/alice/.local/state");
}

#[test]
fn test_find_data_file() {
    let dirs = XdgDirs::with_home("/home/alice");
    let paths = dirs.find_data_file("icons/hicolor");
    assert_eq!(paths[0], "/home/alice/.local/share/icons/hicolor");
    assert!(paths.len() >= 3);
}

#[test]
fn test_find_config_file() {
    let dirs = XdgDirs::with_home("/home/alice");
    let paths = dirs.find_config_file("myapp/config.toml");
    assert_eq!(paths[0], "/home/alice/.config/myapp/config.toml");
    assert!(paths.len() >= 2);
}

#[test]
fn test_data_dirs() {
    let dirs = XdgDirs::new();
    assert!(dirs.data_dirs.contains(&"/usr/local/share".to_string()));
    assert!(dirs.data_dirs.contains(&"/usr/share".to_string()));
}

#[test]
fn test_display() {
    let dirs = XdgDirs::new();
    let s = format!("{dirs}");
    assert!(s.contains("XdgDirs"));
    assert!(s.contains(".config"));
}

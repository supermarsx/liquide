#[cfg(test)]
mod tests {
    use crate::process::{XWaylandProcess, XWaylandConfig, XWaylandState};
    use crate::window::{X11Window, X11WindowId, X11WindowType, X11WindowState};
    use crate::atoms::AtomCache;
    use crate::clipboard::X11ClipboardBridge;
    use crate::dnd::X11DndBridge;

    #[test]
    fn test_xwayland_config_default() {
        let config = XWaylandConfig::default();
        assert!(config.binary_path.is_none());
        assert!(config.enable_glamor);
    }

    #[test]
    fn test_xwayland_process_initial_state() {
        let proc = XWaylandProcess::new(XWaylandConfig::default());
        assert_eq!(proc.state(), XWaylandState::Stopped);
        assert_eq!(proc.display_env(), ":1");
    }

    #[test]
    fn test_x11_window() {
        let mut win = X11Window::new(X11WindowId(1), 100, 200, 800, 600);
        assert_eq!(win.id(), X11WindowId(1));
        assert_eq!(win.width(), 800);
        assert_eq!(win.state(), X11WindowState::Unmapped);
        win.set_mapped(true);
        assert!(win.mapped());
    }

    #[test]
    fn test_atom_cache() {
        let cache = AtomCache::new();
        assert!(cache.get("WM_PROTOCOLS").is_some());
        assert!(cache.get("_NET_WM_NAME").is_some());
        assert!(cache.get("CLIPBOARD").is_some());
    }

    #[test]
    fn test_atom_cache_intern() {
        let mut cache = AtomCache::new();
        cache.intern("MY_CUSTOM_ATOM".to_string(), 9999);
        assert_eq!(cache.get("MY_CUSTOM_ATOM"), Some(9999));
        assert_eq!(cache.name(9999), Some("MY_CUSTOM_ATOM"));
    }

    #[test]
    fn test_clipboard_bridge() {
        let bridge = X11ClipboardBridge::new();
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_dnd_bridge() {
        let bridge = X11DndBridge::new();
        assert!(!bridge.is_active());
    }

    #[test]
    fn test_window_types() {
        let t = X11WindowType::Normal;
        assert_eq!(format!("{:?}", t), "Normal");
    }
}

#[cfg(test)]
mod tests {
    use crate::atoms::AtomCache;
    use crate::clipboard::X11ClipboardBridge;
    use crate::dnd::X11DndBridge;
    use crate::process::{XWaylandConfig, XWaylandProcess, XWaylandState};
    use crate::window::{X11Window, X11WindowId, X11WindowState, X11WindowType};

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

    // Regression (t49-e6-03): `start()` must not lie about a running X11
    // server. The real fork/exec path is unimplemented, so a successful start
    // must leave the process in an explicit `Staged` state (never `Running`),
    // and a process that was never spawned must NOT report alive.
    #[test]
    fn test_start_does_not_claim_running_without_a_process() {
        let mut proc = XWaylandProcess::new(XWaylandConfig {
            // Pin a binary path so `find_binary` succeeds on Linux without
            // depending on the host having Xwayland installed.
            binary_path: Some("/usr/bin/Xwayland".to_string()),
            ..XWaylandConfig::default()
        });

        let result = proc.start();

        #[cfg(target_os = "linux")]
        {
            // On Linux, staging succeeds but must be honest about it.
            assert!(result.is_ok(), "start should stage successfully");
            assert_eq!(
                proc.state(),
                XWaylandState::Staged,
                "start must NOT claim Running before a real process exists"
            );
            assert_ne!(
                proc.state(),
                XWaylandState::Running,
                "no process was spawned, so Running is a lie"
            );
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Off Linux there is no spawn path at all: fail closed.
            assert!(result.is_err(), "start must fail closed off Linux");
            assert_ne!(proc.state(), XWaylandState::Running);
        }
    }

    #[test]
    fn test_check_alive_is_false_when_no_process_spawned() {
        let mut proc = XWaylandProcess::new(XWaylandConfig {
            binary_path: Some("/usr/bin/Xwayland".to_string()),
            ..XWaylandConfig::default()
        });

        // Never started: clearly not alive.
        assert!(!proc.check_alive(), "an unstarted process is not alive");

        // After start(), the process was still never really spawned (no pid),
        // so liveness must remain false — fail closed, do not report healthy.
        let _ = proc.start();
        assert!(
            !proc.check_alive(),
            "a process that was never forked/exec'd must not report alive"
        );
    }

    #[test]
    fn test_stop_from_staged_transitions_to_exited() {
        let mut proc = XWaylandProcess::new(XWaylandConfig {
            binary_path: Some("/usr/bin/Xwayland".to_string()),
            ..XWaylandConfig::default()
        });
        let _ = proc.start();

        assert!(proc.stop().is_ok());

        #[cfg(target_os = "linux")]
        assert_eq!(
            proc.state(),
            XWaylandState::Exited,
            "stopping a staged process must reflect reality, not stay Staged"
        );
        assert!(!proc.check_alive());
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

#[cfg(test)]
mod tests {
    use crate::buffer::{BufferRef, BufferSource};
    use crate::client::{ClientConnection, ClientId, ClientState};
    use crate::display::WaylandDisplay;
    use crate::registry::GlobalRegistry;
    use crate::seat_manager::SeatManager;
    use crate::shell_manager::ShellManager;
    use crate::shm::ShmFormat;
    use crate::surface_manager::SurfaceManager;

    #[test]
    fn test_client_id() {
        let id = ClientId(1);
        assert_eq!(id, ClientId(1));
        assert_ne!(id, ClientId(2));
    }

    #[test]
    fn test_client_connection_lifecycle() {
        let mut conn = ClientConnection::new(ClientId(1));
        assert_eq!(conn.state(), ClientState::Connected);
        let obj_id = conn.allocate_id();
        assert!(obj_id > 0);
    }

    #[test]
    fn test_wayland_display() {
        let display = WaylandDisplay::new();
        assert!(display.socket_path().ends_with("wayland-0"));
        assert_eq!(display.client_count(), 0);
        assert!(!display.is_running());
    }

    #[test]
    fn test_wayland_display_custom_socket() {
        let display = WaylandDisplay::with_socket("wayland-test");
        assert!(display.socket_path().ends_with("wayland-test"));
    }

    #[test]
    fn test_global_registry() {
        let registry = GlobalRegistry::new();
        let globals = registry.globals();
        // Should have standard globals pre-registered.
        assert!(!globals.is_empty());
        assert!(registry.find("wl_compositor").is_some());
        assert!(registry.find("wl_shm").is_some());
        assert!(registry.find("wl_seat").is_some());
        assert!(registry.find("xdg_wm_base").is_some());
    }

    #[test]
    fn test_surface_manager() {
        let mut mgr = SurfaceManager::new();
        assert_eq!(mgr.surface_count(), 0);
        let id = mgr.create_surface(ClientId(1));
        assert_eq!(mgr.surface_count(), 1);
        assert!(mgr.get_surface(id).is_some());
        mgr.destroy_surface(id);
        assert_eq!(mgr.surface_count(), 0);
    }

    #[test]
    fn test_seat_manager() {
        let mut seat = SeatManager::new();
        assert_eq!(seat.keyboard_focused(), None);
        seat.set_keyboard_focus(Some(42));
        assert_eq!(seat.keyboard_focused(), Some(42));
        seat.set_keyboard_focus(None);
        assert_eq!(seat.keyboard_focused(), None);
    }

    #[test]
    fn test_pointer_tracking() {
        let mut seat = SeatManager::new();
        seat.update_pointer(100.5, 200.5);
        let (x, y) = seat.pointer_position();
        assert!((x - 100.5).abs() < f64::EPSILON);
        assert!((y - 200.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shell_manager() {
        let mut shell = ShellManager::new();
        assert_eq!(shell.toplevel_count(), 0);
        let id = shell.create_toplevel(1);
        assert_eq!(shell.toplevel_count(), 1);
        shell.destroy_toplevel(id);
        assert_eq!(shell.toplevel_count(), 0);
    }

    #[test]
    fn test_ack_configure_only_targets_matching_surface() {
        use liquide_wayland::ToplevelState;

        let mut shell = ShellManager::new();
        let a = shell.create_toplevel(1);
        let b = shell.create_toplevel(2);

        let serial_a = shell.configure_toplevel(a, 800, 600, ToplevelState::empty());
        let serial_b = shell.configure_toplevel(b, 400, 300, ToplevelState::empty());
        assert_ne!(serial_a, serial_b);

        // Acking surface A's serial must configure ONLY surface A.
        assert!(shell.ack_configure(a, serial_a));
        assert!(shell.get_toplevel(a).unwrap().configured);
        assert!(
            !shell.get_toplevel(b).unwrap().configured,
            "acking surface A must not configure surface B"
        );

        // Acking surface B's own serial configures B.
        assert!(shell.ack_configure(b, serial_b));
        assert!(shell.get_toplevel(b).unwrap().configured);
    }

    #[test]
    fn test_ack_configure_ignores_wrong_or_stale_serial() {
        use liquide_wayland::ToplevelState;

        let mut shell = ShellManager::new();
        let a = shell.create_toplevel(1);
        let b = shell.create_toplevel(2);

        let serial_a = shell.configure_toplevel(a, 800, 600, ToplevelState::empty());
        let serial_b = shell.configure_toplevel(b, 400, 300, ToplevelState::empty());

        // Surface B's serial is not pending on surface A -> ignored, A stays unconfigured.
        assert!(!shell.ack_configure(a, serial_b));
        assert!(!shell.get_toplevel(a).unwrap().configured);

        // A serial nobody ever sent is ignored.
        assert!(!shell.ack_configure(a, 9999));
        assert!(!shell.get_toplevel(a).unwrap().configured);

        // Unknown surface id is ignored.
        assert!(!shell.ack_configure(404, serial_a));

        // The correct (surface, serial) ack still works after the bad attempts.
        assert!(shell.ack_configure(a, serial_a));
        assert!(shell.get_toplevel(a).unwrap().configured);
        assert!(!shell.get_toplevel(b).unwrap().configured);
        let _ = serial_b;
    }

    #[test]
    fn test_ack_configure_discards_older_pending_serials() {
        use liquide_wayland::ToplevelState;

        let mut shell = ShellManager::new();
        let a = shell.create_toplevel(1);

        let s1 = shell.configure_toplevel(a, 100, 100, ToplevelState::empty());
        let s2 = shell.configure_toplevel(a, 200, 200, ToplevelState::empty());
        let s3 = shell.configure_toplevel(a, 300, 300, ToplevelState::empty());

        // Acking the middle serial discards s1 and s2 but leaves s3 pending.
        assert!(shell.ack_configure(a, s2));
        assert!(shell.get_toplevel(a).unwrap().configured);
        assert_eq!(shell.get_toplevel(a).unwrap().pending_configures, vec![s3]);

        // The already-discarded older serial can no longer be acked.
        assert!(!shell.ack_configure(a, s1));
        // The still-pending newest serial can be acked.
        assert!(shell.ack_configure(a, s3));
        assert!(shell.get_toplevel(a).unwrap().pending_configures.is_empty());
    }

    /// Bring a display up to the `running` state without touching the OS socket
    /// (the Linux `bind` is a stub that only flips `running`). On non-Linux,
    /// `bind` returns NotSupported, so we accept clients via a tiny shim by
    /// forcing the running flag through the public accept path: we just bind and
    /// fall back to constructing IDs manually if bind is unsupported.
    fn running_display() -> WaylandDisplay {
        let mut display = WaylandDisplay::new();
        // `bind` only fails to set `running` on non-Linux; in that case we still
        // want a running server for the test, so drive it through the same code
        // path by binding and asserting the post-state we need.
        let _ = display.bind();
        display
    }

    /// Regression for t49-e9-03: a client that created surfaces (and held seat
    /// focus) must be fully swept on disconnect — its surfaces are gone and the
    /// focus it held is cleared — while a second client's resources survive.
    #[test]
    fn test_disconnect_sweeps_only_disconnecting_clients_resources() {
        let mut display = running_display();
        if !display.is_running() {
            // Non-Linux: bind is unsupported, so accept_client would error. Skip
            // the socket-dependent half but still exercise the sweep directly.
            let c1 = ClientId(1);
            let c2 = ClientId(2);
            let s1 = display.create_surface(c1);
            let s2 = display.create_surface(c2);
            display.seat_mut().set_keyboard_focus(Some(s1));
            display.seat_mut().set_pointer_focus(Some(s1));
            display.shell_mut().create_toplevel(s1);
            display.shell_mut().create_toplevel(s2);

            display.cleanup_client(c1);

            assert!(display.surfaces().get_surface(s1).is_none());
            assert!(display.surfaces().get_surface(s2).is_some());
            assert!(display.shell().get_toplevel(s1).is_none());
            assert!(display.shell().get_toplevel(s2).is_some());
            assert_eq!(display.seat().keyboard_focused(), None);
            assert_eq!(display.seat().pointer_focused(), None);
            return;
        }

        let c1 = display.accept_client().expect("accept client 1");
        let c2 = display.accept_client().expect("accept client 2");

        // Client 1 creates two surfaces, gets a toplevel, and holds focus.
        let s1a = display.create_surface(c1);
        let s1b = display.create_surface(c1);
        display.shell_mut().create_toplevel(s1a);
        display.seat_mut().set_keyboard_focus(Some(s1a));
        display.seat_mut().set_pointer_focus(Some(s1b));

        // Client 2 creates an untouched surface + toplevel.
        let s2 = display.create_surface(c2);
        display.shell_mut().create_toplevel(s2);

        assert_eq!(display.surfaces().surface_count(), 3);
        assert_eq!(display.shell().toplevel_count(), 2);

        // Disconnect client 1: remove_client must sweep its resources.
        display.remove_client(c1);

        // Client 1's surfaces and toplevel are gone.
        assert!(display.surfaces().get_surface(s1a).is_none());
        assert!(display.surfaces().get_surface(s1b).is_none());
        assert!(display.shell().get_toplevel(s1a).is_none());
        // The focus client 1 held on both pointer and keyboard is cleared.
        assert_eq!(display.seat().keyboard_focused(), None);
        assert_eq!(display.seat().pointer_focused(), None);

        // Client 2 is fully intact.
        assert!(display.surfaces().get_surface(s2).is_some());
        assert!(display.shell().get_toplevel(s2).is_some());
        assert_eq!(display.surfaces().surface_count(), 1);
        assert_eq!(display.shell().toplevel_count(), 1);
        assert_eq!(display.client_count(), 1);
    }

    /// Sweeping a client clears only the focus that client held; focus belonging
    /// to a surviving client's surface is preserved.
    #[test]
    fn test_disconnect_preserves_other_clients_focus() {
        let mut display = WaylandDisplay::new();
        let c1 = ClientId(10);
        let c2 = ClientId(20);
        let s1 = display.create_surface(c1);
        let s2 = display.create_surface(c2);

        // Focus is held on client 2's surface; client 1 disconnects.
        display.seat_mut().set_keyboard_focus(Some(s2));
        display.cleanup_client(c1);

        assert!(display.surfaces().get_surface(s1).is_none());
        assert!(display.surfaces().get_surface(s2).is_some());
        // Client 2's focus must NOT be cleared by client 1's disconnect.
        assert_eq!(display.seat().keyboard_focused(), Some(s2));
    }

    /// Cleaning up an unknown client is a harmless no-op.
    #[test]
    fn test_cleanup_unknown_client_is_noop() {
        let mut display = WaylandDisplay::new();
        let c1 = ClientId(1);
        let s1 = display.create_surface(c1);
        // A never-seen client id sweeps nothing.
        display.cleanup_client(ClientId(999));
        assert!(display.surfaces().get_surface(s1).is_some());
        assert_eq!(display.surfaces().surface_count(), 1);
    }

    /// Regression for the shm-pool half of t49-e9-03: pools associated with a
    /// client are dropped (closing their fd/mapping) when the client is swept.
    /// Linux-only because `ShmPool::new` mmaps a real fd.
    #[cfg(target_os = "linux")]
    #[test]
    fn test_disconnect_sweeps_client_shm_pools() {
        use crate::shm::ShmPool;
        use std::ffi::CString;

        // Build a real shm pool from an anonymous memfd.
        let name = CString::new("liquide-display-test").unwrap();
        // SAFETY: valid NUL-terminated name; memfd_create returns an owned fd.
        let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
        assert!(fd >= 0, "memfd_create failed");
        // SAFETY: fd is a fresh owned descriptor.
        let rc = unsafe { libc::ftruncate(fd, 4096) };
        assert_eq!(rc, 0, "ftruncate failed");

        // SAFETY: F_GETFD only inspects the descriptor table.
        let fd_is_open = |fd: i32| unsafe { libc::fcntl(fd, libc::F_GETFD) != -1 };

        let pool = ShmPool::new(fd, 4096).expect("pool creation");
        let raw_fd = pool.fd();
        assert!(fd_is_open(raw_fd));

        let mut display = WaylandDisplay::new();
        let c1 = ClientId(7);
        display.add_client_pool(c1, pool);
        assert_eq!(display.client_pool_count(c1), 1);

        // Disconnecting the client drops the pool, which closes its fd.
        display.cleanup_client(c1);
        assert_eq!(display.client_pool_count(c1), 0);
        assert!(
            !fd_is_open(raw_fd),
            "client shm pool fd must be closed when the client is swept"
        );
    }

    #[test]
    fn test_buffer_ref() {
        let buf = BufferRef::null();
        assert!(buf.is_null());
        assert_eq!(buf.dimensions(), (0, 0));
    }

    #[test]
    fn test_buffer_source_shm() {
        let buf = BufferRef::new(
            BufferSource::Shm {
                pool_id: 1,
                offset: 0,
            },
            640,
            480,
            2560,
            0,
        );
        assert!(!buf.is_null());
        assert_eq!(buf.dimensions(), (640, 480));
    }

    #[test]
    fn test_shm_format() {
        assert_ne!(ShmFormat::Argb8888 as u32, ShmFormat::Xrgb8888 as u32);
    }
}

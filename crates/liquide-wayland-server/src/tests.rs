#[cfg(test)]
mod tests {
    use crate::client::{ClientId, ClientConnection, ClientState};
    use crate::display::WaylandDisplay;
    use crate::global::{Global, GlobalId};
    use crate::registry::GlobalRegistry;
    use crate::surface_manager::SurfaceManager;
    use crate::seat_manager::SeatManager;
    use crate::shell_manager::ShellManager;
    use crate::buffer::{BufferRef, BufferSource};
    use crate::shm::ShmFormat;

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
    fn test_buffer_ref() {
        let buf = BufferRef::null();
        assert!(buf.is_null());
        assert_eq!(buf.dimensions(), (0, 0));
    }

    #[test]
    fn test_buffer_source_shm() {
        let buf = BufferRef::new(
            BufferSource::Shm { pool_id: 1, offset: 0 },
            640, 480, 2560, 0,
        );
        assert!(!buf.is_null());
        assert_eq!(buf.dimensions(), (640, 480));
    }

    #[test]
    fn test_shm_format() {
        assert_ne!(ShmFormat::Argb8888 as u32, ShmFormat::Xrgb8888 as u32);
    }
}

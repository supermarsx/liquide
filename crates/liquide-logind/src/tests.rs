#[cfg(test)]
mod tests {
    use crate::session::{StubSession, SessionProvider, SessionState};
    use crate::seat::StubSeat;
    use crate::vt::VtMode;
    use crate::privileges::Privileges;

    #[test]
    fn test_stub_session() {
        let mut session = StubSession::new();
        assert_eq!(session.state(), SessionState::Active);
        assert!(!session.has_control());

        session.take_control().unwrap();
        assert!(session.has_control());

        session.release_control().unwrap();
        assert!(!session.has_control());
    }

    #[test]
    fn test_session_info() {
        let session = StubSession::new();
        let info = session.session_info().unwrap();
        assert_eq!(info.seat_id, "seat0");
        assert_eq!(info.vt_number, 7);
    }

    #[test]
    fn test_session_poll_event() {
        let mut session = StubSession::new();
        assert!(session.poll_event().is_none());
    }

    #[test]
    fn test_stub_seat() {
        use crate::seat::SeatBackend;
        let seat = StubSeat::new();
        let info = seat.seat_info().unwrap();
        assert_eq!(info.id, "seat0");
    }

    #[test]
    fn test_vt_mode() {
        assert_ne!(VtMode::Text, VtMode::Graphics);
    }

    #[test]
    fn test_environment_setup() {
        let env = Privileges::setup_environment(1000);
        assert!(env.contains_key("XDG_RUNTIME_DIR"));
        assert!(env.contains_key("WAYLAND_DISPLAY"));
    }

    #[test]
    fn test_runtime_dir_format() {
        let path = Privileges::setup_runtime_dir(1000);
        assert!(path.is_ok());
        assert!(path.unwrap().contains("1000"));
    }
}

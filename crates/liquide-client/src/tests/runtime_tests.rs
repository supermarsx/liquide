use crate::config::ClientConfig;
use crate::connection::{ConnectionQuality, ConnectionState};
use crate::crash_screen::CrashScreenType;
use crate::display::DisplayMode;
use crate::runtime::ClientRuntime;

fn make_runtime() -> ClientRuntime {
    ClientRuntime::new(ClientConfig::default())
}

#[test]
fn test_initial_state_disconnected() {
    let runtime = make_runtime();
    assert_eq!(runtime.state(), ConnectionState::Disconnected);
    assert_eq!(runtime.quality(), ConnectionQuality::Disconnected);
}

#[tokio::test]
async fn test_connect_and_disconnect() {
    let (addr, trust_cert, server) = super::helpers::mock_tls_server(true).await;
    let mut runtime = make_runtime();
    runtime.add_trusted_server_certificate_for_tests(trust_cert);
    runtime.connect(&addr.to_string()).await.unwrap();
    assert_eq!(runtime.state(), ConnectionState::Connected);

    runtime.disconnect().await;
    assert_eq!(runtime.state(), ConnectionState::Disconnected);
    server.await.unwrap();
}

#[tokio::test]
async fn test_audit_events_on_connect() {
    let (addr, trust_cert, server) = super::helpers::mock_tls_server(true).await;
    let mut runtime = make_runtime();
    runtime.add_trusted_server_certificate_for_tests(trust_cert);
    runtime.connect(&addr.to_string()).await.unwrap();

    let events = runtime.drain_audit_events();
    assert!(events.len() >= 2);
    assert_eq!(events[0].event_name(), "connection_attempt");
    assert_eq!(events[1].event_name(), "connected");

    runtime.disconnect().await;
    server.await.unwrap();
}

#[tokio::test]
async fn test_drain_clears_events() {
    let (addr, trust_cert, server) = super::helpers::mock_tls_server(true).await;
    let mut runtime = make_runtime();
    runtime.add_trusted_server_certificate_for_tests(trust_cert);
    runtime.connect(&addr.to_string()).await.unwrap();
    let events1 = runtime.drain_audit_events();
    assert!(!events1.is_empty());
    let events2 = runtime.drain_audit_events();
    assert!(events2.is_empty());

    runtime.disconnect().await;
    server.await.unwrap();
}

#[test]
fn test_toggle_fullscreen() {
    let mut runtime = make_runtime();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::SingleWindow
    );
    runtime.toggle_fullscreen();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::Fullscreen
    );
    runtime.toggle_fullscreen();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::SingleWindow
    );
}

#[test]
fn test_cycle_display_mode() {
    let mut runtime = make_runtime();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::SingleWindow
    );
    runtime.cycle_display_mode();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::Fullscreen
    );
    runtime.cycle_display_mode();
    assert_eq!(
        runtime.display_manager_mut().current_mode(),
        DisplayMode::Tabbed
    );
}

#[test]
fn test_show_crash_screen() {
    let mut runtime = make_runtime();
    runtime.show_crash_screen(CrashScreenType::SessionCrash, 42, "something broke");
    assert!(runtime.crash_screen_mut().is_visible());

    let data = runtime.crash_screen_mut().data().unwrap();
    assert_eq!(data.error_code, 42);
    assert!(data.restart_available);

    let events = runtime.drain_audit_events();
    assert!(
        events
            .iter()
            .any(|e| e.event_name() == "crash_screen_shown")
    );
}

#[test]
fn test_overlay_toggle() {
    let mut runtime = make_runtime();
    assert!(!runtime.overlay_mut().is_visible());
    runtime.toggle_overlay();
    assert!(runtime.overlay_mut().is_visible());
    runtime.toggle_overlay();
    assert!(!runtime.overlay_mut().is_visible());
}

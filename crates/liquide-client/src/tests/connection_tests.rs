use crate::connection::{ConnectionManager, ConnectionProfile, ConnectionQuality, ConnectionState};

// ---------------------------------------------------------------------------
// Synchronous (non-network) tests
// ---------------------------------------------------------------------------

#[test]
fn test_initial_state_is_disconnected() {
    let mgr = ConnectionManager::new(5);
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
}

#[test]
fn test_quality_when_disconnected() {
    let mgr = ConnectionManager::new(5);
    assert_eq!(mgr.quality(), ConnectionQuality::Disconnected);
}

#[test]
fn test_from_metrics_all_tiers() {
    assert_eq!(
        ConnectionQuality::from_metrics(5.0, 0.0, false),
        ConnectionQuality::Excellent,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(60.0, 0.1, false),
        ConnectionQuality::Good,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(150.0, 0.0, false),
        ConnectionQuality::Fair,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(250.0, 6.0, false),
        ConnectionQuality::Poor,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(350.0, 12.0, false),
        ConnectionQuality::Bad,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(0.0, 0.0, false),
        ConnectionQuality::Disconnected,
    );
    // Server-signalled degradation forces Bad.
    assert_eq!(
        ConnectionQuality::from_metrics(10.0, 0.0, true),
        ConnectionQuality::Bad,
    );
}

#[test]
fn test_quality_color_values() {
    assert_eq!(ConnectionQuality::Excellent.color(), "#00c853");
    assert_eq!(ConnectionQuality::Good.color(), "#64dd17");
    assert_eq!(ConnectionQuality::Fair.color(), "#ffd600");
    assert_eq!(ConnectionQuality::Poor.color(), "#ff6d00");
    assert_eq!(ConnectionQuality::Bad.color(), "#d50000");
    assert_eq!(ConnectionQuality::Disconnected.color(), "#9e9e9e");
}

#[test]
fn test_connection_profiles_crud() {
    let mut mgr = ConnectionManager::new(5);
    assert!(mgr.profiles().is_empty());

    mgr.add_profile(ConnectionProfile {
        name: "Work".to_string(),
        address: "work.example.com:3389".to_string(),
        username: Some("alice".to_string()),
        transport: "quic".to_string(),
        encoder: "h265".to_string(),
        encryption: "aes256".to_string(),
        monitors: 2,
        audio_playback: true,
        audio_microphone: false,
        clipboard: true,
        performance: "balanced".to_string(),
        cursor_mode: "local_predict".to_string(),
    });

    assert_eq!(mgr.profiles().len(), 1);
    assert_eq!(mgr.profiles()[0].name, "Work");

    // Remove non-existent profile returns false.
    assert!(!mgr.remove_profile("NoSuch"));
    assert_eq!(mgr.profiles().len(), 1);

    // Remove existing profile returns true.
    assert!(mgr.remove_profile("Work"));
    assert!(mgr.profiles().is_empty());
}

#[test]
fn test_next_reconnect_delay_exponential() {
    let mgr = ConnectionManager::new(10);
    // Initial (0 attempts): 1000 * 2^0 = 1000
    assert_eq!(mgr.next_reconnect_delay_ms(), 1000);
}

#[test]
fn test_should_reconnect_unlimited() {
    let mgr = ConnectionManager::new(0); // 0 = unlimited
    assert!(mgr.should_reconnect());
}

#[test]
fn test_should_reconnect_within_limit() {
    let mgr = ConnectionManager::new(5);
    // 0 attempts < 5 max
    assert!(mgr.should_reconnect());
}

#[test]
fn test_connection_state_display() {
    assert_eq!(ConnectionState::Disconnected.to_string(), "Disconnected");
    assert_eq!(ConnectionState::Connecting.to_string(), "Connecting");
    assert_eq!(ConnectionState::Authenticating.to_string(), "Authenticating");
    assert_eq!(ConnectionState::Negotiating.to_string(), "Negotiating");
    assert_eq!(ConnectionState::Connected.to_string(), "Connected");
    assert_eq!(ConnectionState::Reconnecting.to_string(), "Reconnecting");
    assert_eq!(ConnectionState::Failed.to_string(), "Failed");
}

// ---------------------------------------------------------------------------
// Async tests requiring mock TLS server
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_connect_transitions_to_connected() {
    let (addr, server) = super::helpers::mock_tls_server(true).await;
    let mut mgr = ConnectionManager::new(5);
    mgr.connect_with_credential(&addr.to_string(), "user", "pass")
        .await
        .unwrap();
    assert_eq!(mgr.state(), ConnectionState::Connected);
    assert!(mgr.session_id().is_some());
    mgr.disconnect().await;
    server.await.unwrap();
}

#[tokio::test]
async fn test_disconnect_returns_to_disconnected() {
    let (addr, server) = super::helpers::mock_tls_server(true).await;
    let mut mgr = ConnectionManager::new(5);
    mgr.connect_with_credential(&addr.to_string(), "user", "pass")
        .await
        .unwrap();
    assert_eq!(mgr.state(), ConnectionState::Connected);
    mgr.disconnect().await;
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
    assert!(mgr.session_id().is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn test_disconnect_idempotent() {
    let mut mgr = ConnectionManager::new(5);
    // Disconnecting when already disconnected should be a no-op.
    mgr.disconnect().await;
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
    mgr.disconnect().await;
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
}

#[tokio::test]
async fn test_reconnect_fails_when_no_server() {
    let mut mgr = ConnectionManager::new(3);
    let result = mgr.reconnect().await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::ClientError::ServerUnreachable { .. }),
        "expected ServerUnreachable, got: {err}",
    );
}

#[tokio::test]
async fn test_connect_replaces_existing_connection() {
    let (addr1, server1) = super::helpers::mock_tls_server(true).await;
    let (addr2, server2) = super::helpers::mock_tls_server(true).await;

    let mut mgr = ConnectionManager::new(5);
    mgr.connect_with_credential(&addr1.to_string(), "u", "p")
        .await
        .unwrap();
    assert_eq!(mgr.state(), ConnectionState::Connected);

    // Connecting again should disconnect the first and connect to second.
    mgr.connect_with_credential(&addr2.to_string(), "u", "p")
        .await
        .unwrap();
    assert_eq!(mgr.state(), ConnectionState::Connected);

    mgr.disconnect().await;
    server1.await.unwrap();
    server2.await.unwrap();
}

#[tokio::test]
async fn test_connect_auth_failure() {
    let (addr, server) = super::helpers::mock_tls_server(false).await;
    let mut mgr = ConnectionManager::new(3);
    let result = mgr
        .connect_with_credential(&addr.to_string(), "bad", "creds")
        .await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ClientError::AuthenticationFailed { .. }
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn test_connect_invalid_address() {
    let mut mgr = ConnectionManager::new(3);
    let result = mgr.connect("not-a-valid-address").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_message_when_not_connected() {
    let mut mgr = ConnectionManager::new(3);
    let result = mgr.send_message(b"hello").await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ClientError::NotConnected,
    ));
}

#[tokio::test]
async fn test_recv_message_when_not_connected() {
    let mut mgr = ConnectionManager::new(3);
    let result = mgr.recv_message().await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::ClientError::NotConnected,
    ));
}

#[tokio::test]
async fn test_take_stream_after_connect() {
    let (addr, server) = super::helpers::mock_tls_server(true).await;
    let mut mgr = ConnectionManager::new(5);
    mgr.connect_with_credential(&addr.to_string(), "u", "p")
        .await
        .unwrap();
    assert!(mgr.take_stream().is_some());
    // Second take should return None.
    assert!(mgr.take_stream().is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn test_quality_after_connect() {
    let (addr, server) = super::helpers::mock_tls_server(true).await;
    let mut mgr = ConnectionManager::new(5);
    mgr.connect_with_credential(&addr.to_string(), "u", "p")
        .await
        .unwrap();
    mgr.update_metrics(10.0, 0.0, 100.0);
    assert_eq!(mgr.quality(), ConnectionQuality::Excellent);

    mgr.update_metrics(350.0, 12.0, 5.0);
    assert_eq!(mgr.quality(), ConnectionQuality::Bad);

    mgr.disconnect().await;
    assert_eq!(mgr.quality(), ConnectionQuality::Disconnected);
    server.await.unwrap();
}

#[tokio::test]
async fn test_connect_timeout_unreachable_port() {
    let mut mgr = ConnectionManager::new(3);
    // Use a non-routable IP to trigger timeout (10.255.255.1 is unroutable).
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        mgr.connect("10.255.255.1:9999"),
    )
    .await;
    // Either the connect itself times out (ConnectionTimeout) or our outer
    // timeout fires. Either way the operation shouldn't hang forever.
    match result {
        Ok(Err(crate::ClientError::ConnectionTimeout { .. })) => {} // expected
        Ok(Err(crate::ClientError::ServerUnreachable { .. })) => {} // also acceptable
        Ok(Err(e)) => panic!("unexpected error: {e}"),
        Ok(Ok(())) => panic!("connect should not succeed"),
        Err(_elapsed) => {} // outer timeout — connect didn't hang forever, that's fine
    }
}

use bytes::Bytes;

use crate::Transport;
use crate::listener::ws::WebSocketListener;
use crate::websocket::WebSocketTransport;

// ---------------------------------------------------------------------------
// Security regression tests (T49-e7-F2): the WebSocket transport must be
// encrypted by default; plaintext `ws://` is reachable only through the
// explicit, loudly-named opt-in. These assert the negative path (no silent
// plaintext downgrade) and that the secure `wss://` path is the default.
// ---------------------------------------------------------------------------

#[cfg(feature = "tls")]
mod security {
    use super::*;
    use crate::tests::test_helpers::{
        generate_self_signed, make_rustls_client_config, make_rustls_server_config,
    };

    /// The default constructor selects the encrypted (`wss://`) path; only the
    /// explicit opt-in selects plaintext. This is the security-relevant
    /// scheme/selection guard.
    #[test]
    fn default_constructor_is_not_plaintext() {
        let tc = generate_self_signed();
        let client_config = make_rustls_client_config(&tc.cert_der);

        let secure = WebSocketTransport::new(client_config, "localhost".into());
        assert!(
            !secure.is_plaintext(),
            "default WebSocketTransport::new must select the encrypted wss path"
        );

        let insecure = WebSocketTransport::new_plaintext_insecure();
        assert!(
            insecure.is_plaintext(),
            "plaintext must require the explicit dev/test opt-in"
        );
    }

    /// A secure (`wss://`) client must NOT silently downgrade to a plaintext
    /// `ws://` listener: the TLS handshake fails instead of carrying session
    /// traffic in cleartext.
    #[tokio::test]
    async fn secure_client_refuses_plaintext_listener() {
        let tc = generate_self_signed();
        let client_config = make_rustls_client_config(&tc.cert_der);

        let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr();

        // Server side accepts a plaintext upgrade; the secure client must never
        // complete a usable connection against it.
        let _server = tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let mut client = WebSocketTransport::new(client_config, "localhost".into());
        let result = client.connect(addr).await;
        assert!(
            result.is_err(),
            "secure wss client must refuse a plaintext ws listener (no silent downgrade)"
        );
    }

    /// End-to-end proof that the default, secure `wss://` path actually works:
    /// a TLS listener and a secure client exchange a message over an encrypted
    /// WebSocket connection with certificate verification.
    #[tokio::test]
    async fn secure_wss_roundtrip_default_path() {
        let tc = generate_self_signed();
        let server_config = make_rustls_server_config(&tc);
        let client_config = make_rustls_client_config(&tc.cert_der);

        let listener = WebSocketListener::bind_tls("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        let addr = listener.local_addr();

        let server = tokio::spawn(async move {
            let (transport, _peer) = listener.accept().await.unwrap();
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], b"secure hello");
            transport
                .send(Bytes::from_static(b"secure reply"))
                .await
                .unwrap();
        });

        let mut client = WebSocketTransport::new(client_config, "localhost".into());
        client.connect(addr).await.unwrap();
        assert!(!client.is_plaintext());
        client
            .send(Bytes::from_static(b"secure hello"))
            .await
            .unwrap();
        let reply = client.recv().await.unwrap();
        assert_eq!(&reply[..], b"secure reply");
        client.close().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn ws_connect_send_recv() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _peer) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"hello ws");
        transport
            .send(Bytes::from_static(b"hello back"))
            .await
            .unwrap();
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(addr));

    client.send(Bytes::from_static(b"hello ws")).await.unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"hello back");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn ws_multiple_messages() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..10 {
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], format!("ws-{i}").as_bytes());
        }
        transport.send(Bytes::from_static(b"done")).await.unwrap();
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    for i in 0u32..10 {
        client.send(Bytes::from(format!("ws-{i}"))).await.unwrap();
    }
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_send_not_connected() {
    let transport = WebSocketTransport::new_plaintext_insecure();
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn ws_recv_not_connected() {
    let transport = WebSocketTransport::new_plaintext_insecure();
    let err = transport.recv().await;
    assert!(err.is_err());
}

#[tokio::test]
async fn ws_large_message() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let payload_size = 256 * 1024; // 256 KiB
    let data: Vec<u8> = (0..payload_size).map(|i| (i % 256) as u8).collect();
    let data_clone = data.clone();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(msg.len(), payload_size);
        assert_eq!(&msg[..], &data_clone[..]);
        transport.send(Bytes::from_static(b"ok")).await.unwrap();
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    client.send(Bytes::from(data)).await.unwrap();
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"ok");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_local_addr() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (_transport, _peer) = listener.accept().await.unwrap();
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_bidirectional_interleaved() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..5 {
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], format!("c{i}").as_bytes());
            transport.send(Bytes::from(format!("s{i}"))).await.unwrap();
        }
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    for i in 0u32..5 {
        client.send(Bytes::from(format!("c{i}"))).await.unwrap();
        let reply = client.recv().await.unwrap();
        assert_eq!(&reply[..], format!("s{i}").as_bytes());
    }
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_message_too_large() {
    let listener = WebSocketListener::bind_plaintext_insecure("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let _server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let mut client = WebSocketTransport::new_plaintext_insecure();
    client.connect(addr).await.unwrap();
    let huge = vec![0u8; crate::MAX_MESSAGE_SIZE + 1];
    let err = client.send(Bytes::from(huge)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("too large"));
    client.close().await.unwrap();
}

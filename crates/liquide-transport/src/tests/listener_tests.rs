use bytes::Bytes;

use crate::listener::{ListenerConfig, TcpListener};
use crate::tcp::TcpTransport;
use crate::Transport;

// ---------------------------------------------------------------------------
// TCP Listener
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tcp_listener_bind_addr() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();
    assert!(addr.ip().is_loopback());
    assert_ne!(addr.port(), 0);
}

#[tokio::test]
async fn tcp_listener_accept_multiple() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        for _ in 0u32..3 {
            let (transport, peer) = listener.accept().await.unwrap();
            assert!(peer.ip().is_loopback());
            transport
                .send(Bytes::from_static(b"accepted"))
                .await
                .unwrap();
        }
    });

    for _ in 0u32..3 {
        let mut client = TcpTransport::new();
        client.connect(addr).await.unwrap();
        let msg = client.recv().await.unwrap();
        assert_eq!(&msg[..], b"accepted");
        client.close().await.unwrap();
    }

    server.await.unwrap();
}

#[tokio::test]
async fn tcp_listener_accept_raw() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (stream, peer) = listener.accept_raw().await.unwrap();
        assert!(peer.ip().is_loopback());
        // Verify we get a raw TcpStream by wrapping it ourselves
        let transport = TcpTransport::from_stream(stream).unwrap();
        transport
            .send(Bytes::from_static(b"raw_accept"))
            .await
            .unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"raw_accept");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_listener_bind_config() {
    let config = ListenerConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        backlog: 128,
    };
    let listener = TcpListener::bind_config(&config).await.unwrap();
    let addr = listener.local_addr();
    assert_ne!(addr.port(), 0);

    let server = tokio::spawn(async move {
        let (transport, _peer) = listener.accept().await.unwrap();
        transport
            .send(Bytes::from_static(b"config_bind"))
            .await
            .unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"config_bind");
    client.close().await.unwrap();
    server.await.unwrap();
}

// ---------------------------------------------------------------------------
// WebSocket Listener
// ---------------------------------------------------------------------------

#[cfg(feature = "websocket")]
mod ws_listener_tests {
    use bytes::Bytes;

    use crate::listener::ws::WebSocketListener;
    use crate::websocket::WebSocketTransport;
    use crate::Transport;

    #[tokio::test]
    async fn ws_listener_bind_addr() {
        let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr();
        assert!(addr.ip().is_loopback());
        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn ws_listener_accept() {
        let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr();

        let server = tokio::spawn(async move {
            let (transport, peer) = listener.accept().await.unwrap();
            assert!(peer.ip().is_loopback());
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], b"ws_listener_test");
            transport
                .send(Bytes::from_static(b"ws_accepted"))
                .await
                .unwrap();
        });

        let mut client = WebSocketTransport::new();
        client.connect(addr).await.unwrap();
        client
            .send(Bytes::from_static(b"ws_listener_test"))
            .await
            .unwrap();
        let reply = client.recv().await.unwrap();
        assert_eq!(&reply[..], b"ws_accepted");
        client.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn ws_listener_accept_multiple() {
        let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr();

        let server = tokio::spawn(async move {
            for _ in 0u32..3 {
                let (transport, _) = listener.accept().await.unwrap();
                transport
                    .send(Bytes::from_static(b"ws_multi"))
                    .await
                    .unwrap();
            }
        });

        for _ in 0u32..3 {
            let mut client = WebSocketTransport::new();
            client.connect(addr).await.unwrap();
            let msg = client.recv().await.unwrap();
            assert_eq!(&msg[..], b"ws_multi");
            client.close().await.unwrap();
        }

        server.await.unwrap();
    }
}

// ---------------------------------------------------------------------------
// TLS Listener
// ---------------------------------------------------------------------------

#[cfg(feature = "tls")]
mod tls_listener_tests {
    use bytes::Bytes;

    use crate::listener::tls::TlsListener;
    use crate::tls::TlsTcpTransport;
    use crate::Transport;

    use crate::tests::test_helpers::{
        generate_self_signed, make_rustls_client_config, make_rustls_server_config,
    };

    #[tokio::test]
    async fn tls_listener_bind_addr() {
        let tc = generate_self_signed();
        let server_config = make_rustls_server_config(&tc);
        let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        let addr = listener.local_addr();
        assert!(addr.ip().is_loopback());
        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn tls_listener_accept_multiple() {
        let tc = generate_self_signed();
        let server_config = make_rustls_server_config(&tc);
        let client_config = make_rustls_client_config(&tc.cert_der);

        let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        let addr = listener.local_addr();

        let server = tokio::spawn(async move {
            for _ in 0u32..3 {
                let (transport, peer) = listener.accept().await.unwrap();
                assert!(peer.ip().is_loopback());
                transport
                    .send(Bytes::from_static(b"tls_accepted"))
                    .await
                    .unwrap();
            }
        });

        for _ in 0u32..3 {
            let mut client =
                TlsTcpTransport::new(client_config.clone(), "localhost".into());
            client.connect(addr).await.unwrap();
            let msg = client.recv().await.unwrap();
            assert_eq!(&msg[..], b"tls_accepted");
            client.close().await.unwrap();
        }

        server.await.unwrap();
    }
}

// ---------------------------------------------------------------------------
// QUIC Listener
// ---------------------------------------------------------------------------

#[cfg(feature = "quic")]
mod quic_listener_tests {
    use bytes::Bytes;

    use crate::listener::quic::QuicListener;
    use crate::quic::QuicTransport;
    use crate::Transport;

    use crate::tests::test_helpers::{generate_self_signed, make_quinn_server_config};

    #[tokio::test]
    async fn quic_listener_bind_addr() {
        let tc = generate_self_signed();
        let server_config = make_quinn_server_config(&tc);
        let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        let addr = listener.local_addr();
        assert!(addr.ip().is_loopback());
        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn quic_listener_accept() {
        let tc = generate_self_signed();
        let server_config = make_quinn_server_config(&tc);

        let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        let addr = listener.local_addr();

        let server = tokio::spawn(async move {
            let (transport, _peer) = listener.accept().await.unwrap();
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], b"quic_listener_test");
            transport
                .send(Bytes::from_static(b"quic_accepted"))
                .await
                .unwrap();
            // Hold alive until client closes
            let _ = transport.recv().await;
        });

        let mut client =
            QuicTransport::with_client_config(QuicTransport::insecure_client_config());
        client.connect(addr).await.unwrap();
        client
            .send(Bytes::from_static(b"quic_listener_test"))
            .await
            .unwrap();
        let reply = client.recv().await.unwrap();
        assert_eq!(&reply[..], b"quic_accepted");
        client.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn quic_listener_endpoint() {
        let tc = generate_self_signed();
        let server_config = make_quinn_server_config(&tc);
        let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
            .await
            .unwrap();
        // Verify we can access the endpoint
        let ep = listener.endpoint();
        assert!(ep.local_addr().is_ok());
    }
}

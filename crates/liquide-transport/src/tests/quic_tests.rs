use bytes::Bytes;

use crate::listener::quic::QuicListener;
use crate::quic::QuicTransport;
use crate::Transport;

use super::test_helpers::{generate_self_signed, make_quinn_server_config};

#[tokio::test]
async fn quic_connect_send_recv() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _peer) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"hello quic");
        transport
            .send(Bytes::from_static(b"hello back"))
            .await
            .unwrap();
        // Hold transport alive until the client closes its side;
        // dropping early would send CONNECTION_CLOSE before the data
        // is actually transmitted to the peer.
        let _ = transport.recv().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(addr));

    client
        .send(Bytes::from_static(b"hello quic"))
        .await
        .unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"hello back");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn quic_multiple_messages() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..10 {
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], format!("quic-{i}").as_bytes());
        }
        transport.send(Bytes::from_static(b"done")).await.unwrap();
        let _ = transport.recv().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    for i in 0u32..10 {
        client
            .send(Bytes::from(format!("quic-{i}")))
            .await
            .unwrap();
    }
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn quic_from_connection() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = endpoint.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let incoming = endpoint.accept().await.unwrap();
        let connection = incoming.await.unwrap();
        let transport = QuicTransport::from_connection(connection).await.unwrap();
        assert!(transport.is_connected());
        // Read the initial message that triggers the bi stream
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"ping");
        transport
            .send(Bytes::from_static(b"from_connection"))
            .await
            .unwrap();
        let _ = transport.recv().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    // Send initial message — this creates the STREAM frame that the
    // server's accept_bi() needs to see.
    client
        .send(Bytes::from_static(b"ping"))
        .await
        .unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"from_connection");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn quic_send_not_connected() {
    let transport = QuicTransport::new();
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn quic_recv_not_connected() {
    let transport = QuicTransport::new();
    let err = transport.recv().await;
    assert!(err.is_err());
}

#[tokio::test]
async fn quic_large_message() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
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
        let _ = transport.recv().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    client.send(Bytes::from(data)).await.unwrap();
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"ok");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn quic_local_addr() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let _server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
}

#[tokio::test]
async fn quic_bidirectional_interleaved() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..5 {
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], format!("c{i}").as_bytes());
            transport
                .send(Bytes::from(format!("s{i}")))
                .await
                .unwrap();
        }
        // Hold alive until client closes
        let _ = transport.recv().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
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
async fn quic_message_too_large() {
    let tc = generate_self_signed();
    let server_config = make_quinn_server_config(&tc);

    let listener = QuicListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let _server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let mut client = QuicTransport::with_client_config(QuicTransport::insecure_client_config());
    client.connect(addr).await.unwrap();
    let huge = vec![0u8; crate::MAX_MESSAGE_SIZE + 1];
    let err = client.send(Bytes::from(huge)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("too large"));
    client.close().await.unwrap();
}

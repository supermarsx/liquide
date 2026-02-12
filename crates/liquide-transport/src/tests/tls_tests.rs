use bytes::Bytes;
use std::sync::Arc;

use crate::Transport;
use crate::listener::tls::TlsListener;
use crate::tls::TlsTcpTransport;

use super::test_helpers::{
    generate_self_signed, make_rustls_client_config, make_rustls_server_config,
};

#[tokio::test]
async fn tls_connect_send_recv() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _peer) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"hello tls");
        transport
            .send(Bytes::from_static(b"hello back"))
            .await
            .unwrap();
    });

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
    client.connect(addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(addr));

    client.send(Bytes::from_static(b"hello tls")).await.unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"hello back");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn tls_multiple_messages() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..10 {
            let msg = transport.recv().await.unwrap();
            assert_eq!(&msg[..], format!("tls-{i}").as_bytes());
        }
        transport.send(Bytes::from_static(b"done")).await.unwrap();
    });

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
    client.connect(addr).await.unwrap();
    for i in 0u32..10 {
        client.send(Bytes::from(format!("tls-{i}"))).await.unwrap();
    }
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tls_from_server_stream() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (tls_stream, peer) = listener.accept_raw().await.unwrap();
        let transport = TlsTcpTransport::from_server_stream(tls_stream, peer).unwrap();
        transport
            .send(Bytes::from_static(b"via from_server_stream"))
            .await
            .unwrap();
    });

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
    client.connect(addr).await.unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"via from_server_stream");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tls_send_not_connected() {
    let tc = generate_self_signed();
    let client_config = make_rustls_client_config(&tc.cert_der);
    let transport = TlsTcpTransport::new(client_config, "localhost".into());
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn tls_recv_not_connected() {
    let tc = generate_self_signed();
    let client_config = make_rustls_client_config(&tc.cert_der);
    let transport = TlsTcpTransport::new(client_config, "localhost".into());
    let err = transport.recv().await;
    assert!(err.is_err());
}

#[tokio::test]
async fn tls_large_message() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
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

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
    client.connect(addr).await.unwrap();
    client.send(Bytes::from(data)).await.unwrap();
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"ok");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tls_local_addr() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (_transport, _peer) = listener.accept().await.unwrap();
    });

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
    client.connect(addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tls_bidirectional_interleaved() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
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

    let mut client = TlsTcpTransport::new(client_config, "localhost".into());
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
async fn tls_message_too_large() {
    let tc = generate_self_signed();
    let server_config = make_rustls_server_config(&tc);
    let client_config = make_rustls_client_config(&tc.cert_der);

    let listener = TlsListener::bind("127.0.0.1:0".parse().unwrap(), server_config)
        .await
        .unwrap();
    let addr = listener.local_addr();

    let _server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let mut client = TlsTcpTransport::new(Arc::clone(&client_config), "localhost".into());
    client.connect(addr).await.unwrap();
    let huge = vec![0u8; crate::MAX_MESSAGE_SIZE + 1];
    let err = client.send(Bytes::from(huge)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("too large"));
    client.close().await.unwrap();
}

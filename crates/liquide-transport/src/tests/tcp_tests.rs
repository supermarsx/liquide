use bytes::Bytes;

use crate::tcp::TcpTransport;
use crate::listener::TcpListener;
use crate::Transport;

#[tokio::test]
async fn tcp_connect_send_recv() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _peer) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"hello from client");
        transport
            .send(Bytes::from_static(b"hello from server"))
            .await
            .unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(addr));

    client
        .send(Bytes::from_static(b"hello from client"))
        .await
        .unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"hello from server");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_multiple_messages() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0u32..10 {
            let msg = transport.recv().await.unwrap();
            let expected = format!("msg-{i}");
            assert_eq!(&msg[..], expected.as_bytes());
        }
        transport.send(Bytes::from_static(b"done")).await.unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    for i in 0u32..10 {
        let payload = format!("msg-{i}");
        client.send(Bytes::from(payload)).await.unwrap();
    }
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_from_stream() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (stream, _peer) = listener.accept_raw().await.unwrap();
        let transport = TcpTransport::from_stream(stream).unwrap();
        transport
            .send(Bytes::from_static(b"via from_stream"))
            .await
            .unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"via from_stream");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_send_not_connected() {
    let transport = TcpTransport::new();
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn tcp_recv_not_connected() {
    let transport = TcpTransport::new();
    let err = transport.recv().await;
    assert!(err.is_err());
}

#[tokio::test]
async fn tcp_large_message() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
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

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    client.send(Bytes::from(data)).await.unwrap();
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"ok");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_local_addr() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (_transport, _peer) = listener.accept().await.unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_bidirectional_interleaved() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
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
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    for i in 0u32..5 {
        client.send(Bytes::from(format!("c{i}"))).await.unwrap();
        let reply = client.recv().await.unwrap();
        assert_eq!(&reply[..], format!("s{i}").as_bytes());
    }
    client.close().await.unwrap();
    server.await.unwrap();
}

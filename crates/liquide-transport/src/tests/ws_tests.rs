use bytes::Bytes;

use crate::listener::ws::WebSocketListener;
use crate::websocket::WebSocketTransport;
use crate::Transport;

#[tokio::test]
async fn ws_connect_send_recv() {
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
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

    let mut client = WebSocketTransport::new();
    client.connect(addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(addr));

    client
        .send(Bytes::from_static(b"hello ws"))
        .await
        .unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"hello back");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn ws_multiple_messages() {
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
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

    let mut client = WebSocketTransport::new();
    client.connect(addr).await.unwrap();
    for i in 0u32..10 {
        client
            .send(Bytes::from(format!("ws-{i}")))
            .await
            .unwrap();
    }
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_send_not_connected() {
    let transport = WebSocketTransport::new();
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn ws_recv_not_connected() {
    let transport = WebSocketTransport::new();
    let err = transport.recv().await;
    assert!(err.is_err());
}

#[tokio::test]
async fn ws_large_message() {
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
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

    let mut client = WebSocketTransport::new();
    client.connect(addr).await.unwrap();
    client.send(Bytes::from(data)).await.unwrap();
    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"ok");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_local_addr() {
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (_transport, _peer) = listener.accept().await.unwrap();
    });

    let mut client = WebSocketTransport::new();
    client.connect(addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ws_bidirectional_interleaved() {
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
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

    let mut client = WebSocketTransport::new();
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
    let listener = WebSocketListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let _server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let mut client = WebSocketTransport::new();
    client.connect(addr).await.unwrap();
    let huge = vec![0u8; crate::MAX_MESSAGE_SIZE + 1];
    let err = client.send(Bytes::from(huge)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("too large"));
    client.close().await.unwrap();
}

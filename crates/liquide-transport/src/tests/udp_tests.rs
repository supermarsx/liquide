use bytes::Bytes;
use tokio::net::UdpSocket;

use crate::Transport;
use crate::udp::UdpTransport;

#[tokio::test]
async fn udp_connect_send_recv() {
    // Bind a "server" UDP socket.
    let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_sock.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        let (n, peer) = server_sock.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"udp hello");
        server_sock.send_to(b"udp reply", peer).await.unwrap();
    });

    let mut client = UdpTransport::new();
    client.connect(server_addr).await.unwrap();
    assert!(client.is_connected());
    assert_eq!(client.peer_addr(), Some(server_addr));

    client.send(Bytes::from_static(b"udp hello")).await.unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"udp reply");

    client.close().await.unwrap();
    assert!(!client.is_connected());
    server.await.unwrap();
}

#[tokio::test]
async fn udp_multiple_datagrams() {
    let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_sock.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        for i in 0u32..5 {
            let (n, _peer) = server_sock.recv_from(&mut buf).await.unwrap();
            let expected = format!("dgram-{i}");
            assert_eq!(&buf[..n], expected.as_bytes());
        }
    });

    let mut client = UdpTransport::new();
    client.connect(server_addr).await.unwrap();
    for i in 0u32..5 {
        let payload = format!("dgram-{i}");
        client.send(Bytes::from(payload)).await.unwrap();
    }
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn udp_send_not_connected() {
    let transport = UdpTransport::new();
    let err = transport.send(Bytes::from_static(b"nope")).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("not connected"));
}

#[tokio::test]
async fn udp_from_socket() {
    let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_sock.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let mut buf = [0u8; 65536];
        let (n, _) = server_sock.recv_from(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"from_socket");
    });

    let client_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client_sock.connect(server_addr).await.unwrap();
    let transport = UdpTransport::from_socket(client_sock, server_addr).unwrap();
    assert!(transport.is_connected());
    transport
        .send(Bytes::from_static(b"from_socket"))
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn udp_local_addr() {
    let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_sock.local_addr().unwrap();

    let mut client = UdpTransport::new();
    client.connect(server_addr).await.unwrap();
    assert!(client.local_addr().is_some());
    client.close().await.unwrap();
}

#[tokio::test]
async fn udp_message_too_large() {
    let server_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_sock.local_addr().unwrap();

    let mut client = UdpTransport::new();
    client.connect(server_addr).await.unwrap();

    let huge = vec![0u8; 70_000]; // exceeds 65507
    let err = client.send(Bytes::from(huge)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("too large"));
    client.close().await.unwrap();
}

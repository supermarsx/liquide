use bytes::Bytes;
use liquide_protocol::{ChannelId, FrameFlags, FrameHeader};

use crate::connection::Connection;
use crate::listener::TcpListener;
use crate::tcp::TcpTransport;

#[tokio::test]
async fn connection_send_recv_frame() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Connection::new(transport);
        let (header, payload) = conn.recv_frame().await.unwrap();
        assert_eq!(header.channel, ChannelId::VIDEO);
        assert_eq!(header.sequence, 1);
        assert_eq!(&payload[..], b"tile data");

        let reply_hdr = FrameHeader::new(ChannelId::CONTROL, 2, 0, 0, FrameFlags::RELIABLE, 2);
        conn.send_frame(&reply_hdr, b"ok").await.unwrap();
    });

    let conn = Connection::connect(TcpTransport::new(), addr)
        .await
        .unwrap();
    assert!(conn.is_connected());

    let hdr = FrameHeader::new(ChannelId::VIDEO, 1, 0, 0, 0, 9);
    conn.send_frame(&hdr, b"tile data").await.unwrap();

    let (reply_hdr, reply_payload) = conn.recv_frame().await.unwrap();
    assert_eq!(reply_hdr.channel, ChannelId::CONTROL);
    assert_eq!(reply_hdr.sequence, 2);
    assert!(reply_hdr.is_reliable());
    assert_eq!(&reply_payload[..], b"ok");

    server.await.unwrap();
}

#[tokio::test]
async fn connection_stats_tracking() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Connection::new(transport);
        let _ = conn.recv_raw().await.unwrap();
        conn.send_raw(Bytes::from_static(b"pong")).await.unwrap();
    });

    let conn = Connection::connect(TcpTransport::new(), addr)
        .await
        .unwrap();
    conn.send_raw(Bytes::from_static(b"ping")).await.unwrap();
    let _ = conn.recv_raw().await.unwrap();

    let stats = conn.stats();
    assert_eq!(stats.messages_sent(), 1);
    assert_eq!(stats.messages_recv(), 1);
    assert!(stats.bytes_sent() > 0);
    assert!(stats.bytes_recv() > 0);

    server.await.unwrap();
}

#[tokio::test]
async fn connection_send_recv_raw() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Connection::new(transport);
        let data = conn.recv_raw().await.unwrap();
        assert_eq!(&data[..], b"raw bytes");
        conn.send_raw(Bytes::from_static(b"raw reply"))
            .await
            .unwrap();
    });

    let conn = Connection::connect(TcpTransport::new(), addr)
        .await
        .unwrap();
    conn.send_raw(Bytes::from_static(b"raw bytes"))
        .await
        .unwrap();
    let reply = conn.recv_raw().await.unwrap();
    assert_eq!(&reply[..], b"raw reply");
    server.await.unwrap();
}

#[tokio::test]
async fn connection_shared_stats() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Connection::new(transport);
        // Just accept and let connection drop.
        let _ = conn.recv_raw().await;
    });

    let conn = Connection::connect(TcpTransport::new(), addr)
        .await
        .unwrap();
    let shared = conn.stats_shared();
    conn.send_raw(Bytes::from_static(b"test")).await.unwrap();
    assert_eq!(shared.messages_sent(), 1);

    server.await.unwrap();
}

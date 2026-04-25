use std::time::Duration;

use bytes::Bytes;

use crate::Transport;
use crate::listener::TcpListener;
use crate::tcp::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE, TcpTransport, TcpTuning};

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
            transport.send(Bytes::from(format!("s{i}"))).await.unwrap();
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

// ---------------------------------------------------------------------------
// TCP Tuning Config Tests
// ---------------------------------------------------------------------------

#[test]
fn tuning_interactive_defaults() {
    let tuning = TcpTuning::interactive();
    assert!(tuning.nodelay);
    assert!(tuning.keepalive);
    assert!(tuning.auto_buffer);
    assert_eq!(tuning.keepalive_idle, Duration::from_secs(30));
    assert_eq!(tuning.keepalive_interval, Duration::from_secs(10));
    assert!(tuning.send_buffer.is_none());
    assert!(tuning.recv_buffer.is_none());
}

#[test]
fn tuning_bulk_disables_nodelay() {
    let tuning = TcpTuning::bulk();
    assert!(!tuning.nodelay);
    assert!(tuning.keepalive);
}

#[test]
fn tuning_default_is_interactive() {
    let def = TcpTuning::default();
    let interactive = TcpTuning::interactive();
    assert_eq!(def.nodelay, interactive.nodelay);
    assert_eq!(def.keepalive, interactive.keepalive);
    assert_eq!(def.keepalive_idle, interactive.keepalive_idle);
}

// ---------------------------------------------------------------------------
// Auto Buffer Sizing
// ---------------------------------------------------------------------------

#[test]
fn auto_buffer_size_typical() {
    // 50ms RTT, 100 Mbps = 12.5 MB/s
    let rtt = Duration::from_millis(50);
    let bw = 12_500_000.0;
    let size = TcpTuning::auto_buffer_size(rtt, bw);
    // BDP = 0.05 * 12_500_000 = 625_000
    // target = 625_000 * 2 = 1_250_000
    assert_eq!(size, 1_250_000);
}

#[test]
fn auto_buffer_size_clamps_to_min() {
    let rtt = Duration::from_millis(1);
    let bw = 125_000.0;
    let size = TcpTuning::auto_buffer_size(rtt, bw);
    assert_eq!(size, MIN_BUFFER_SIZE);
}

#[test]
fn auto_buffer_size_clamps_to_max() {
    let rtt = Duration::from_millis(200);
    let bw = 125_000_000.0;
    let size = TcpTuning::auto_buffer_size(rtt, bw);
    assert_eq!(size, MAX_BUFFER_SIZE);
}

#[test]
fn auto_buffer_constants() {
    assert_eq!(MIN_BUFFER_SIZE, 256 * 1024);
    assert_eq!(MAX_BUFFER_SIZE, 4 * 1024 * 1024);
}

// ---------------------------------------------------------------------------
// With Tuning Constructor
// ---------------------------------------------------------------------------

#[test]
fn with_tuning_constructor() {
    let tuning = TcpTuning::bulk();
    let transport = TcpTransport::with_tuning(tuning.clone());
    assert_eq!(transport.tuning().nodelay, false);
    assert!(!transport.is_connected());
}

#[test]
fn set_tuning() {
    let mut transport = TcpTransport::new();
    assert!(transport.tuning().nodelay);

    transport.set_tuning(TcpTuning::bulk());
    assert!(!transport.tuning().nodelay);
}

// ---------------------------------------------------------------------------
// Tuned Connection (integration)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tcp_connect_with_tuning() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let msg = transport.recv().await.unwrap();
        assert_eq!(&msg[..], b"tuned");
        transport.send(Bytes::from_static(b"ok")).await.unwrap();
    });

    let tuning = TcpTuning {
        nodelay: true,
        keepalive: true,
        keepalive_idle: Duration::from_secs(15),
        keepalive_interval: Duration::from_secs(5),
        auto_buffer: false,
        send_buffer: Some(512 * 1024),
        recv_buffer: Some(512 * 1024),
    };
    let mut client = TcpTransport::with_tuning(tuning);
    client.connect(addr).await.unwrap();

    client.send(Bytes::from_static(b"tuned")).await.unwrap();
    let reply = client.recv().await.unwrap();
    assert_eq!(&reply[..], b"ok");

    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_from_stream_tuned() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let tuning = TcpTuning {
        nodelay: true,
        keepalive: true,
        keepalive_idle: Duration::from_secs(20),
        keepalive_interval: Duration::from_secs(5),
        auto_buffer: false,
        send_buffer: None,
        recv_buffer: None,
    };

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept_raw().await.unwrap();
        let transport = TcpTransport::from_stream_tuned(stream, tuning).unwrap();
        transport
            .send(Bytes::from_static(b"tuned-stream"))
            .await
            .unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    let msg = client.recv().await.unwrap();
    assert_eq!(&msg[..], b"tuned-stream");
    client.close().await.unwrap();
    server.await.unwrap();
}

// ---------------------------------------------------------------------------
// Batch Send
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tcp_send_batch() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0..3 {
            let msg = transport.recv().await.unwrap();
            let expected = format!("batch-{i}");
            assert_eq!(&msg[..], expected.as_bytes());
        }
        transport.send(Bytes::from_static(b"done")).await.unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();

    let payloads = vec![
        Bytes::from_static(b"batch-0"),
        Bytes::from_static(b"batch-1"),
        Bytes::from_static(b"batch-2"),
    ];
    client.send_batch(&payloads).await.unwrap();

    let ack = client.recv().await.unwrap();
    assert_eq!(&ack[..], b"done");
    client.close().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn tcp_send_batch_not_connected() {
    let transport = TcpTransport::new();
    let payloads = vec![Bytes::from_static(b"data")];
    let err = transport.send_batch(&payloads).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn tcp_send_batch_empty() {
    let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = listener.local_addr();

    let server = tokio::spawn(async move {
        let (_transport, _) = listener.accept().await.unwrap();
    });

    let mut client = TcpTransport::new();
    client.connect(addr).await.unwrap();
    client.send_batch(&[]).await.unwrap();
    client.close().await.unwrap();
    server.await.unwrap();
}

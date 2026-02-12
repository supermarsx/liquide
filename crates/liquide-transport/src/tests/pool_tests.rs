use bytes::Bytes;

use crate::listener::TcpListener;
use crate::pool::Pool;
use crate::tcp::TcpTransport;
use crate::Transport;

#[test]
fn pool_starts_empty() {
    let pool: Pool<TcpTransport> = Pool::new();
    assert!(pool.is_empty());
    assert_eq!(pool.len(), 0);
}

#[tokio::test]
async fn pool_send_not_connected() {
    let pool: Pool<TcpTransport> = Pool::new();
    let err = pool.send(Bytes::from_static(b"test")).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn pool_round_robin() {
    // Create two server listeners.
    let l1 = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let l2 = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let a1 = l1.local_addr();
    let a2 = l2.local_addr();

    // Server tasks: each accepts one connection and reads one message.
    let s1 = tokio::spawn(async move {
        let (t, _) = l1.accept().await.unwrap();
        let msg = t.recv().await.unwrap();
        String::from_utf8(msg.to_vec()).unwrap()
    });
    let s2 = tokio::spawn(async move {
        let (t, _) = l2.accept().await.unwrap();
        let msg = t.recv().await.unwrap();
        String::from_utf8(msg.to_vec()).unwrap()
    });

    // Create two client transports and pool them.
    let mut t1 = TcpTransport::new();
    t1.connect(a1).await.unwrap();
    let mut t2 = TcpTransport::new();
    t2.connect(a2).await.unwrap();

    let mut pool = Pool::new();
    pool.push(t1);
    pool.push(t2);
    assert_eq!(pool.len(), 2);

    // First send goes to t1 (index 0), second to t2 (index 1).
    pool.send(Bytes::from_static(b"for-server-1")).await.unwrap();
    pool.send(Bytes::from_static(b"for-server-2")).await.unwrap();

    let msg1 = s1.await.unwrap();
    let msg2 = s2.await.unwrap();
    assert_eq!(msg1, "for-server-1");
    assert_eq!(msg2, "for-server-2");

    pool.close_all().await.unwrap();
    assert!(pool.is_empty());
}

#[tokio::test]
async fn pool_peers() {
    let l1 = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let a1 = l1.local_addr();

    let _server = tokio::spawn(async move {
        let _ = l1.accept().await;
    });

    let mut t1 = TcpTransport::new();
    t1.connect(a1).await.unwrap();

    let mut pool = Pool::new();
    pool.push(t1);

    let peers = pool.peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0], Some(a1));

    pool.close_all().await.unwrap();
}

#[tokio::test]
async fn pool_drain() {
    let l = TcpListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let addr = l.local_addr();

    let _server = tokio::spawn(async move {
        let _ = l.accept().await;
    });

    let mut t = TcpTransport::new();
    t.connect(addr).await.unwrap();

    let mut pool = Pool::new();
    pool.push(t);
    assert_eq!(pool.len(), 1);

    let drained = pool.drain();
    assert_eq!(drained.len(), 1);
    assert!(pool.is_empty());
}

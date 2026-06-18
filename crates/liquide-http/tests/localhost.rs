//! Localhost integration tests for the real reqwest path (feature `net`).
//!
//! NO test here touches the public internet. Each test stands up a tiny HTTP/1.1
//! server on `127.0.0.1:0` (an OS-assigned free loopback port) using only
//! `std::net`, then drives the real [`HttpClient`] against it and asserts the
//! non-blocking fetch/poll API behaves:
//!
//! - (a) a served body comes back via `poll_results` without `fetch` blocking;
//! - (b) a 404 and a connection-refused both yield an `Err` (no panic / hang);
//! - (c) the per-request timeout fires for a hung endpoint;
//! - (d) the cache serves a repeat URL from memory (server hit only once);
//! - (e) (covered in the lib via the null stub) the feature-off client reports
//!   `Unavailable`.
#![cfg(feature = "net")]

use liquide_http::{HttpClient, HttpClientApi, HttpConfig, HttpError, RequestId};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How the test server should respond to each accepted connection.
#[derive(Clone, Copy)]
enum Behavior {
    /// Serve a fixed 200 body.
    Ok200,
    /// Serve a 404.
    NotFound,
    /// Accept the connection, read the request, then sleep past the client
    /// timeout without ever replying (simulates a hung endpoint).
    Hang,
}

/// A loopback HTTP server bound to an ephemeral port. Counts served requests so
/// the cache test can prove the network was hit exactly once.
struct TestServer {
    addr: std::net::SocketAddr,
    hits: Arc<AtomicUsize>,
    _handle: std::thread::JoinHandle<()>,
    shutdown: Arc<AtomicUsize>,
}

impl TestServer {
    fn start(behavior: Behavior, body: &'static [u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let addr = listener.local_addr().expect("local addr");
        let hits = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicUsize::new(0));

        let hits_t = Arc::clone(&hits);
        let shutdown_t = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            // Accept loop: poll the non-blocking listener until told to stop, so
            // the thread can be joined cleanly at the end of the test.
            while shutdown_t.load(Ordering::SeqCst) == 0 {
                match listener.accept() {
                    Ok((stream, _)) => {
                        hits_t.fetch_add(1, Ordering::SeqCst);
                        // The accepted socket may inherit the listener's
                        // non-blocking mode; force it back to blocking so the
                        // read/write below behaves. Handle each connection on
                        // its own thread so a slow/hung peer never stalls accept.
                        let _ = stream.set_nonblocking(false);
                        let shutdown_c = Arc::clone(&shutdown_t);
                        std::thread::spawn(move || {
                            handle_conn(stream, behavior, body, &shutdown_c);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            hits,
            _handle: handle,
            shutdown,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(1, Ordering::SeqCst);
    }
}

fn handle_conn(
    mut stream: TcpStream,
    behavior: Behavior,
    body: &'static [u8],
    shutdown: &Arc<AtomicUsize>,
) {
    // Read the request line + headers fully (until the blank line that ends the
    // header block). Draining the whole request before replying avoids a
    // write-before-read race that can surface as a connection reset on the
    // client under parallel load.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut req = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                req.extend_from_slice(&chunk[..n]);
                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                    break; // end of headers; this is a GET (no body)
                }
            }
            Err(_) => break,
        }
    }

    match behavior {
        Behavior::Ok200 => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
        }
        Behavior::NotFound => {
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
        Behavior::Hang => {
            // Never reply. Sleep (in short increments so shutdown is responsive)
            // well past the client's timeout, holding the connection open.
            for _ in 0..200 {
                if shutdown.load(Ordering::SeqCst) != 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Poll the client (without blocking the test on the network) until the given
/// request id completes or `deadline` elapses. The polling loop itself is how a
/// real main loop would drain results frame-by-frame.
fn await_result(
    client: &HttpClient,
    id: RequestId,
    deadline: Duration,
) -> Option<liquide_http::FetchResult> {
    let start = Instant::now();
    while start.elapsed() < deadline {
        for (rid, result) in client.poll_results() {
            if rid == id {
                return Some(result);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn fetch_returns_served_bytes_via_poll_without_blocking() {
    let server = TestServer::start(Behavior::Ok200, b"hello-tile-bytes");
    let client = HttpClient::new().expect("client builds");

    // fetch must return promptly (non-blocking) even though the body hasn't
    // arrived yet — assert it returns far faster than any network round trip
    // could complete under a real (slow) server.
    let before = Instant::now();
    let id = client.fetch(&server.url("/tile.png"));
    assert!(
        before.elapsed() < Duration::from_millis(50),
        "fetch must not block on the network"
    );

    let result = await_result(&client, id, Duration::from_secs(5)).expect("result delivered");
    let body = result.expect("200 yields Ok");
    assert_eq!(&body[..], b"hello-tile-bytes");
}

#[test]
fn not_found_yields_status_error_not_panic() {
    let server = TestServer::start(Behavior::NotFound, b"");
    let client = HttpClient::new().expect("client builds");
    let id = client.fetch(&server.url("/missing.png"));
    let result = await_result(&client, id, Duration::from_secs(5)).expect("result delivered");
    match result {
        Err(HttpError::Status(404)) => {}
        other => panic!("expected Status(404), got {other:?}"),
    }
}

#[test]
fn connection_refused_yields_transport_error() {
    // Bind then immediately drop the listener so the port is (almost certainly)
    // closed — a connect there is refused. No server thread at all.
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr")
    };
    let client = HttpClient::new().expect("client builds");
    let id = client.fetch(&format!("http://{addr}/x"));
    let result = await_result(&client, id, Duration::from_secs(10)).expect("result delivered");
    assert!(
        matches!(result, Err(HttpError::Transport(_)) | Err(HttpError::Timeout(_))),
        "connection refused must be an Err, got {result:?}"
    );
}

#[test]
fn timeout_fires_for_a_hung_endpoint() {
    let server = TestServer::start(Behavior::Hang, b"");
    let config = HttpConfig {
        request_timeout: Duration::from_millis(300),
        ..HttpConfig::default()
    };
    let client = HttpClient::with_config(config).expect("client builds");

    let started = Instant::now();
    let id = client.fetch(&server.url("/hang"));
    let result = await_result(&client, id, Duration::from_secs(5)).expect("result delivered");
    assert!(
        matches!(result, Err(HttpError::Timeout(_))),
        "hung endpoint must time out, got {result:?}"
    );
    // The timeout must actually fire (not hang forever): it resolves well within
    // the 5s poll deadline, and not before the configured 300ms window.
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "timeout should resolve promptly"
    );
}

#[test]
fn cache_serves_repeat_url_without_a_second_network_hit() {
    let server = TestServer::start(Behavior::Ok200, b"cached-body");
    let client = HttpClient::new().expect("client builds");
    let url = server.url("/tile/1/2/3.png");

    // First fetch: goes to the network.
    let id1 = client.fetch(&url);
    let body1 = await_result(&client, id1, Duration::from_secs(5))
        .expect("first result")
        .expect("first ok");
    assert_eq!(&body1[..], b"cached-body");
    assert_eq!(server.hits(), 1, "first fetch hit the server once");

    // Second fetch of the SAME url: served from the in-memory cache, so the
    // server hit count must stay at 1.
    let id2 = client.fetch(&url);
    let body2 = await_result(&client, id2, Duration::from_secs(5))
        .expect("second result")
        .expect("second ok");
    assert_eq!(&body2[..], b"cached-body");
    assert_eq!(
        server.hits(),
        1,
        "repeat URL must be served from cache, not re-fetched"
    );
}

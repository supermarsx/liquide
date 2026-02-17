//! TCP transport backend with length-prefixed message framing.

use std::io::IoSlice;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::codec;

// ---------------------------------------------------------------------------
// TCP Tuning Config
// ---------------------------------------------------------------------------

/// Minimum socket buffer size (256 KiB).
pub const MIN_BUFFER_SIZE: usize = 256 * 1024;

/// Maximum socket buffer size (4 MiB).
pub const MAX_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Default keepalive idle time.
const DEFAULT_KEEPALIVE_IDLE: Duration = Duration::from_secs(30);

/// Default keepalive probe interval.
const DEFAULT_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Configuration for TCP socket tuning.
#[derive(Debug, Clone)]
pub struct TcpTuning {
    /// Enable TCP_NODELAY (disable Nagle's algorithm). Default: true.
    pub nodelay: bool,
    /// Enable TCP keepalive. Default: true.
    pub keepalive: bool,
    /// Time a connection must be idle before sending keepalive probes.
    pub keepalive_idle: Duration,
    /// Interval between keepalive probes.
    pub keepalive_interval: Duration,
    /// Whether to auto-size socket buffers based on BDP. Default: true.
    pub auto_buffer: bool,
    /// Send buffer size override (bytes). None = OS default or auto-tuned.
    pub send_buffer: Option<usize>,
    /// Receive buffer size override (bytes). None = OS default or auto-tuned.
    pub recv_buffer: Option<usize>,
}

impl TcpTuning {
    /// Default tuning for interactive (low-latency) traffic.
    #[must_use]
    pub fn interactive() -> Self {
        Self {
            nodelay: true,
            keepalive: true,
            keepalive_idle: DEFAULT_KEEPALIVE_IDLE,
            keepalive_interval: DEFAULT_KEEPALIVE_INTERVAL,
            auto_buffer: true,
            send_buffer: None,
            recv_buffer: None,
        }
    }

    /// Tuning for bulk transfer (higher throughput, more buffering).
    #[must_use]
    pub fn bulk() -> Self {
        Self {
            nodelay: false, // allow Nagle for better coalescing
            keepalive: true,
            keepalive_idle: DEFAULT_KEEPALIVE_IDLE,
            keepalive_interval: DEFAULT_KEEPALIVE_INTERVAL,
            auto_buffer: true,
            send_buffer: None,
            recv_buffer: None,
        }
    }

    /// Compute optimal socket buffer size from network conditions.
    ///
    /// Uses Bandwidth-Delay Product (BDP): `rtt_seconds * bandwidth_bytes_per_sec`.
    /// The buffer is set to `BDP * 2` (double-buffering), clamped to
    /// `[MIN_BUFFER_SIZE, MAX_BUFFER_SIZE]`.
    #[must_use]
    pub fn auto_buffer_size(rtt: Duration, bandwidth_bytes_per_sec: f64) -> usize {
        let bdp = rtt.as_secs_f64() * bandwidth_bytes_per_sec;
        let target = (bdp * 2.0) as usize;
        target.clamp(MIN_BUFFER_SIZE, MAX_BUFFER_SIZE)
    }
}

impl Default for TcpTuning {
    fn default() -> Self {
        Self::interactive()
    }
}

// ---------------------------------------------------------------------------
// TCP Transport
// ---------------------------------------------------------------------------

/// Plain TCP transport with automatic length-prefix framing.
///
/// Each call to [`send`](crate::Transport::send) writes a 4-byte little-endian
/// length followed by the payload.  [`recv`](crate::Transport::recv) reads the
/// length prefix and then exactly that many bytes.
pub struct TcpTransport {
    remote: Option<SocketAddr>,
    local: Option<SocketAddr>,
    reader: Option<Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>>,
    writer: Option<Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>>,
    tuning: TcpTuning,
}

impl TcpTransport {
    /// Create a new, unconnected TCP transport.
    #[must_use]
    pub fn new() -> Self {
        Self {
            remote: None,
            local: None,
            reader: None,
            writer: None,
            tuning: TcpTuning::default(),
        }
    }

    /// Create with the given tuning configuration.
    #[must_use]
    pub fn with_tuning(tuning: TcpTuning) -> Self {
        Self {
            remote: None,
            local: None,
            reader: None,
            writer: None,
            tuning,
        }
    }

    /// Wrap an already-connected [`TcpStream`].
    pub fn from_stream(stream: TcpStream) -> crate::Result<Self> {
        let peer = stream.peer_addr()?;
        let local = stream.local_addr().ok();
        stream.set_nodelay(true)?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            remote: Some(peer),
            local,
            reader: Some(Arc::new(Mutex::new(reader))),
            writer: Some(Arc::new(Mutex::new(writer))),
            tuning: TcpTuning::default(),
        })
    }

    /// Get the current tuning configuration.
    #[must_use]
    pub fn tuning(&self) -> &TcpTuning {
        &self.tuning
    }

    /// Set the tuning configuration.
    ///
    /// If the transport is already connected, call [`apply_tuning`] afterward
    /// to apply the settings to the live socket.
    pub fn set_tuning(&mut self, tuning: TcpTuning) {
        self.tuning = tuning;
    }

    /// Apply the current tuning to a raw `TcpStream` via socket2.
    ///
    /// This is called automatically during `connect` and `from_stream_tuned`,
    /// but can also be called manually if tuning is changed after connect.
    fn apply_tuning_to_stream(stream: &TcpStream, tuning: &TcpTuning) -> crate::Result<()> {
        stream.set_nodelay(tuning.nodelay)?;

        let sock_ref = socket2::SockRef::from(stream);

        if tuning.keepalive {
            let ka = socket2::TcpKeepalive::new()
                .with_time(tuning.keepalive_idle)
                .with_interval(tuning.keepalive_interval);
            sock_ref.set_tcp_keepalive(&ka)?;
        }

        if let Some(size) = tuning.send_buffer {
            sock_ref.set_send_buffer_size(size)?;
        }
        if let Some(size) = tuning.recv_buffer {
            sock_ref.set_recv_buffer_size(size)?;
        }

        Ok(())
    }

    /// Wrap an already-connected `TcpStream`, applying the given tuning.
    pub fn from_stream_tuned(stream: TcpStream, tuning: TcpTuning) -> crate::Result<Self> {
        let peer = stream.peer_addr()?;
        let local = stream.local_addr().ok();
        Self::apply_tuning_to_stream(&stream, &tuning)?;
        let (reader, writer) = stream.into_split();
        Ok(Self {
            remote: Some(peer),
            local,
            reader: Some(Arc::new(Mutex::new(reader))),
            writer: Some(Arc::new(Mutex::new(writer))),
            tuning,
        })
    }

    /// Update socket buffers for current network conditions.
    ///
    /// Computes ideal buffer sizes from RTT and bandwidth, then applies them.
    /// This requires the transport to be connected.
    pub fn auto_tune_buffers(
        &self,
        stream: &TcpStream,
        rtt: Duration,
        bandwidth: f64,
    ) -> crate::Result<()> {
        let size = TcpTuning::auto_buffer_size(rtt, bandwidth);
        let sock_ref = socket2::SockRef::from(stream);
        sock_ref.set_send_buffer_size(size)?;
        sock_ref.set_recv_buffer_size(size)?;
        Ok(())
    }

    /// Send multiple payloads in a single coalesced write using vectored I/O.
    ///
    /// Each payload is length-prefixed, and all are written in one syscall
    /// (via `write_vectored`), reducing the number of small packets sent.
    pub async fn send_batch(&self, payloads: &[Bytes]) -> crate::Result<()> {
        let writer = self
            .writer
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;

        // Pre-encode each payload as length + data
        let mut buffers: Vec<Vec<u8>> = Vec::with_capacity(payloads.len());
        for payload in payloads {
            if payload.len() > crate::MAX_MESSAGE_SIZE {
                return Err(crate::TransportError::MessageTooLarge {
                    size: payload.len(),
                    max: crate::MAX_MESSAGE_SIZE,
                });
            }
            let len = (payload.len() as u32).to_le_bytes();
            let mut buf = Vec::with_capacity(4 + payload.len());
            buf.extend_from_slice(&len);
            buf.extend_from_slice(payload);
            buffers.push(buf);
        }

        let mut w = writer.lock().await;
        
        // Track position: which buffer and offset within that buffer
        let mut buf_idx = 0;
        let mut byte_offset = 0;

        while buf_idx < buffers.len() {
            // Build IoSlice array from current position
            let mut slices: Vec<IoSlice<'_>> = Vec::with_capacity(buffers.len() - buf_idx);
            
            // First slice starts at byte_offset
            if buf_idx < buffers.len() {
                slices.push(IoSlice::new(&buffers[buf_idx][byte_offset..]));
            }
            // Remaining buffers start at 0
            for buf in &buffers[buf_idx + 1..] {
                slices.push(IoSlice::new(buf));
            }
            
            if slices.is_empty() {
                break;
            }

            let n = w.write_vectored(&slices).await?;
            if n == 0 {
                return Err(crate::TransportError::ConnectionReset);
            }

            // Advance position by n bytes
            let mut remaining = n;
            while remaining > 0 && buf_idx < buffers.len() {
                let buf_remaining = buffers[buf_idx].len() - byte_offset;
                if remaining >= buf_remaining {
                    // Fully wrote this buffer, move to next
                    remaining -= buf_remaining;
                    buf_idx += 1;
                    byte_offset = 0;
                } else {
                    // Partially wrote this buffer
                    byte_offset += remaining;
                    remaining = 0;
                }
            }
        }

        w.flush().await?;
        Ok(())
    }
}

impl Default for TcpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::Transport for TcpTransport {
    async fn connect(&mut self, addr: SocketAddr) -> crate::Result<()> {
        let stream = TcpStream::connect(addr).await?;
        Self::apply_tuning_to_stream(&stream, &self.tuning)?;
        self.local = stream.local_addr().ok();
        let (reader, writer) = stream.into_split();
        self.remote = Some(addr);
        self.reader = Some(Arc::new(Mutex::new(reader)));
        self.writer = Some(Arc::new(Mutex::new(writer)));
        tracing::debug!(%addr, "TCP connected");
        Ok(())
    }

    async fn send(&self, data: Bytes) -> crate::Result<()> {
        let writer = self
            .writer
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        if data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(crate::TransportError::MessageTooLarge {
                size: data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }
        let mut w = writer.lock().await;
        codec::write_msg(&mut *w, &data).await?;
        Ok(())
    }

    async fn recv(&self) -> crate::Result<Bytes> {
        let reader = self
            .reader
            .as_ref()
            .ok_or(crate::TransportError::NotConnected)?;
        let mut r = reader.lock().await;
        codec::read_msg(&mut *r, crate::MAX_MESSAGE_SIZE).await
    }

    async fn close(&mut self) -> crate::Result<()> {
        // Attempt a graceful shutdown on the write half.
        if let Some(writer) = self.writer.take() {
            // Use lock-based shutdown to work even with other references
            let mut w = writer.lock().await;
            let _ = w.shutdown().await;
        }
        self.reader = None;
        self.remote = None;
        self.local = None;
        Ok(())
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        self.remote
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.local
    }
}

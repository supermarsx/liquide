//! Transport I/O bridge with priority-scheduled writer and demux reader.
//!
//! The bridge owns a single transport connection and multiplexes frames
//! across priority-ordered send queues.  A writer task drains the queues
//! according to the priority scheduling algorithm, while a reader task
//! demuxes incoming frames to per-channel receive queues.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::sync::Notify;

use liquide_protocol::channel::ChannelId;
use liquide_protocol::frame::FrameHeader;

use crate::congestion::CongestionController;
use crate::priority::{Priority, PriorityMapper, NUM_PRIORITIES};
use crate::sendbuf::SendBufferPool;

// ---------------------------------------------------------------------------
// Scheduling Mode
// ---------------------------------------------------------------------------

/// Writer scheduling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SchedulingMode {
    /// No traffic — park on notify, zero CPU.
    Idle = 0,
    /// Normal traffic — 1 ms interval tick.
    Normal = 1,
    /// Emergency — tight loop on P0/P1 until drained.
    Priority = 2,
}

impl SchedulingMode {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            2 => Self::Priority,
            _ => Self::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// Queued Frame
// ---------------------------------------------------------------------------

/// A frame waiting to be sent, with its priority.
#[derive(Debug, Clone)]
pub struct QueuedFrame {
    /// The frame header.
    pub header: FrameHeader,
    /// The frame payload.
    pub payload: Bytes,
    /// Resolved priority.
    pub priority: Priority,
}

// ---------------------------------------------------------------------------
// Channel Handle
// ---------------------------------------------------------------------------

/// Handle for sending and receiving on a specific channel.
#[derive(Debug)]
pub struct ChannelHandle {
    channel: ChannelId,
    send_tx: mpsc::Sender<QueuedFrame>,
    recv_rx: Mutex<mpsc::Receiver<Bytes>>,
}

impl ChannelHandle {
    /// The channel this handle is for.
    #[must_use]
    pub fn channel(&self) -> ChannelId {
        self.channel
    }

    /// Send a frame on this channel.
    pub async fn send(&self, header: FrameHeader, payload: Bytes) -> Result<(), BridgeError> {
        let frame = QueuedFrame {
            header,
            payload,
            priority: Priority::P5Graphics, // Will be resolved by bridge
        };
        self.send_tx
            .send(frame)
            .await
            .map_err(|_| BridgeError::Closed)
    }

    /// Receive a payload from this channel.
    pub fn recv(&self) -> Result<Bytes, BridgeError> {
        let mut rx = self.recv_rx.lock().unwrap();
        rx.try_recv().map_err(|_| BridgeError::Empty)
    }
}

// ---------------------------------------------------------------------------
// Bridge Error
// ---------------------------------------------------------------------------

/// Errors from bridge operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BridgeError {
    /// The bridge has been shut down.
    #[error("bridge closed")]
    Closed,
    /// No data available (non-blocking recv).
    #[error("no data available")]
    Empty,
    /// The channel is not registered.
    #[error("unknown channel: {0:?}")]
    UnknownChannel(ChannelId),
    /// Transport I/O error.
    #[error("transport error: {0}")]
    Transport(String),
}

// ---------------------------------------------------------------------------
// Bridge Config
// ---------------------------------------------------------------------------

/// Configuration for the transport bridge.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Capacity of each priority send queue.
    pub send_queue_capacity: usize,
    /// Capacity of each channel receive queue.
    pub recv_queue_capacity: usize,
    /// P4 budget fraction (of remaining bandwidth).
    pub p4_budget_fraction: f64,
    /// P5 budget fraction.
    pub p5_budget_fraction: f64,
    /// P6 budget fraction.
    pub p6_budget_fraction: f64,
    /// Minimum bandwidth (bytes/sec) for P6 to be active.
    pub p6_min_bandwidth: f64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            send_queue_capacity: 256,
            recv_queue_capacity: 256,
            p4_budget_fraction: 0.10,
            p5_budget_fraction: 0.85,
            p6_budget_fraction: 0.05,
            p6_min_bandwidth: 2_000_000.0 / 8.0, // 2 Mbps in bytes/sec
        }
    }
}

// ---------------------------------------------------------------------------
// Transport Bridge
// ---------------------------------------------------------------------------

/// The transport I/O bridge.
///
/// Manages priority-ordered send queues, per-channel receive queues,
/// and coordinates with the congestion controller and send buffer pool.
pub struct TransportBridge {
    config: BridgeConfig,
    /// Priority mapper for resolving frame priorities.
    mapper: PriorityMapper,
    /// Per-priority send queues.
    pub(crate) send_queues: Vec<(mpsc::Sender<QueuedFrame>, Mutex<mpsc::Receiver<QueuedFrame>>)>,
    /// Per-channel receive queues.
    recv_queues: HashMap<ChannelId, mpsc::Sender<Bytes>>,
    /// Channel handles (for external callers).
    channel_handles: HashMap<ChannelId, Arc<ChannelHandle>>,
    /// Congestion controller.
    congestion: Arc<Mutex<Box<dyn CongestionController>>>,
    /// Send buffer pool.
    pool: Arc<SendBufferPool>,
    /// Current scheduling mode.
    mode: Arc<AtomicU8>,
    /// Notify for Idle→Normal wakeup.
    wake: Arc<Notify>,
    /// Cancellation flag for shutdown.
    cancelled: Arc<AtomicBool>,
    /// Bytes sent counter (for budget tracking).
    bytes_sent: u64,
}

impl std::fmt::Debug for TransportBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportBridge")
            .field("config", &self.config)
            .field("mode", &self.scheduling_mode())
            .field("cancelled", &self.is_shutdown())
            .field("bytes_sent", &self.bytes_sent)
            .finish()
    }
}

impl TransportBridge {
    /// Create a new bridge with the given configuration.
    pub fn new(
        config: BridgeConfig,
        congestion: Box<dyn CongestionController>,
        pool: Arc<SendBufferPool>,
    ) -> Self {
        let mut send_queues = Vec::with_capacity(NUM_PRIORITIES);
        for _ in 0..NUM_PRIORITIES {
            let (tx, rx) = mpsc::channel(config.send_queue_capacity);
            send_queues.push((tx, Mutex::new(rx)));
        }

        Self {
            config,
            mapper: PriorityMapper::new(),
            send_queues,
            recv_queues: HashMap::new(),
            channel_handles: HashMap::new(),
            congestion: Arc::new(Mutex::new(congestion)),
            pool,
            mode: Arc::new(AtomicU8::new(SchedulingMode::Idle as u8)),
            wake: Arc::new(Notify::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
            bytes_sent: 0,
        }
    }

    /// Create with default config.
    pub fn with_defaults(
        congestion: Box<dyn CongestionController>,
        pool: Arc<SendBufferPool>,
    ) -> Self {
        Self::new(BridgeConfig::default(), congestion, pool)
    }

    /// Register a channel and return a handle for it.
    pub fn register_channel(&mut self, channel: ChannelId) -> Arc<ChannelHandle> {
        let priority = self.mapper.base_priority(channel);
        let send_tx = self.send_queues[priority.as_index()].0.clone();

        let (recv_tx, recv_rx) = mpsc::channel(self.config.recv_queue_capacity);
        self.recv_queues.insert(channel, recv_tx);

        let handle = Arc::new(ChannelHandle {
            channel,
            send_tx,
            recv_rx: Mutex::new(recv_rx),
        });
        self.channel_handles.insert(channel, handle.clone());
        handle
    }

    /// Get a previously registered channel handle.
    #[must_use]
    pub fn channel(&self, id: ChannelId) -> Option<Arc<ChannelHandle>> {
        self.channel_handles.get(&id).cloned()
    }

    /// Enqueue a frame for sending.
    pub fn enqueue(&self, header: FrameHeader, payload: Bytes) -> Result<(), BridgeError> {
        let priority = self.mapper.effective_priority(header.channel, header.flags);
        let frame = QueuedFrame {
            header,
            payload,
            priority,
        };
        let idx = priority.as_index();
        self.send_queues[idx]
            .0
            .try_send(frame)
            .map_err(|_| BridgeError::Closed)?;

        // Wake writer if idle
        if self.scheduling_mode() == SchedulingMode::Idle {
            self.set_scheduling_mode(SchedulingMode::Normal);
            self.wake.notify_one();
        }

        // Promote to Priority mode for P0/P1
        if priority <= Priority::P1Input {
            self.set_scheduling_mode(SchedulingMode::Priority);
        }

        Ok(())
    }

    /// Deliver a received payload to the appropriate channel queue.
    pub fn deliver(&self, channel: ChannelId, payload: Bytes) -> Result<(), BridgeError> {
        let tx = self
            .recv_queues
            .get(&channel)
            .ok_or(BridgeError::UnknownChannel(channel))?;
        tx.try_send(payload)
            .map_err(|_| BridgeError::Closed)
    }

    /// Drain frames from the priority queues according to the scheduling algorithm.
    ///
    /// Returns the frames to send, in priority order.
    pub fn drain_queues(&mut self, pacing_budget: u64) -> Vec<QueuedFrame> {
        let mut frames = Vec::new();
        let mut remaining = pacing_budget;

        // P0: drain all (unlimited)
        self.drain_priority(&mut frames, Priority::P0Emergency, &mut remaining, u64::MAX);

        // P1: drain all input events
        self.drain_priority(&mut frames, Priority::P1Input, &mut remaining, u64::MAX);

        // P2: latest cursor only (take last, skip rest)
        self.drain_latest(&mut frames, Priority::P2Cursor, &mut remaining);

        // P3: one audio frame
        self.drain_priority(&mut frames, Priority::P3Audio, &mut remaining, 1);

        // Budget for remaining priorities
        if remaining > 0 {
            // P4: budget * p4_fraction
            let p4_budget = (remaining as f64 * self.config.p4_budget_fraction) as u64;
            self.drain_priority_budget(&mut frames, Priority::P4Control, &mut remaining, p4_budget);

            // P5: budget * p5_fraction (backpressure check)
            if !self.pool.is_backpressure() {
                let p5_budget = (remaining as f64 * self.config.p5_budget_fraction) as u64;
                self.drain_priority_budget(
                    &mut frames,
                    Priority::P5Graphics,
                    &mut remaining,
                    p5_budget,
                );
            }

            // P6: budget * p6_fraction (suspend check)
            let cc = self.congestion.lock().unwrap();
            let pacing_rate = cc.pacing_rate();
            drop(cc);
            if !self.pool.is_suspended() && pacing_rate >= self.config.p6_min_bandwidth {
                let p6_budget = (remaining as f64 * self.config.p6_budget_fraction) as u64;
                self.drain_priority_budget(
                    &mut frames,
                    Priority::P6Bulk,
                    &mut remaining,
                    p6_budget,
                );
            }
        }

        // Update mode: if no frames remain, go Idle
        let any_queued = self.send_queues.iter().any(|(_, rx)| {
            let rx = rx.lock().unwrap();
            !rx.is_empty()
        });
        if !any_queued {
            self.set_scheduling_mode(SchedulingMode::Idle);
        } else if self.scheduling_mode() == SchedulingMode::Priority {
            // Check if P0/P1 are drained
            let p0_empty = self.send_queues[0].1.lock().unwrap().is_empty();
            let p1_empty = self.send_queues[1].1.lock().unwrap().is_empty();
            if p0_empty && p1_empty {
                self.set_scheduling_mode(SchedulingMode::Normal);
            }
        }

        frames
    }

    /// Current scheduling mode.
    #[must_use]
    pub fn scheduling_mode(&self) -> SchedulingMode {
        SchedulingMode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    /// Set the scheduling mode.
    pub fn set_scheduling_mode(&self, mode: SchedulingMode) {
        self.mode.store(mode as u8, Ordering::Relaxed);
    }

    /// Signal shutdown.
    pub fn shutdown(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.wake.notify_waiters();
    }

    /// Whether the bridge has been shut down.
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Access the priority mapper.
    #[must_use]
    pub fn mapper(&self) -> &PriorityMapper {
        &self.mapper
    }

    /// Access the congestion controller.
    #[must_use]
    pub fn congestion(&self) -> &Arc<Mutex<Box<dyn CongestionController>>> {
        &self.congestion
    }

    /// Running total of bytes sent.
    #[must_use]
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Record bytes sent (for budget tracking).
    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
    }

    // -- internal helpers --

    fn drain_priority(
        &self,
        out: &mut Vec<QueuedFrame>,
        priority: Priority,
        remaining: &mut u64,
        max_frames: u64,
    ) {
        let idx = priority.as_index();
        let mut rx = self.send_queues[idx].1.lock().unwrap();
        let mut count = 0u64;
        while count < max_frames {
            match rx.try_recv() {
                Ok(frame) => {
                    let size = frame.payload.len() as u64;
                    *remaining = remaining.saturating_sub(size);
                    out.push(frame);
                    count += 1;
                }
                Err(_) => break,
            }
        }
    }

    fn drain_latest(
        &self,
        out: &mut Vec<QueuedFrame>,
        priority: Priority,
        remaining: &mut u64,
    ) {
        let idx = priority.as_index();
        let mut rx = self.send_queues[idx].1.lock().unwrap();
        let mut latest = None;
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }
        if let Some(frame) = latest {
            let size = frame.payload.len() as u64;
            *remaining = remaining.saturating_sub(size);
            out.push(frame);
        }
    }

    fn drain_priority_budget(
        &self,
        out: &mut Vec<QueuedFrame>,
        priority: Priority,
        remaining: &mut u64,
        mut budget: u64,
    ) {
        let idx = priority.as_index();
        let mut rx = self.send_queues[idx].1.lock().unwrap();
        while budget > 0 {
            match rx.try_recv() {
                Ok(frame) => {
                    let size = frame.payload.len() as u64;
                    budget = budget.saturating_sub(size);
                    *remaining = remaining.saturating_sub(size);
                    out.push(frame);
                }
                Err(_) => break,
            }
        }
    }
}

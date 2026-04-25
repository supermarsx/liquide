//! Per-channel loss recovery strategies.
//!
//! Each channel type has a distinct recovery strategy tailored to its
//! traffic pattern: video channels request key frames, audio uses PLC
//! (packet loss concealment), input retransmits reliably, and bulk
//! channels use standard retransmission.

use std::collections::HashMap;

use liquide_protocol::channel::ChannelId;

// ---------------------------------------------------------------------------
// Recovery Strategy
// ---------------------------------------------------------------------------

/// The loss recovery strategy for a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryStrategy {
    /// Request a key frame from the encoder (video/graphics).
    VideoKeyframe,
    /// Packet loss concealment — interpolate or repeat (audio).
    AudioPlc,
    /// Use the latest value only; discard stale updates (cursor).
    CursorLatest,
    /// Reliable retransmission (control, input, bulk).
    ReliableRetransmit,
}

// ---------------------------------------------------------------------------
// Recovery Action
// ---------------------------------------------------------------------------

/// An action to take when loss is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Request a key frame from the encoder.
    KeyFrameRequest,
    /// Apply packet loss concealment.
    Plc,
    /// Attempt FEC recovery.
    FecRecover,
    /// Repeat the previous frame with fade-out.
    RepeatWithFade,
    /// Retransmit the lost packet.
    Retransmit,
    /// Ignore the loss (e.g. stale cursor update).
    Ignore,
}

// ---------------------------------------------------------------------------
// Channel Sequence Tracker
// ---------------------------------------------------------------------------

/// Tracks expected sequence numbers for a single channel and detects gaps.
#[derive(Debug)]
pub struct ChannelSequenceTracker {
    /// The next expected sequence number.
    expected_seq: u32,
    /// Number of gaps detected.
    gaps: u64,
    /// Number of out-of-order arrivals.
    reordered: u64,
    /// Whether we have received any packet yet.
    initialized: bool,
}

impl ChannelSequenceTracker {
    /// Create a new tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            expected_seq: 0,
            gaps: 0,
            reordered: 0,
            initialized: false,
        }
    }

    /// Record arrival of a packet with the given sequence number.
    ///
    /// Returns `true` if the packet is in-order, `false` if it represents
    /// a gap or out-of-order arrival.
    pub fn on_packet(&mut self, seq: u32) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.expected_seq = seq.wrapping_add(1);
            return true;
        }

        if seq == self.expected_seq {
            self.expected_seq = seq.wrapping_add(1);
            true
        } else if seq > self.expected_seq {
            // Gap detected
            let gap_size = seq.wrapping_sub(self.expected_seq);
            self.gaps += gap_size as u64;
            self.expected_seq = seq.wrapping_add(1);
            false
        } else {
            // Out of order (seq < expected)
            self.reordered += 1;
            false
        }
    }

    /// Number of detected gaps (missing sequence numbers).
    #[must_use]
    pub fn gap_count(&self) -> u64 {
        self.gaps
    }

    /// Number of out-of-order arrivals.
    #[must_use]
    pub fn reorder_count(&self) -> u64 {
        self.reordered
    }

    /// The next expected sequence number.
    #[must_use]
    pub fn expected_seq(&self) -> u32 {
        self.expected_seq
    }
}

impl Default for ChannelSequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Loss Recovery Manager
// ---------------------------------------------------------------------------

/// Manages per-channel sequence tracking and strategy dispatch.
#[derive(Debug)]
pub struct LossRecoveryManager {
    /// Per-channel trackers.
    trackers: HashMap<ChannelId, ChannelSequenceTracker>,
    /// Per-channel recovery strategies.
    strategies: HashMap<ChannelId, RecoveryStrategy>,
}

impl LossRecoveryManager {
    /// Create a new manager with default strategies for all channels.
    #[must_use]
    pub fn new() -> Self {
        let mut strategies = HashMap::new();
        strategies.insert(ChannelId::CONTROL, RecoveryStrategy::ReliableRetransmit);
        strategies.insert(ChannelId::EMERGENCY, RecoveryStrategy::ReliableRetransmit);
        strategies.insert(ChannelId::VIDEO, RecoveryStrategy::VideoKeyframe);
        strategies.insert(ChannelId::TILE, RecoveryStrategy::VideoKeyframe);
        strategies.insert(ChannelId::AUDIO_PLAYBACK, RecoveryStrategy::AudioPlc);
        strategies.insert(ChannelId::AUDIO_CAPTURE, RecoveryStrategy::AudioPlc);
        strategies.insert(ChannelId::INPUT, RecoveryStrategy::ReliableRetransmit);
        strategies.insert(ChannelId::CURSOR, RecoveryStrategy::CursorLatest);
        strategies.insert(ChannelId::CLIPBOARD, RecoveryStrategy::ReliableRetransmit);
        strategies.insert(ChannelId::USB, RecoveryStrategy::ReliableRetransmit);
        strategies.insert(
            ChannelId::FILE_TRANSFER,
            RecoveryStrategy::ReliableRetransmit,
        );
        strategies.insert(ChannelId::CAMERA, RecoveryStrategy::VideoKeyframe);

        Self {
            trackers: HashMap::new(),
            strategies,
        }
    }

    /// Override the recovery strategy for a specific channel.
    pub fn set_strategy(&mut self, channel: ChannelId, strategy: RecoveryStrategy) {
        self.strategies.insert(channel, strategy);
    }

    /// Get the recovery strategy for a channel.
    #[must_use]
    pub fn strategy(&self, channel: ChannelId) -> RecoveryStrategy {
        self.strategies
            .get(&channel)
            .copied()
            .unwrap_or(RecoveryStrategy::ReliableRetransmit)
    }

    /// Record a packet arrival and return the recovery action if loss is detected.
    ///
    /// Returns `None` if the packet is in-order, or `Some(action)` if a gap
    /// or out-of-order arrival was detected.
    pub fn on_packet(&mut self, channel: ChannelId, seq: u32) -> Option<RecoveryAction> {
        let tracker = self
            .trackers
            .entry(channel)
            .or_insert_with(ChannelSequenceTracker::new);

        if tracker.on_packet(seq) {
            None
        } else {
            Some(self.dispatch(channel))
        }
    }

    /// Get the sequence tracker for a channel.
    #[must_use]
    pub fn tracker(&self, channel: ChannelId) -> Option<&ChannelSequenceTracker> {
        self.trackers.get(&channel)
    }

    /// Dispatch a recovery action based on the channel's strategy.
    fn dispatch(&self, channel: ChannelId) -> RecoveryAction {
        match self.strategy(channel) {
            RecoveryStrategy::VideoKeyframe => RecoveryAction::KeyFrameRequest,
            RecoveryStrategy::AudioPlc => RecoveryAction::Plc,
            RecoveryStrategy::CursorLatest => RecoveryAction::Ignore,
            RecoveryStrategy::ReliableRetransmit => RecoveryAction::Retransmit,
        }
    }
}

impl Default for LossRecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

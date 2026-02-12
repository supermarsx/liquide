//! Priority levels and channel-to-priority mapping.
//!
//! The LiquiDE protocol defines seven priority levels (P0–P6) that control
//! how the transport bridge schedules outgoing frames.

use liquide_protocol::channel::ChannelId;
use liquide_protocol::frame::FrameFlags;
use serde::{Deserialize, Serialize};

/// Number of distinct priority levels.
pub const NUM_PRIORITIES: usize = 7;

/// Transport priority levels from highest (P0) to lowest (P6).
///
/// Derives `Ord` so that `P0 < P1 < ... < P6` — a *lower* numeric value means
/// *higher* scheduling priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    /// Emergency frames (e.g. graceful-disconnect, urgent control).
    P0Emergency = 0,
    /// Input events (keyboard, mouse, touch) — latency-critical.
    P1Input = 1,
    /// Cursor position updates — latest-only, may be coalesced.
    P2Cursor = 2,
    /// Audio frames — cadence-sensitive (20 ms).
    P3Audio = 3,
    /// Control channel messages (handshake, capability, keepalive).
    P4Control = 4,
    /// Graphics / video tile data — bandwidth-dominant.
    P5Graphics = 5,
    /// Bulk channels: clipboard, USB, file, print, serial, plugin, recording.
    P6Bulk = 6,
}

impl Priority {
    /// Convert to the underlying index (0..7).
    #[must_use]
    pub fn as_index(self) -> usize {
        self as usize
    }

    /// Try to convert a raw index back to a `Priority`.
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::P0Emergency),
            1 => Some(Self::P1Input),
            2 => Some(Self::P2Cursor),
            3 => Some(Self::P3Audio),
            4 => Some(Self::P4Control),
            5 => Some(Self::P5Graphics),
            6 => Some(Self::P6Bulk),
            _ => None,
        }
    }

    /// Returns `true` for priorities that should never be dropped or delayed.
    #[must_use]
    pub fn is_realtime(self) -> bool {
        matches!(
            self,
            Self::P0Emergency | Self::P1Input | Self::P2Cursor | Self::P3Audio
        )
    }

    /// Returns `true` for bulk priorities that can be paused under pressure.
    #[must_use]
    pub fn is_bulk(self) -> bool {
        self == Self::P6Bulk
    }

    /// Iterator over all priority levels in scheduling order (P0 first).
    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::P0Emergency,
            Self::P1Input,
            Self::P2Cursor,
            Self::P3Audio,
            Self::P4Control,
            Self::P5Graphics,
            Self::P6Bulk,
        ]
        .into_iter()
    }
}

/// Maps channel IDs and frame flags to transport priority levels.
///
/// The default mapping follows the spec:
///
/// | Priority | Channels |
/// |----------|----------|
/// | P0 | Any channel with `FrameFlags::PRIORITY` on CONTROL |
/// | P1 | INPUT |
/// | P2 | CURSOR (detected at bridge level) |
/// | P3 | AUDIO_PLAYBACK, AUDIO_CAPTURE |
/// | P4 | CONTROL, EMERGENCY |
/// | P5 | VIDEO, TILE |
/// | P6 | CLIPBOARD, USB, FILE_TRANSFER, CAMERA, virtual channels |
#[derive(Debug, Clone)]
pub struct PriorityMapper {
    /// Per-channel base priority.
    table: std::collections::HashMap<ChannelId, Priority>,
}

impl PriorityMapper {
    /// Create a mapper with the spec-default channel→priority mapping.
    #[must_use]
    pub fn new() -> Self {
        let mut table = std::collections::HashMap::new();
        table.insert(ChannelId::CONTROL, Priority::P4Control);
        table.insert(ChannelId::EMERGENCY, Priority::P4Control);
        table.insert(ChannelId::VIDEO, Priority::P5Graphics);
        table.insert(ChannelId::TILE, Priority::P5Graphics);
        table.insert(ChannelId::AUDIO_PLAYBACK, Priority::P3Audio);
        table.insert(ChannelId::AUDIO_CAPTURE, Priority::P3Audio);
        table.insert(ChannelId::INPUT, Priority::P1Input);
        table.insert(ChannelId::CURSOR, Priority::P2Cursor);
        // Bulk channels remain P6Bulk (the default for unmapped)
        table.insert(ChannelId::CLIPBOARD, Priority::P6Bulk);
        table.insert(ChannelId::USB, Priority::P6Bulk);
        table.insert(ChannelId::FILE_TRANSFER, Priority::P6Bulk);
        table.insert(ChannelId::CAMERA, Priority::P6Bulk);
        Self { table }
    }

    /// Look up the base priority for a channel.
    #[must_use]
    pub fn base_priority(&self, channel: ChannelId) -> Priority {
        self.table
            .get(&channel)
            .copied()
            .unwrap_or(Priority::P6Bulk)
    }

    /// Determine the effective priority for a frame, taking flags into account.
    ///
    /// If the `PRIORITY` flag is set on a `CONTROL` frame, it is promoted to P0.
    #[must_use]
    pub fn effective_priority(&self, channel: ChannelId, flags: u8) -> Priority {
        if channel == ChannelId::CONTROL && (flags & FrameFlags::PRIORITY) != 0 {
            return Priority::P0Emergency;
        }
        self.base_priority(channel)
    }

    /// Override the base priority for a specific channel.
    pub fn set_channel_priority(&mut self, channel: ChannelId, priority: Priority) {
        self.table.insert(channel, priority);
    }
}

impl Default for PriorityMapper {
    fn default() -> Self {
        Self::new()
    }
}

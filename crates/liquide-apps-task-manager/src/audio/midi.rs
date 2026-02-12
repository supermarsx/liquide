//! MIDI device and message types (spec section 16.10).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// MidiDeviceType
// ---------------------------------------------------------------------------

/// MIDI port direction capability (spec section 16.10 – Type column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiDeviceType {
    Input,
    Output,
    Both,
}

impl MidiDeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Output => "Output",
            Self::Both => "Input+Output",
        }
    }
}

impl fmt::Display for MidiDeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MidiDevice
// ---------------------------------------------------------------------------

/// A connected MIDI device (spec section 16.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    /// Unique device identifier.
    pub id: String,
    /// Friendly device name.
    pub name: String,
    /// Port direction capability.
    pub device_type: MidiDeviceType,
    /// Device manufacturer (if reported).
    pub manufacturer: Option<String>,
    /// Whether the device is currently connected.
    pub connected: bool,
    /// MIDI port number.
    pub port_number: u8,
    /// Driver name (if applicable).
    pub driver_name: Option<String>,
    /// Total MIDI messages received from this device.
    pub messages_received: u64,
    /// Total MIDI messages sent to this device.
    pub messages_sent: u64,
    /// ISO-8601 timestamp of the last MIDI activity (if any).
    pub last_activity: Option<String>,
    /// Whether System Exclusive messages are enabled.
    pub sysex_enabled: bool,
}

// ---------------------------------------------------------------------------
// MidiMessage
// ---------------------------------------------------------------------------

/// A single MIDI message captured in the MIDI monitor (spec section 16.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiMessage {
    /// ISO-8601 timestamp of the message.
    pub timestamp: String,
    /// Identifier of the MIDI device that sent or received the message.
    pub device_id: String,
    /// MIDI channel (0–15).
    pub channel: u8,
    /// Message type description (e.g., "Note On", "CC", "Program Change").
    pub message_type: String,
    /// Raw MIDI data bytes.
    pub data: Vec<u8>,
}

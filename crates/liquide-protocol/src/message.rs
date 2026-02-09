//! Message types exchanged between server and client.

use serde::{Deserialize, Serialize};

/// Top-level message type discriminant.
///
/// Every message on the control channel begins with this tag so the peer
/// knows how to interpret the CBOR payload that follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    // ── Handshake & session lifecycle ──────────────────────────
    /// Client hello (first message sent by the client).
    ClientHello = 0x0001,
    /// Server hello (response to ClientHello).
    ServerHello = 0x0002,
    /// Capability negotiation request.
    CapabilityRequest = 0x0003,
    /// Capability negotiation response.
    CapabilityResponse = 0x0004,
    /// Graceful session disconnect.
    Disconnect = 0x0005,
    /// Session keepalive ping.
    Ping = 0x0006,
    /// Session keepalive pong.
    Pong = 0x0007,

    // ── Authentication ────────────────────────────────────────
    /// Authentication challenge from the server.
    AuthChallenge = 0x0100,
    /// Authentication response from the client.
    AuthResponse = 0x0101,
    /// Authentication succeeded.
    AuthSuccess = 0x0102,
    /// Authentication failed.
    AuthFailure = 0x0103,

    // ── Graphics ──────────────────────────────────────────────
    /// Full-frame graphics update.
    FrameUpdate = 0x0200,
    /// Tile (region) update.
    TileUpdate = 0x0201,
    /// Cursor shape change.
    CursorUpdate = 0x0202,

    // ── Input ─────────────────────────────────────────────────
    /// Keyboard event.
    KeyEvent = 0x0300,
    /// Pointer / mouse event.
    PointerEvent = 0x0301,
    /// Touch event.
    TouchEvent = 0x0302,

    // ── Clipboard ─────────────────────────────────────────────
    /// Clipboard offer (advertise available formats).
    ClipboardOffer = 0x0400,
    /// Clipboard data request.
    ClipboardRequest = 0x0401,
    /// Clipboard data payload.
    ClipboardData = 0x0402,

    // ── Audio ─────────────────────────────────────────────────
    /// Audio configuration.
    AudioConfig = 0x0500,
    /// Audio sample data.
    AudioData = 0x0501,

    // ── USB ───────────────────────────────────────────────────
    /// USB device attach notification.
    UsbAttach = 0x0600,
    /// USB device detach notification.
    UsbDetach = 0x0601,
    /// USB bulk data transfer.
    UsbData = 0x0602,
}

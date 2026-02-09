//! Protocol version constants and negotiation helpers.

/// Magic bytes at the start of every Liquide connection (`0x4C44` = ASCII `"LD"`).
pub const MAGIC: u16 = 0x4C44;

/// Current protocol version string sent during the handshake.
pub const PROTOCOL_VERSION: &str = "proto/1";

/// Minimum protocol version that this build can still understand.
pub const MIN_SUPPORTED_VERSION: &str = "proto/1";

/// Check whether a peer's advertised version is compatible with ours.
#[must_use]
pub fn is_compatible(peer_version: &str) -> bool {
    // For proto/1 the only compatible version is proto/1 itself.
    peer_version == PROTOCOL_VERSION
}

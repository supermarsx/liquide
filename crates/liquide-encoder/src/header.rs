//! Compressed tile header for wire serialization.
//!
//! Each compressed tile on the wire is prefixed by an 8-byte header:
//!
//! ```text
//! ┌────────┬──────────┬──────────┬──────────┬────────────────┐
//! │ tx (u8)│ ty (u8)  │ enc (u8) │ flags(u8)│ length (u32 LE)│
//! └────────┴──────────┴──────────┴──────────┴────────────────┘
//! ```

/// Compressed tile header (8 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressedTileHeader {
    /// Tile X coordinate.
    pub tx: u8,
    /// Tile Y coordinate.
    pub ty: u8,
    /// Encoding type (0=Skip, 1=Delta, 2=Full, 3=Copy, 4=Solid).
    pub encoding: u8,
    /// Flags (reserved, currently 0).
    pub flags: u8,
    /// Length of the compressed payload in bytes.
    pub payload_length: u32,
}

/// Encoding type constants.
pub const ENC_SKIP: u8 = 0;
pub const ENC_DELTA: u8 = 1;
pub const ENC_FULL: u8 = 2;
pub const ENC_COPY: u8 = 3;
pub const ENC_SOLID: u8 = 4;

/// Size of the tile header on the wire.
pub const HEADER_SIZE: usize = 8;

impl CompressedTileHeader {
    /// Serialize the header into 8 bytes (little-endian).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let len = self.payload_length.to_le_bytes();
        [
            self.tx,
            self.ty,
            self.encoding,
            self.flags,
            len[0],
            len[1],
            len[2],
            len[3],
        ]
    }

    /// Deserialize a header from 8 bytes.
    ///
    /// Returns `None` if the slice is too short.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < HEADER_SIZE {
            return None;
        }
        Some(Self {
            tx: data[0],
            ty: data[1],
            encoding: data[2],
            flags: data[3],
            payload_length: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        })
    }

    /// Create a header for a skip tile (no payload).
    #[must_use]
    pub fn skip(tx: u8, ty: u8) -> Self {
        Self {
            tx,
            ty,
            encoding: ENC_SKIP,
            flags: 0,
            payload_length: 0,
        }
    }
}

//! Core Wayland-like protocol types.
//!
//! Implements the wire format encoding/decoding for protocol messages,
//! with 4-byte aligned argument serialization as defined by the
//! Wayland wire protocol specification.

use std::fmt;

// ---------------------------------------------------------------------------
// ObjectId
// ---------------------------------------------------------------------------

/// Unique identifier for a protocol object.
///
/// Object IDs are unsigned 32-bit integers. ID 0 is reserved as a null/invalid
/// sentinel. Client-allocated IDs start at 1, and the display singleton is
/// always ID 1.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

impl ObjectId {
    /// The null object ID (represents "no object").
    pub const NULL: Self = Self(0);

    /// The wl_display singleton is always object 1.
    pub const DISPLAY: Self = Self(1);

    /// Returns `true` if this is the null object.
    #[inline]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Returns the raw u32 value.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", self.0)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl From<u32> for ObjectId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

// ---------------------------------------------------------------------------
// MessageHeader
// ---------------------------------------------------------------------------

/// Wire-format message header.
///
/// Layout (8 bytes total):
/// - bytes 0..4: object ID (sender)
/// - bytes 4..6: message size in bytes (including header)
/// - bytes 6..8: opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    /// The protocol object this message is addressed to (or sent from).
    pub object_id: ObjectId,
    /// The opcode identifying which request/event this is.
    pub opcode: u16,
    /// Total message size in bytes, including the 8-byte header.
    pub size: u16,
}

impl MessageHeader {
    /// Header size is always 8 bytes on the wire.
    pub const SIZE: usize = 8;

    /// Encode the header into an 8-byte array (little-endian).
    pub fn encode(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&self.object_id.0.to_le_bytes());
        let size_opcode = (self.size as u32) << 16 | (self.opcode as u32);
        buf[4..8].copy_from_slice(&size_opcode.to_le_bytes());
        buf
    }

    /// Decode a header from an 8-byte slice (little-endian).
    ///
    /// Returns `None` if the slice is shorter than 8 bytes.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 8 {
            return None;
        }
        let object_id = ObjectId(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]));
        let size_opcode = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let opcode = (size_opcode & 0xFFFF) as u16;
        let size = (size_opcode >> 16) as u16;
        Some(Self {
            object_id,
            opcode,
            size,
        })
    }
}

// ---------------------------------------------------------------------------
// Arg — protocol argument types
// ---------------------------------------------------------------------------

/// A single argument in a protocol message.
///
/// Matches the Wayland wire protocol argument types:
/// - `Int`: signed 32-bit integer
/// - `Uint`: unsigned 32-bit integer
/// - `Fixed`: 24.8 signed fixed-point number
/// - `String`: length-prefixed UTF-8 string (NUL-terminated on wire)
/// - `NewId`: an object ID allocated by the sender
/// - `Array`: length-prefixed byte array
/// - `Fd`: file descriptor (out-of-band, encoded as zero bytes on wire)
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Int(i32),
    Uint(u32),
    Fixed(i32),
    String(String),
    NewId(ObjectId),
    Array(Vec<u8>),
    Fd,
}

impl Arg {
    /// Convert a floating-point value to the Wayland 24.8 fixed-point format.
    #[inline]
    pub fn float_to_fixed(v: f64) -> i32 {
        (v * 256.0) as i32
    }

    /// Convert a Wayland 24.8 fixed-point value back to floating-point.
    #[inline]
    pub fn fixed_to_float(v: i32) -> f64 {
        v as f64 / 256.0
    }

    /// Size of this argument on the wire, in bytes (4-byte aligned).
    pub fn wire_size(&self) -> usize {
        match self {
            Arg::Int(_) | Arg::Uint(_) | Arg::Fixed(_) | Arg::NewId(_) => 4,
            Arg::String(s) => {
                // 4 bytes length prefix + string bytes + NUL + padding to 4
                let byte_len = s.len() + 1; // +1 for NUL
                4 + align_up(byte_len, 4)
            }
            Arg::Array(a) => {
                // 4 bytes length prefix + data bytes + padding to 4
                4 + align_up(a.len(), 4)
            }
            Arg::Fd => 0, // sent out-of-band via SCM_RIGHTS
        }
    }
}

// ---------------------------------------------------------------------------
// WlMessage
// ---------------------------------------------------------------------------

/// A complete protocol message: header plus arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct WlMessage {
    pub header: MessageHeader,
    pub args: Vec<Arg>,
}

impl WlMessage {
    /// Build a new message for the given object/opcode and arguments.
    ///
    /// The header size is computed automatically from the arguments.
    pub fn new(object_id: ObjectId, opcode: u16, args: Vec<Arg>) -> Self {
        let body_size: usize = args.iter().map(|a| a.wire_size()).sum();
        let total = MessageHeader::SIZE + body_size;
        Self {
            header: MessageHeader {
                object_id,
                opcode,
                size: total as u16,
            },
            args,
        }
    }

    /// Encode the entire message (header + arguments) to a byte vector.
    ///
    /// The result is 4-byte aligned and ready for the wire.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.header.size as usize);
        buf.extend_from_slice(&self.header.encode());

        for arg in &self.args {
            encode_arg(&mut buf, arg);
        }

        buf
    }

    /// Decode a message from a byte buffer, using the provided argument type
    /// descriptors to know how to parse the body.
    ///
    /// `arg_types` gives the expected type of each argument in order.
    /// Returns `None` on malformed data.
    pub fn decode(buf: &[u8], arg_types: &[ArgType]) -> Option<Self> {
        let header = MessageHeader::decode(buf)?;
        if buf.len() < header.size as usize {
            return None;
        }

        let mut offset = MessageHeader::SIZE;
        let mut args = Vec::with_capacity(arg_types.len());

        for ty in arg_types {
            let (arg, consumed) = decode_arg(&buf[offset..], *ty)?;
            args.push(arg);
            offset += consumed;
        }

        Some(Self { header, args })
    }
}

// ---------------------------------------------------------------------------
// ArgType — used during decoding
// ---------------------------------------------------------------------------

/// Describes the type of an argument, used for decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Int,
    Uint,
    Fixed,
    String,
    NewId,
    Array,
    Fd,
}

// ---------------------------------------------------------------------------
// Interface & MessageDesc — protocol introspection
// ---------------------------------------------------------------------------

/// Describes a protocol interface (e.g. wl_surface, xdg_toplevel).
#[derive(Debug, Clone)]
pub struct Interface {
    /// Interface name (e.g. "wl_surface").
    pub name: String,
    /// Interface version.
    pub version: u32,
    /// Descriptions of request messages (client → server).
    pub requests: Vec<MessageDesc>,
    /// Descriptions of event messages (server → client).
    pub events: Vec<MessageDesc>,
}

impl Interface {
    /// Look up a request by opcode.
    pub fn request(&self, opcode: u16) -> Option<&MessageDesc> {
        self.requests.get(opcode as usize)
    }

    /// Look up an event by opcode.
    pub fn event(&self, opcode: u16) -> Option<&MessageDesc> {
        self.events.get(opcode as usize)
    }
}

/// Describes a single request or event in a protocol interface.
#[derive(Debug, Clone)]
pub struct MessageDesc {
    /// Human-readable name (e.g. "attach", "commit").
    pub name: String,
    /// Ordered argument types.
    pub args: Vec<ArgType>,
}

// ---------------------------------------------------------------------------
// Wire-format helpers
// ---------------------------------------------------------------------------

/// Round `n` up to the next multiple of `align`.
#[inline]
fn align_up(n: usize, align: usize) -> usize {
    (n + align - 1) & !(align - 1)
}

/// Encode a single argument onto the buffer.
fn encode_arg(buf: &mut Vec<u8>, arg: &Arg) {
    match arg {
        Arg::Int(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::Uint(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::Fixed(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::NewId(id) => buf.extend_from_slice(&id.0.to_le_bytes()),
        Arg::String(s) => {
            let bytes = s.as_bytes();
            let len_with_nul = (bytes.len() + 1) as u32; // +1 for NUL
            buf.extend_from_slice(&len_with_nul.to_le_bytes());
            buf.extend_from_slice(bytes);
            buf.push(0); // NUL terminator
            // Pad to 4-byte alignment
            let padded = align_up(len_with_nul as usize, 4);
            for _ in len_with_nul as usize..padded {
                buf.push(0);
            }
        }
        Arg::Array(a) => {
            let len = a.len() as u32;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(a);
            let padded = align_up(a.len(), 4);
            for _ in a.len()..padded {
                buf.push(0);
            }
        }
        Arg::Fd => {
            // FDs are sent out-of-band; nothing on the wire.
        }
    }
}

/// Decode a single argument from the buffer.
///
/// Returns `(Arg, bytes_consumed)` or `None` on error.
fn decode_arg(buf: &[u8], ty: ArgType) -> Option<(Arg, usize)> {
    match ty {
        ArgType::Int => {
            if buf.len() < 4 {
                return None;
            }
            let v = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            Some((Arg::Int(v), 4))
        }
        ArgType::Uint => {
            if buf.len() < 4 {
                return None;
            }
            let v = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            Some((Arg::Uint(v), 4))
        }
        ArgType::Fixed => {
            if buf.len() < 4 {
                return None;
            }
            let v = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            Some((Arg::Fixed(v), 4))
        }
        ArgType::NewId => {
            if buf.len() < 4 {
                return None;
            }
            let v = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            Some((Arg::NewId(ObjectId(v)), 4))
        }
        ArgType::String => {
            if buf.len() < 4 {
                return None;
            }
            let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            if len == 0 {
                return Some((Arg::String(String::new()), 4));
            }
            let padded = align_up(len, 4);
            if buf.len() < 4 + padded {
                return None;
            }
            // len includes NUL terminator
            let str_bytes = &buf[4..4 + len - 1];
            let s = String::from_utf8(str_bytes.to_vec()).ok()?;
            Some((Arg::String(s), 4 + padded))
        }
        ArgType::Array => {
            if buf.len() < 4 {
                return None;
            }
            let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            let padded = align_up(len, 4);
            if buf.len() < 4 + padded {
                return None;
            }
            let data = buf[4..4 + len].to_vec();
            Some((Arg::Array(data), 4 + padded))
        }
        ArgType::Fd => {
            // No wire data for FDs.
            Some((Arg::Fd, 0))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_basics() {
        assert!(ObjectId::NULL.is_null());
        assert!(!ObjectId::DISPLAY.is_null());
        assert_eq!(ObjectId::DISPLAY.raw(), 1);
        assert_eq!(ObjectId::from(42).raw(), 42);
    }

    #[test]
    fn object_id_display_format() {
        assert_eq!(format!("{}", ObjectId(7)), "@7");
        assert_eq!(format!("{:?}", ObjectId(7)), "ObjectId(7)");
    }

    #[test]
    fn object_id_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ObjectId(1));
        set.insert(ObjectId(2));
        set.insert(ObjectId(1));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn header_encode_decode_roundtrip() {
        let hdr = MessageHeader {
            object_id: ObjectId(42),
            opcode: 3,
            size: 16,
        };
        let bytes = hdr.encode();
        assert_eq!(bytes.len(), 8);
        let decoded = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded, hdr);
    }

    #[test]
    fn header_decode_too_short() {
        assert!(MessageHeader::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn header_size_field() {
        let hdr = MessageHeader {
            object_id: ObjectId(1),
            opcode: 0,
            size: 256,
        };
        let bytes = hdr.encode();
        let decoded = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.size, 256);
    }

    #[test]
    fn arg_int_wire_size() {
        assert_eq!(Arg::Int(0).wire_size(), 4);
        assert_eq!(Arg::Int(-1).wire_size(), 4);
    }

    #[test]
    fn arg_uint_wire_size() {
        assert_eq!(Arg::Uint(0).wire_size(), 4);
    }

    #[test]
    fn arg_fixed_wire_size() {
        assert_eq!(Arg::Fixed(0).wire_size(), 4);
    }

    #[test]
    fn arg_new_id_wire_size() {
        assert_eq!(Arg::NewId(ObjectId(1)).wire_size(), 4);
    }

    #[test]
    fn arg_fd_wire_size() {
        assert_eq!(Arg::Fd.wire_size(), 0);
    }

    #[test]
    fn arg_string_wire_size_aligned() {
        // "hi" => 3 bytes (h, i, NUL) => len prefix 4 + padded 4 = 8
        assert_eq!(Arg::String("hi".into()).wire_size(), 8);
        // "hello" => 6 bytes (5 + NUL) => len prefix 4 + padded 8 = 12
        assert_eq!(Arg::String("hello".into()).wire_size(), 12);
        // "abc" => 4 bytes (3 + NUL) => len prefix 4 + padded 4 = 8
        assert_eq!(Arg::String("abc".into()).wire_size(), 8);
    }

    #[test]
    fn arg_array_wire_size_aligned() {
        assert_eq!(Arg::Array(vec![1, 2, 3]).wire_size(), 8); // 4 + pad(3)=4
        assert_eq!(Arg::Array(vec![1, 2, 3, 4]).wire_size(), 8); // 4 + 4
        assert_eq!(Arg::Array(vec![1, 2, 3, 4, 5]).wire_size(), 12); // 4 + pad(5)=8
    }

    #[test]
    fn fixed_point_conversion() {
        let fixed = Arg::float_to_fixed(1.5);
        assert_eq!(fixed, 384); // 1.5 * 256
        let back = Arg::fixed_to_float(fixed);
        assert!((back - 1.5).abs() < 0.01);
    }

    #[test]
    fn fixed_point_negative() {
        let fixed = Arg::float_to_fixed(-2.25);
        assert_eq!(fixed, -576); // -2.25 * 256
        let back = Arg::fixed_to_float(fixed);
        assert!((back - (-2.25)).abs() < 0.01);
    }

    #[test]
    fn fixed_point_zero() {
        assert_eq!(Arg::float_to_fixed(0.0), 0);
        assert_eq!(Arg::fixed_to_float(0), 0.0);
    }

    #[test]
    fn message_encode_int_args() {
        let msg = WlMessage::new(ObjectId(5), 2, vec![Arg::Int(-42), Arg::Uint(100)]);
        let bytes = msg.encode();
        assert_eq!(bytes.len(), 16); // 8 header + 4 + 4
        let decoded = WlMessage::decode(&bytes, &[ArgType::Int, ArgType::Uint]).unwrap();
        assert_eq!(decoded.header.object_id, ObjectId(5));
        assert_eq!(decoded.header.opcode, 2);
        assert_eq!(decoded.args, vec![Arg::Int(-42), Arg::Uint(100)]);
    }

    #[test]
    fn message_encode_string_arg() {
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::String("wl_surface".into())]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::String]).unwrap();
        assert_eq!(decoded.args[0], Arg::String("wl_surface".into()));
    }

    #[test]
    fn message_encode_empty_string() {
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::String(String::new())]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::String]).unwrap();
        assert_eq!(decoded.args[0], Arg::String(String::new()));
    }

    #[test]
    fn message_encode_array_arg() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x42];
        let msg = WlMessage::new(ObjectId(3), 1, vec![Arg::Array(data.clone())]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::Array]).unwrap();
        assert_eq!(decoded.args[0], Arg::Array(data));
    }

    #[test]
    fn message_encode_new_id_arg() {
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::NewId(ObjectId(99))]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::NewId]).unwrap();
        assert_eq!(decoded.args[0], Arg::NewId(ObjectId(99)));
    }

    #[test]
    fn message_encode_fixed_arg() {
        let fixed = Arg::float_to_fixed(3.75);
        let msg = WlMessage::new(ObjectId(2), 5, vec![Arg::Fixed(fixed)]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::Fixed]).unwrap();
        if let Arg::Fixed(v) = decoded.args[0] {
            assert!((Arg::fixed_to_float(v) - 3.75).abs() < 0.01);
        } else {
            panic!("expected Fixed");
        }
    }

    #[test]
    fn message_encode_fd_arg() {
        let msg = WlMessage::new(
            ObjectId(1),
            0,
            vec![Arg::Int(1), Arg::Fd, Arg::Uint(2)],
        );
        let bytes = msg.encode();
        // Fd takes 0 bytes on wire
        assert_eq!(bytes.len(), 8 + 4 + 0 + 4);
        let decoded =
            WlMessage::decode(&bytes, &[ArgType::Int, ArgType::Fd, ArgType::Uint]).unwrap();
        assert_eq!(decoded.args, vec![Arg::Int(1), Arg::Fd, Arg::Uint(2)]);
    }

    #[test]
    fn message_mixed_args_roundtrip() {
        let args = vec![
            Arg::Uint(1),
            Arg::String("hello".into()),
            Arg::Int(-5),
            Arg::Array(vec![1, 2]),
            Arg::NewId(ObjectId(10)),
        ];
        let types = vec![
            ArgType::Uint,
            ArgType::String,
            ArgType::Int,
            ArgType::Array,
            ArgType::NewId,
        ];
        let msg = WlMessage::new(ObjectId(7), 3, args.clone());
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &types).unwrap();
        assert_eq!(decoded.args, args);
    }

    #[test]
    fn message_no_args() {
        let msg = WlMessage::new(ObjectId(1), 6, vec![]);
        let bytes = msg.encode();
        assert_eq!(bytes.len(), 8);
        let decoded = WlMessage::decode(&bytes, &[]).unwrap();
        assert_eq!(decoded.header.opcode, 6);
        assert!(decoded.args.is_empty());
    }

    #[test]
    fn decode_truncated_body() {
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::Int(42)]);
        let bytes = msg.encode();
        // Truncate: give only header
        assert!(WlMessage::decode(&bytes[..8], &[ArgType::Int]).is_none());
    }

    #[test]
    fn decode_truncated_header() {
        assert!(WlMessage::decode(&[0u8; 4], &[]).is_none());
    }

    #[test]
    fn interface_lookup() {
        let iface = Interface {
            name: "wl_surface".into(),
            version: 6,
            requests: vec![
                MessageDesc {
                    name: "destroy".into(),
                    args: vec![],
                },
                MessageDesc {
                    name: "attach".into(),
                    args: vec![ArgType::NewId, ArgType::Int, ArgType::Int],
                },
            ],
            events: vec![MessageDesc {
                name: "enter".into(),
                args: vec![ArgType::NewId],
            }],
        };
        assert_eq!(iface.request(0).unwrap().name, "destroy");
        assert_eq!(iface.request(1).unwrap().name, "attach");
        assert!(iface.request(2).is_none());
        assert_eq!(iface.event(0).unwrap().name, "enter");
        assert!(iface.event(1).is_none());
    }

    #[test]
    fn align_up_values() {
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(8, 4), 8);
    }

    #[test]
    fn string_with_exact_alignment() {
        // "abcdefg" = 7 chars + NUL = 8 bytes (already aligned)
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::String("abcdefg".into())]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::String]).unwrap();
        assert_eq!(decoded.args[0], Arg::String("abcdefg".into()));
    }

    #[test]
    fn array_empty() {
        let msg = WlMessage::new(ObjectId(1), 0, vec![Arg::Array(vec![])]);
        let bytes = msg.encode();
        let decoded = WlMessage::decode(&bytes, &[ArgType::Array]).unwrap();
        assert_eq!(decoded.args[0], Arg::Array(vec![]));
    }

    #[test]
    fn message_header_max_opcode() {
        let hdr = MessageHeader {
            object_id: ObjectId(1),
            opcode: u16::MAX,
            size: 8,
        };
        let bytes = hdr.encode();
        let decoded = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.opcode, u16::MAX);
    }

    #[test]
    fn message_header_large_object_id() {
        let hdr = MessageHeader {
            object_id: ObjectId(u32::MAX),
            opcode: 0,
            size: 8,
        };
        let bytes = hdr.encode();
        let decoded = MessageHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.object_id, ObjectId(u32::MAX));
    }
}

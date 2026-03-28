//! Message serialization for the IPC message bus.
//!
//! Provides a [`BusValue`] enum that can represent common data types exchanged
//! over the bus, along with `serialize` / `deserialize` functions and D-Bus-style
//! type signature strings.
//!
//! The wire format is intentionally simple:
//! - 1-byte type tag
//! - Payload (fixed-size for scalars, length-prefixed for variable-size)
//!
//! All multi-byte integers are encoded in **little-endian** byte order.

/// A dynamically typed value that can be sent over the message bus.
///
/// Modelled after the D-Bus type system but kept minimal for in-process use.
#[derive(Debug, Clone, PartialEq)]
pub enum BusValue {
    /// Boolean (D-Bus signature: `"b"`).
    Bool(bool),
    /// Signed 32-bit integer (`"i"`).
    Int32(i32),
    /// Signed 64-bit integer (`"x"`).
    Int64(i64),
    /// Unsigned 32-bit integer (`"u"`).
    Uint32(u32),
    /// Unsigned 64-bit integer (`"t"`).
    Uint64(u64),
    /// 64-bit IEEE 754 double (`"d"`).
    Float64(f64),
    /// UTF-8 string (`"s"`).
    String(String),
    /// Raw byte array (`"ay"`).
    ByteArray(Vec<u8>),
    /// Homogeneous array of values (`"a<T>"`).
    Array(Vec<BusValue>),
    /// String-keyed dictionary (`"a{sv}"`).
    Dict(Vec<(String, BusValue)>),
}

// ── Type tags (wire format) ─────────────────────────────────────────────

const TAG_BOOL: u8 = 0x01;
const TAG_INT32: u8 = 0x02;
const TAG_INT64: u8 = 0x03;
const TAG_UINT32: u8 = 0x04;
const TAG_UINT64: u8 = 0x05;
const TAG_FLOAT64: u8 = 0x06;
const TAG_STRING: u8 = 0x07;
const TAG_BYTE_ARRAY: u8 = 0x08;
const TAG_ARRAY: u8 = 0x09;
const TAG_DICT: u8 = 0x0A;

/// Errors that can occur during deserialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeserializeError {
    /// The input buffer is shorter than expected.
    UnexpectedEof,
    /// An unknown type tag was encountered.
    UnknownTag(u8),
    /// A string payload is not valid UTF-8.
    InvalidUtf8,
    /// Trailing bytes remain after a complete value.
    TrailingData,
}

impl std::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of input"),
            Self::UnknownTag(t) => write!(f, "unknown type tag: 0x{t:02X}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string payload"),
            Self::TrailingData => write!(f, "trailing data after value"),
        }
    }
}

impl std::error::Error for DeserializeError {}

// ── Signature strings ───────────────────────────────────────────────────

impl BusValue {
    /// Return the D-Bus-style type signature string for this value.
    ///
    /// Examples: `"b"`, `"i"`, `"s"`, `"ay"`, `"a{sv}"`.
    #[must_use]
    pub fn type_signature(&self) -> String {
        match self {
            Self::Bool(_) => "b".into(),
            Self::Int32(_) => "i".into(),
            Self::Int64(_) => "x".into(),
            Self::Uint32(_) => "u".into(),
            Self::Uint64(_) => "t".into(),
            Self::Float64(_) => "d".into(),
            Self::String(_) => "s".into(),
            Self::ByteArray(_) => "ay".into(),
            Self::Array(items) => {
                if let Some(first) = items.first() {
                    format!("a{}", first.type_signature())
                } else {
                    // Empty array — use variant as inner type.
                    "av".into()
                }
            }
            Self::Dict(_) => "a{sv}".into(),
        }
    }

    /// Returns `true` if the value is a container type (Array or Dict).
    #[must_use]
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Array(_) | Self::Dict(_))
    }

    /// Convenience: try to extract a `&str` if this is a `String` variant.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convenience: try to extract an `i32` if this is an `Int32` variant.
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int32(v) => Some(*v),
            _ => None,
        }
    }

    /// Convenience: try to extract a `bool` if this is a `Bool` variant.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

// ── Serialize ───────────────────────────────────────────────────────────

/// Serialize a [`BusValue`] into a byte vector.
#[must_use]
pub fn serialize(value: &BusValue) -> Vec<u8> {
    let mut buf = Vec::new();
    serialize_into(&mut buf, value);
    buf
}

fn serialize_into(buf: &mut Vec<u8>, value: &BusValue) {
    match value {
        BusValue::Bool(v) => {
            buf.push(TAG_BOOL);
            buf.push(if *v { 1 } else { 0 });
        }
        BusValue::Int32(v) => {
            buf.push(TAG_INT32);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        BusValue::Int64(v) => {
            buf.push(TAG_INT64);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        BusValue::Uint32(v) => {
            buf.push(TAG_UINT32);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        BusValue::Uint64(v) => {
            buf.push(TAG_UINT64);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        BusValue::Float64(v) => {
            buf.push(TAG_FLOAT64);
            buf.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        BusValue::String(s) => {
            buf.push(TAG_STRING);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        BusValue::ByteArray(bytes) => {
            buf.push(TAG_BYTE_ARRAY);
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        BusValue::Array(items) => {
            buf.push(TAG_ARRAY);
            buf.extend_from_slice(&(items.len() as u32).to_le_bytes());
            for item in items {
                serialize_into(buf, item);
            }
        }
        BusValue::Dict(entries) => {
            buf.push(TAG_DICT);
            buf.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for (key, val) in entries {
                let key_bytes = key.as_bytes();
                buf.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
                buf.extend_from_slice(key_bytes);
                serialize_into(buf, val);
            }
        }
    }
}

// ── Deserialize ─────────────────────────────────────────────────────────

/// Deserialize a [`BusValue`] from the given byte slice.
///
/// Returns an error if the bytes are malformed, truncated, or contain
/// trailing data after a complete value.
pub fn deserialize(bytes: &[u8]) -> Result<BusValue, DeserializeError> {
    let (value, rest) = deserialize_one(bytes)?;
    if !rest.is_empty() {
        return Err(DeserializeError::TrailingData);
    }
    Ok(value)
}

/// Deserialize one value, returning the remaining unconsumed bytes.
fn deserialize_one(bytes: &[u8]) -> Result<(BusValue, &[u8]), DeserializeError> {
    let (&tag, rest) = bytes.split_first().ok_or(DeserializeError::UnexpectedEof)?;
    match tag {
        TAG_BOOL => {
            let (&b, rest) = rest.split_first().ok_or(DeserializeError::UnexpectedEof)?;
            Ok((BusValue::Bool(b != 0), rest))
        }
        TAG_INT32 => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let val = i32::from_le_bytes(rest[..4].try_into().unwrap());
            Ok((BusValue::Int32(val), &rest[4..]))
        }
        TAG_INT64 => {
            if rest.len() < 8 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let val = i64::from_le_bytes(rest[..8].try_into().unwrap());
            Ok((BusValue::Int64(val), &rest[8..]))
        }
        TAG_UINT32 => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let val = u32::from_le_bytes(rest[..4].try_into().unwrap());
            Ok((BusValue::Uint32(val), &rest[4..]))
        }
        TAG_UINT64 => {
            if rest.len() < 8 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let val = u64::from_le_bytes(rest[..8].try_into().unwrap());
            Ok((BusValue::Uint64(val), &rest[8..]))
        }
        TAG_FLOAT64 => {
            if rest.len() < 8 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let bits = u64::from_le_bytes(rest[..8].try_into().unwrap());
            Ok((BusValue::Float64(f64::from_bits(bits)), &rest[8..]))
        }
        TAG_STRING => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let rest = &rest[4..];
            if rest.len() < len {
                return Err(DeserializeError::UnexpectedEof);
            }
            let s = std::str::from_utf8(&rest[..len])
                .map_err(|_| DeserializeError::InvalidUtf8)?;
            Ok((BusValue::String(s.to_owned()), &rest[len..]))
        }
        TAG_BYTE_ARRAY => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let rest = &rest[4..];
            if rest.len() < len {
                return Err(DeserializeError::UnexpectedEof);
            }
            Ok((BusValue::ByteArray(rest[..len].to_vec()), &rest[len..]))
        }
        TAG_ARRAY => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let count = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let mut cursor = &rest[4..];
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let (val, remaining) = deserialize_one(cursor)?;
                items.push(val);
                cursor = remaining;
            }
            Ok((BusValue::Array(items), cursor))
        }
        TAG_DICT => {
            if rest.len() < 4 {
                return Err(DeserializeError::UnexpectedEof);
            }
            let count = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let mut cursor = &rest[4..];
            let mut entries = Vec::with_capacity(count);
            for _ in 0..count {
                // Read key
                if cursor.len() < 4 {
                    return Err(DeserializeError::UnexpectedEof);
                }
                let key_len = u32::from_le_bytes(cursor[..4].try_into().unwrap()) as usize;
                cursor = &cursor[4..];
                if cursor.len() < key_len {
                    return Err(DeserializeError::UnexpectedEof);
                }
                let key = std::str::from_utf8(&cursor[..key_len])
                    .map_err(|_| DeserializeError::InvalidUtf8)?
                    .to_owned();
                cursor = &cursor[key_len..];
                // Read value
                let (val, remaining) = deserialize_one(cursor)?;
                entries.push((key, val));
                cursor = remaining;
            }
            Ok((BusValue::Dict(entries), cursor))
        }
        other => Err(DeserializeError::UnknownTag(other)),
    }
}

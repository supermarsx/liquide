use std::fmt;

/// Error returned when accessing extra data bytes at an out-of-bounds offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtraDataError {
    /// The requested offset + size exceeds the allocated byte buffer.
    OutOfBounds {
        offset: usize,
        size: usize,
        capacity: usize,
    },
}

impl fmt::Display for ExtraDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds {
                offset,
                size,
                capacity,
            } => {
                write!(
                    f,
                    "extra data access out of bounds: offset {offset} + size {size} > capacity {capacity}"
                )
            }
        }
    }
}

impl std::error::Error for ExtraDataError {}

/// Raw byte storage for per-window or per-class extra data.
///
/// This is analogous to `cbWndExtra` / `cbClsExtra` in NT — an opaque bag of
/// bytes that the window procedure can read/write through `Get/SetWindowLong`.
#[derive(Debug, Clone)]
pub struct ExtraData {
    bytes: Vec<u8>,
}

impl ExtraData {
    /// Create a zero-initialized buffer of `size` bytes.
    pub fn new(size: usize) -> Self {
        Self {
            bytes: vec![0u8; size],
        }
    }

    /// Total capacity in bytes.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` when the buffer is empty (zero extra bytes requested).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Read an `i64` (8 bytes, little-endian) at the given byte offset.
    pub fn get_long(&self, offset: usize) -> Option<i64> {
        let end = offset.checked_add(8)?;
        if end > self.bytes.len() {
            return None;
        }
        let slice: [u8; 8] = self.bytes[offset..end].try_into().ok()?;
        Some(i64::from_le_bytes(slice))
    }

    /// Write an `i64` (8 bytes, little-endian) at the given byte offset.
    pub fn set_long(&mut self, offset: usize, value: i64) -> Result<(), ExtraDataError> {
        let end = offset.checked_add(8).ok_or(ExtraDataError::OutOfBounds {
            offset,
            size: 8,
            capacity: self.bytes.len(),
        })?;
        if end > self.bytes.len() {
            return Err(ExtraDataError::OutOfBounds {
                offset,
                size: 8,
                capacity: self.bytes.len(),
            });
        }
        self.bytes[offset..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Read a pointer-sized value (`u64`, 8 bytes LE) at the given offset.
    pub fn get_ptr(&self, offset: usize) -> Option<u64> {
        let end = offset.checked_add(8)?;
        if end > self.bytes.len() {
            return None;
        }
        let slice: [u8; 8] = self.bytes[offset..end].try_into().ok()?;
        Some(u64::from_le_bytes(slice))
    }

    /// Write a pointer-sized value (`u64`, 8 bytes LE) at the given offset.
    pub fn set_ptr(&mut self, offset: usize, value: u64) -> Result<(), ExtraDataError> {
        let end = offset.checked_add(8).ok_or(ExtraDataError::OutOfBounds {
            offset,
            size: 8,
            capacity: self.bytes.len(),
        })?;
        if end > self.bytes.len() {
            return Err(ExtraDataError::OutOfBounds {
                offset,
                size: 8,
                capacity: self.bytes.len(),
            });
        }
        self.bytes[offset..end].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Direct immutable access to the raw bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Direct mutable access to the raw bytes.
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

/// Per-class shared extra data — same API surface as [`ExtraData`] but
/// semantically shared across all window instances of the class.
pub type ClassExtraData = ExtraData;

/// Per-window extra data — each window instance has its own copy.
pub type WindowExtraData = ExtraData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zeroed() {
        let ed = ExtraData::new(16);
        assert_eq!(ed.capacity(), 16);
        assert_eq!(ed.get_long(0), Some(0));
        assert_eq!(ed.get_long(8), Some(0));
    }

    #[test]
    fn set_get_long() {
        let mut ed = ExtraData::new(16);
        ed.set_long(0, 0x1234_5678_9ABC_DEF0).unwrap();
        assert_eq!(ed.get_long(0), Some(0x1234_5678_9ABC_DEF0));
    }

    #[test]
    fn set_get_ptr() {
        let mut ed = ExtraData::new(16);
        ed.set_ptr(8, 0xDEAD_BEEF_CAFE_BABE).unwrap();
        assert_eq!(ed.get_ptr(8), Some(0xDEAD_BEEF_CAFE_BABE));
    }

    #[test]
    fn out_of_bounds_long() {
        let mut ed = ExtraData::new(4);
        assert!(ed.set_long(0, 1).is_err());
        assert_eq!(ed.get_long(0), None);
    }

    #[test]
    fn out_of_bounds_ptr() {
        let mut ed = ExtraData::new(4);
        assert!(ed.set_ptr(0, 1).is_err());
        assert_eq!(ed.get_ptr(0), None);
    }

    #[test]
    fn empty_extra_data() {
        let ed = ExtraData::new(0);
        assert!(ed.is_empty());
        assert_eq!(ed.get_long(0), None);
    }

    #[test]
    fn overlapping_writes() {
        let mut ed = ExtraData::new(16);
        ed.set_long(0, -1).unwrap();
        ed.set_long(4, 0).unwrap();
        // First write put 0xFF..FF at [0..8], second put 0x00..00 at [4..12].
        // Bytes [0..4] should still be 0xFF, [4..8] now 0x00.
        let val = ed.get_long(0).unwrap();
        assert_eq!(val, 0x0000_0000_FFFF_FFFF_u64 as i64);
    }

    #[test]
    fn raw_bytes_access() {
        let mut ed = ExtraData::new(8);
        ed.as_bytes_mut()[0] = 0xAB;
        assert_eq!(ed.as_bytes()[0], 0xAB);
    }

    #[test]
    fn error_display() {
        let err = ExtraDataError::OutOfBounds {
            offset: 10,
            size: 8,
            capacity: 12,
        };
        let msg = format!("{err}");
        assert!(msg.contains("out of bounds"));
    }
}

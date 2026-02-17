//! Message fragmentation and reassembly.
//!
//! Messages larger than 65535 bytes must be fragmented into multiple frames.
//! The first fragment has the FRAGMENTED flag set and a 4-byte fragment_total
//! prepended to the payload. Middle fragments have FRAGMENTED set. The last
//! fragment has the FRAGMENTED flag cleared.

use std::collections::HashMap;

use bytes::{BufMut, Bytes, BytesMut};

use crate::frame::{FrameFlags, FrameHeader};

/// Maximum payload per fragment frame.
pub const MAX_FRAGMENT_PAYLOAD: usize = u16::MAX as usize;

/// Maximum number of concurrent reassembly operations per reassembler.
pub const MAX_PENDING_REASSEMBLIES: usize = 256;

/// Maximum number of fragments allowed per message.
pub const MAX_FRAGMENTS_PER_MESSAGE: u32 = 1024;

/// Maximum total reassembled message size (64 MiB).
pub const MAX_REASSEMBLED_SIZE: usize = 64 * 1024 * 1024;

/// Fragment a large payload into multiple frame-sized chunks.
///
/// Returns a list of `(flags_to_add, payload_chunk)` pairs. The caller
/// is responsible for creating `FrameHeader`s with the appropriate
/// sequence numbers and adding the returned flags.
pub fn fragment(data: &[u8], max_payload: usize) -> Vec<(u8, Bytes)> {
    if data.len() <= max_payload {
        // No fragmentation needed
        return vec![(0, Bytes::copy_from_slice(data))];
    }

    let first_payload_max = max_payload - 4; // Reserve 4 bytes for fragment_total
    let first_end = first_payload_max.min(data.len());
    let remaining = data.len() - first_end;
    // total_fragments = 1 (first) + ceil(remaining / max_payload)
    let tail_count = (remaining + max_payload - 1) / max_payload;
    let total_fragments = 1 + tail_count;

    let mut fragments = Vec::with_capacity(total_fragments);

    // First fragment: prepend fragment_total as 4 bytes
    let mut first_buf = BytesMut::with_capacity(4 + first_end);
    first_buf.put_u32(total_fragments as u32);
    first_buf.put_slice(&data[..first_end]);
    fragments.push((FrameFlags::FRAGMENTED, first_buf.freeze()));

    let mut offset = first_end;

    // Middle fragments
    while offset + max_payload < data.len() {
        let chunk = Bytes::copy_from_slice(&data[offset..offset + max_payload]);
        fragments.push((FrameFlags::FRAGMENTED, chunk));
        offset += max_payload;
    }

    // Last fragment (no FRAGMENTED flag)
    if offset < data.len() {
        let chunk = Bytes::copy_from_slice(&data[offset..]);
        fragments.push((0, chunk)); // No FRAGMENTED flag on last
    }

    fragments
}

/// Reassembles fragmented frames back into a complete message.
///
/// On reliable channels, fragments arrive in order. Reassembly is
/// tracked per channel: the first fragment (with `FRAGMENTED` flag and
/// a 4-byte total prepended) starts a new reassembly, and the final
/// fragment (without `FRAGMENTED` flag) completes it.
///
/// **Security Limits:**
/// - Maximum [`MAX_PENDING_REASSEMBLIES`] concurrent reassemblies
/// - Maximum [`MAX_FRAGMENTS_PER_MESSAGE`] fragments per message
/// - Maximum [`MAX_REASSEMBLED_SIZE`] bytes per reassembled message
#[derive(Debug, Default)]
pub struct Reassembler {
    /// In-progress reassembly keyed by channel ID.
    pending: HashMap<u16, ReassemblyState>,
    /// Current total bytes held across all pending reassemblies.
    current_bytes: usize,
}

#[derive(Debug)]
struct ReassemblyState {
    total_fragments: u32,
    fragments: Vec<Bytes>,
    /// Accumulated byte count for this reassembly.
    accumulated_bytes: usize,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a frame into the reassembler.
    ///
    /// Returns `Some(complete_payload)` when the last fragment arrives
    /// and the message is fully reassembled. Returns `None` if more
    /// fragments are needed or if security limits are exceeded.
    pub fn feed(&mut self, header: &FrameHeader, payload: Bytes) -> Option<Bytes> {
        let ch = header.channel.as_u16();

        if header.is_fragmented() {
            if !self.pending.contains_key(&ch) {
                // First fragment: extract total from first 4 bytes
                if payload.len() < 4 {
                    return None;
                }
                
                // Check if we've hit the max pending limit
                if self.pending.len() >= MAX_PENDING_REASSEMBLIES {
                    // Reject new reassemblies when at capacity
                    return None;
                }
                
                let total =
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                
                // Reject messages with too many fragments
                if total > MAX_FRAGMENTS_PER_MESSAGE || total == 0 {
                    return None;
                }
                
                let data = payload.slice(4..);
                let data_len = data.len();
                
                self.pending.insert(
                    ch,
                    ReassemblyState {
                        total_fragments: total,
                        fragments: vec![data],
                        accumulated_bytes: data_len,
                    },
                );
                self.current_bytes += data_len;
            } else {
                // Middle fragment
                let state = self.pending.get_mut(&ch).unwrap();
                let payload_len = payload.len();
                
                // Check if we'd exceed max reassembled size
                if state.accumulated_bytes + payload_len > MAX_REASSEMBLED_SIZE {
                    // Drop this reassembly - it's too large
                    self.current_bytes -= state.accumulated_bytes;
                    self.pending.remove(&ch);
                    return None;
                }
                
                // Check fragment count
                if state.fragments.len() >= state.total_fragments as usize {
                    // More fragments than expected - protocol error
                    self.current_bytes -= state.accumulated_bytes;
                    self.pending.remove(&ch);
                    return None;
                }
                
                state.fragments.push(payload);
                state.accumulated_bytes += payload_len;
                self.current_bytes += payload_len;
            }
            None
        } else if let Some(mut state) = self.pending.remove(&ch) {
            // Last fragment (no FRAGMENTED flag)
            let payload_len = payload.len();
            
            // Final size check
            if state.accumulated_bytes + payload_len > MAX_REASSEMBLED_SIZE {
                self.current_bytes -= state.accumulated_bytes;
                return None;
            }
            
            self.current_bytes -= state.accumulated_bytes;
            state.fragments.push(payload);
            
            let total_len: usize = state.fragments.iter().map(|b| b.len()).sum();
            let mut buf = BytesMut::with_capacity(total_len);
            for chunk in state.fragments {
                buf.put(chunk);
            }
            Some(buf.freeze())
        } else {
            // Not fragmented at all - return as-is
            Some(payload)
        }
    }

    /// Remove any pending reassembly for the given channel.
    /// Call periodically to prevent memory leaks from lost fragments.
    pub fn expire(&mut self, channel_id: u16) {
        if let Some(state) = self.pending.remove(&channel_id) {
            self.current_bytes -= state.accumulated_bytes;
        }
    }

    /// Number of in-progress reassemblies.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    
    /// Current memory usage across all pending reassemblies.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }
}

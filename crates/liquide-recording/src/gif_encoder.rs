//! Simple GIF89a encoder with color quantization and LZW compression.
//!
//! Produces animated GIFs from a sequence of RGBA frames. Uses uniform
//! quantization to 256 colors and variable-length LZW compression per
//! the GIF specification.

/// A simple animated GIF encoder.
pub struct GifEncoder {
    width: u16,
    height: u16,
    /// Delay between frames in centiseconds (100ths of a second).
    delay_cs: u16,
    /// Accumulated GIF file bytes (header already written).
    output: Vec<u8>,
    frame_count: u32,
    finished: bool,
}

impl GifEncoder {
    /// Create a new GIF encoder.
    ///
    /// `framerate` is in frames per second; it is converted to the GIF
    /// delay field (centiseconds per frame).
    #[must_use]
    pub fn new(width: u16, height: u16, framerate: u16) -> Self {
        let delay_cs = if framerate > 0 { 100 / framerate } else { 10 };
        let delay_cs = delay_cs.max(2); // GIF minimum practical delay

        let mut output = Vec::with_capacity(1024 * 64);

        // --- GIF89a Header ---
        output.extend_from_slice(b"GIF89a");

        // --- Logical Screen Descriptor ---
        output.extend_from_slice(&width.to_le_bytes());
        output.extend_from_slice(&height.to_le_bytes());
        // Packed: global color table flag=1, color resolution=7 (8 bits),
        // sort=0, size of GCT = 7 (2^(7+1) = 256 entries)
        output.push(0b1_111_0_111);
        output.push(0); // background color index
        output.push(0); // pixel aspect ratio

        // --- Global Color Table (256 entries, RGB) ---
        // Write a uniform 6x6x6 color cube + 40 grays
        write_uniform_color_table(&mut output);

        // --- NETSCAPE2.0 Application Extension for looping ---
        output.push(0x21); // extension introducer
        output.push(0xFF); // application extension label
        output.push(11); // block size
        output.extend_from_slice(b"NETSCAPE2.0");
        output.push(3); // sub-block size
        output.push(1); // sub-block ID
        output.extend_from_slice(&0u16.to_le_bytes()); // loop count 0 = infinite
        output.push(0); // block terminator

        Self {
            width,
            height,
            delay_cs,
            output,
            frame_count: 0,
            finished: false,
        }
    }

    /// Add a frame from RGBA pixel data.
    ///
    /// The data must be exactly `width * height * 4` bytes (RGBA).
    /// The frame is quantized to the 256-color global palette and
    /// LZW-compressed.
    pub fn add_frame(&mut self, rgba_data: &[u8]) {
        if self.finished {
            return;
        }
        let expected = self.width as usize * self.height as usize * 4;
        if rgba_data.len() < expected {
            return;
        }

        // --- Graphic Control Extension ---
        self.output.push(0x21); // extension introducer
        self.output.push(0xF9); // graphic control label
        self.output.push(4); // block size
        // Packed: disposal=none(0), user input=0, transparent=0
        self.output.push(0x00);
        self.output.extend_from_slice(&self.delay_cs.to_le_bytes());
        self.output.push(0); // transparent color index (unused)
        self.output.push(0); // block terminator

        // --- Image Descriptor ---
        self.output.push(0x2C); // image separator
        self.output.extend_from_slice(&0u16.to_le_bytes()); // left
        self.output.extend_from_slice(&0u16.to_le_bytes()); // top
        self.output.extend_from_slice(&self.width.to_le_bytes());
        self.output.extend_from_slice(&self.height.to_le_bytes());
        self.output.push(0x00); // packed: no local color table, not interlaced

        // --- Quantize RGBA to palette indices ---
        let pixel_count = self.width as usize * self.height as usize;
        let mut indices = Vec::with_capacity(pixel_count);
        for i in 0..pixel_count {
            let r = rgba_data[i * 4];
            let g = rgba_data[i * 4 + 1];
            let b = rgba_data[i * 4 + 2];
            indices.push(quantize_to_uniform_palette(r, g, b));
        }

        // --- LZW compress ---
        let min_code_size: u8 = 8; // 256-color palette
        self.output.push(min_code_size);
        let compressed = lzw_compress(&indices, min_code_size);
        // Write in sub-blocks of max 255 bytes
        for chunk in compressed.chunks(255) {
            self.output.push(chunk.len() as u8);
            self.output.extend_from_slice(chunk);
        }
        self.output.push(0); // block terminator

        self.frame_count += 1;
    }

    /// Finish the GIF and return the complete file bytes.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        if !self.finished {
            self.output.push(0x3B); // GIF trailer
            self.finished = true;
        }
        self.output
    }

    /// Number of frames added so far.
    #[must_use]
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Current accumulated output size in bytes (not yet finalised).
    #[must_use]
    pub fn current_size(&self) -> usize {
        self.output.len()
    }

    /// The configured delay between frames in centiseconds.
    #[must_use]
    pub fn delay_cs(&self) -> u16 {
        self.delay_cs
    }
}

impl std::fmt::Display for GifEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GifEncoder({}x{}, frames={}, {} bytes)",
            self.width,
            self.height,
            self.frame_count,
            self.output.len()
        )
    }
}

/// Write a uniform 256-color palette (6x6x6 color cube + 40 grays) to output.
fn write_uniform_color_table(output: &mut Vec<u8>) {
    // Indices 0..215: 6x6x6 color cube
    for r_idx in 0u8..6 {
        for g_idx in 0u8..6 {
            for b_idx in 0u8..6 {
                output.push(r_idx * 51);
                output.push(g_idx * 51);
                output.push(b_idx * 51);
            }
        }
    }
    // Indices 216..255: 40 gray levels
    for i in 0u8..40 {
        let v = (i as u16 * 255 / 39) as u8;
        output.push(v);
        output.push(v);
        output.push(v);
    }
}

/// Map an RGB color to the nearest index in the uniform 256-color palette.
fn quantize_to_uniform_palette(r: u8, g: u8, b: u8) -> u8 {
    // Check if it's close to gray (all channels within 12 of each other)
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    if max_c - min_c < 12 {
        // Use the gray ramp (indices 216..255)
        let avg = ((r as u16 + g as u16 + b as u16) / 3) as u8;
        let gray_idx = ((avg as u16) * 39 / 255) as u8;
        return 216 + gray_idx;
    }
    // Use the 6x6x6 color cube (indices 0..215)
    let ri = ((r as u16 + 25) / 51).min(5) as u8;
    let gi = ((g as u16 + 25) / 51).min(5) as u8;
    let bi = ((b as u16 + 25) / 51).min(5) as u8;
    ri * 36 + gi * 6 + bi
}

/// LZW compression for GIF.
///
/// Implements variable-width code LZW as specified in the GIF standard.
/// `min_code_size` is typically 8 for 256-color images.
fn lzw_compress(indices: &[u8], min_code_size: u8) -> Vec<u8> {
    let clear_code: u16 = 1 << min_code_size;
    let eoi_code: u16 = clear_code + 1;

    let mut writer = BitWriter::new();
    let mut code_size = min_code_size as u16 + 1;
    let max_table_size: u16 = 4096;

    // Initialize dictionary with single-character entries
    let mut table: Vec<(u16, u8)> = Vec::new(); // (prefix_code, suffix_byte)
    let mut next_code: u16 = eoi_code + 1;

    // Emit clear code
    writer.write_bits(clear_code, code_size);

    if indices.is_empty() {
        writer.write_bits(eoi_code, code_size);
        return writer.finish();
    }

    // We use a hash-based dictionary for fast prefix+suffix lookups.
    // Key = (prefix_code, suffix_byte), Value = code
    let mut dict = LzwDict::new();

    let mut prefix_code: u16 = indices[0] as u16;

    for &byte in &indices[1..] {
        if let Some(code) = dict.get(prefix_code, byte) {
            prefix_code = code;
        } else {
            // Emit the prefix code
            writer.write_bits(prefix_code, code_size);

            // Add new entry to dictionary
            if next_code < max_table_size {
                dict.insert(prefix_code, byte, next_code);
                table.push((prefix_code, byte));
                next_code += 1;

                // Increase code size if needed
                if next_code > (1 << code_size) && code_size < 12 {
                    code_size += 1;
                }
            } else {
                // Table full — emit clear code and reset
                writer.write_bits(clear_code, code_size);
                dict.clear();
                table.clear();
                next_code = eoi_code + 1;
                code_size = min_code_size as u16 + 1;
            }

            prefix_code = byte as u16;
        }
    }

    // Emit final prefix code
    writer.write_bits(prefix_code, code_size);
    // Emit end-of-information code
    writer.write_bits(eoi_code, code_size);

    writer.finish()
}

/// A simple hash-based LZW dictionary.
struct LzwDict {
    /// Hash table: each slot is (prefix, suffix, code) or empty.
    /// Uses open addressing with linear probing.
    slots: Vec<(u16, u8, u16)>,
    mask: usize,
    count: usize,
}

impl LzwDict {
    fn new() -> Self {
        let size = 8192; // power of 2, larger than max 4096 entries
        Self {
            slots: vec![(0xFFFF, 0, 0); size],
            mask: size - 1,
            count: 0,
        }
    }

    fn hash(prefix: u16, suffix: u8) -> usize {
        // Simple hash combining prefix and suffix
        let h = (prefix as usize) ^ ((suffix as usize) << 5) ^ ((suffix as usize) << 11);
        h
    }

    fn get(&self, prefix: u16, suffix: u8) -> Option<u16> {
        let mut idx = Self::hash(prefix, suffix) & self.mask;
        loop {
            let (p, s, c) = self.slots[idx];
            if p == 0xFFFF {
                return None;
            }
            if p == prefix && s == suffix {
                return Some(c);
            }
            idx = (idx + 1) & self.mask;
        }
    }

    fn insert(&mut self, prefix: u16, suffix: u8, code: u16) {
        let mut idx = Self::hash(prefix, suffix) & self.mask;
        loop {
            if self.slots[idx].0 == 0xFFFF {
                self.slots[idx] = (prefix, suffix, code);
                self.count += 1;
                return;
            }
            idx = (idx + 1) & self.mask;
        }
    }

    fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.0 = 0xFFFF;
        }
        self.count = 0;
    }
}

/// Bit writer that packs variable-width codes into a byte stream (LSB first).
struct BitWriter {
    buffer: Vec<u8>,
    current_byte: u32,
    bits_in_byte: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
            current_byte: 0,
            bits_in_byte: 0,
        }
    }

    fn write_bits(&mut self, code: u16, code_size: u16) {
        let mut code = code as u32;
        let mut remaining = code_size as u8;

        while remaining > 0 {
            let space = 8 - self.bits_in_byte;
            let to_write = remaining.min(space);
            self.current_byte |= (code & ((1 << to_write) - 1)) << self.bits_in_byte;
            self.bits_in_byte += to_write;
            code >>= to_write;
            remaining -= to_write;

            if self.bits_in_byte == 8 {
                self.buffer.push(self.current_byte as u8);
                self.current_byte = 0;
                self.bits_in_byte = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits_in_byte > 0 {
            self.buffer.push(self.current_byte as u8);
        }
        self.buffer
    }
}

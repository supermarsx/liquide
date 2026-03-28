/// Programmatic WAV file generation for built-in sound effects.
///
/// All output is 16-bit PCM mono at 44100 Hz, wrapped in a standard
/// 44-byte RIFF/WAVE header.

const SAMPLE_RATE: u32 = 44100;
const BITS_PER_SAMPLE: u16 = 16;
const NUM_CHANNELS: u16 = 1;

/// Write a complete WAV file header for PCM data of the given byte length.
fn wav_header(data_len: u32) -> [u8; 44] {
    let byte_rate = SAMPLE_RATE * (NUM_CHANNELS as u32) * (BITS_PER_SAMPLE as u32 / 8);
    let block_align = NUM_CHANNELS * (BITS_PER_SAMPLE / 8);
    let file_size = 36 + data_len; // total - 8 bytes for RIFF header

    let mut h = [0u8; 44];

    // RIFF chunk descriptor
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&file_size.to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");

    // fmt sub-chunk
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes()); // sub-chunk size (PCM = 16)
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // audio format (1 = PCM)
    h[22..24].copy_from_slice(&NUM_CHANNELS.to_le_bytes());
    h[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
    h[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    h[32..34].copy_from_slice(&block_align.to_le_bytes());
    h[34..36].copy_from_slice(&BITS_PER_SAMPLE.to_le_bytes());

    // data sub-chunk
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_len.to_le_bytes());

    h
}

/// Number of samples for a given duration in milliseconds.
fn sample_count(duration_ms: u32) -> usize {
    ((SAMPLE_RATE as u64 * duration_ms as u64) / 1000) as usize
}

/// Clamp a floating-point sample to i16 range and return little-endian bytes.
fn sample_to_le(sample: f32) -> [u8; 2] {
    let clamped = sample.clamp(-1.0, 1.0);
    let val = (clamped * 32767.0) as i16;
    val.to_le_bytes()
}

/// Generate a pure sine-wave tone as a WAV byte buffer.
///
/// - `frequency_hz`: tone frequency (e.g. 440.0 for concert A)
/// - `duration_ms`: length in milliseconds
/// - `volume`: amplitude 0.0 (silent) to 1.0 (full scale)
///
/// Returns a complete WAV file as `Vec<u8>`.
pub fn generate_beep(frequency_hz: f32, duration_ms: u32, volume: f32) -> Vec<u8> {
    let vol = volume.clamp(0.0, 1.0);
    let num_samples = sample_count(duration_ms);
    let data_len = (num_samples * 2) as u32;
    let header = wav_header(data_len);

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(&header);

    let two_pi = std::f32::consts::PI * 2.0;
    let sample_rate_f = SAMPLE_RATE as f32;

    // Apply a short fade-in/fade-out envelope to avoid clicks.
    let fade_samples = (sample_rate_f * 0.005) as usize; // 5ms fade

    for i in 0..num_samples {
        let t = i as f32 / sample_rate_f;
        let raw = (two_pi * frequency_hz * t).sin() * vol;

        // Envelope: linear fade in/out
        let envelope = if i < fade_samples {
            i as f32 / fade_samples as f32
        } else if i >= num_samples - fade_samples {
            (num_samples - 1 - i) as f32 / fade_samples as f32
        } else {
            1.0
        };

        buf.extend_from_slice(&sample_to_le(raw * envelope));
    }

    buf
}

/// Generate a multi-tone chime: each frequency in `frequencies` is
/// played simultaneously (additive synthesis), normalized to avoid
/// clipping.
///
/// - `frequencies`: slice of frequencies in Hz
/// - `duration_ms`: total duration in milliseconds
///
/// Returns a complete WAV file as `Vec<u8>`.
pub fn generate_chime(frequencies: &[f32], duration_ms: u32) -> Vec<u8> {
    if frequencies.is_empty() {
        return generate_beep(440.0, duration_ms, 0.0);
    }

    let num_samples = sample_count(duration_ms);
    let data_len = (num_samples * 2) as u32;
    let header = wav_header(data_len);

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(&header);

    let two_pi = std::f32::consts::PI * 2.0;
    let sample_rate_f = SAMPLE_RATE as f32;
    let norm = 1.0 / frequencies.len() as f32;

    // Exponential decay envelope for a bell-like chime.
    let decay_rate = 5.0 / (duration_ms as f32 / 1000.0);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate_f;
        let envelope = (-decay_rate * t).exp();

        let mut sample = 0.0f32;
        for &freq in frequencies {
            sample += (two_pi * freq * t).sin();
        }
        sample *= norm * envelope;

        buf.extend_from_slice(&sample_to_le(sample));
    }

    buf
}

/// Generate a short click/tick sound (a single impulse with rapid decay).
///
/// - `duration_ms`: total duration (typically 5-20ms)
///
/// Returns a complete WAV file as `Vec<u8>`.
pub fn generate_click(duration_ms: u32) -> Vec<u8> {
    let num_samples = sample_count(duration_ms);
    let data_len = (num_samples * 2) as u32;
    let header = wav_header(data_len);

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(&header);

    // A click is modeled as a short burst of filtered noise with
    // exponential decay — produces a satisfying "tick" sound.
    let sample_rate_f = SAMPLE_RATE as f32;
    let decay_rate = 20.0 / (duration_ms as f32 / 1000.0);

    // Simple linear-feedback pseudo-random noise (deterministic so
    // the output is reproducible across runs).
    let mut lfsr: u32 = 0xACE1;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate_f;
        let envelope = (-decay_rate * t).exp();

        // Galois LFSR for noise
        let bit = lfsr & 1;
        lfsr >>= 1;
        if bit == 1 {
            lfsr ^= 0xB400;
        }
        // Map to -1..1
        let noise = (lfsr as f32 / 0xFFFF as f32) * 2.0 - 1.0;

        let sample = noise * envelope * 0.8;
        buf.extend_from_slice(&sample_to_le(sample));
    }

    buf
}

/// Generate a rising two-tone alert (for errors/warnings).
///
/// Produces a short tone that sweeps from `freq_start` to `freq_end`.
///
/// - `freq_start`: starting frequency in Hz
/// - `freq_end`: ending frequency in Hz
/// - `duration_ms`: total duration in milliseconds
/// - `volume`: amplitude 0.0 to 1.0
///
/// Returns a complete WAV file as `Vec<u8>`.
pub fn generate_sweep(freq_start: f32, freq_end: f32, duration_ms: u32, volume: f32) -> Vec<u8> {
    let vol = volume.clamp(0.0, 1.0);
    let num_samples = sample_count(duration_ms);
    let data_len = (num_samples * 2) as u32;
    let header = wav_header(data_len);

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(&header);

    let two_pi = std::f32::consts::PI * 2.0;
    let sample_rate_f = SAMPLE_RATE as f32;
    let fade_samples = (sample_rate_f * 0.003) as usize; // 3ms fade

    let mut phase: f32 = 0.0;

    for i in 0..num_samples {
        let frac = i as f32 / num_samples as f32;
        let freq = freq_start + (freq_end - freq_start) * frac;

        phase += two_pi * freq / sample_rate_f;
        if phase > two_pi {
            phase -= two_pi;
        }

        let raw = phase.sin() * vol;

        let envelope = if i < fade_samples {
            i as f32 / fade_samples as f32
        } else if i >= num_samples - fade_samples {
            (num_samples - 1 - i) as f32 / fade_samples as f32
        } else {
            1.0
        };

        buf.extend_from_slice(&sample_to_le(raw * envelope));
    }

    buf
}

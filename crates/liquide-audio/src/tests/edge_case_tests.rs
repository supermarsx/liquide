use crate::buffer::*;
use crate::format::*;

#[test]
fn zero_size_audio_buffer() {
    let fmt = AudioFormat::new(
        SampleFormat::F32,
        SampleRate::Hz48000,
        ChannelLayout::Stereo,
    );
    let buf = AudioBuffer::from_silence(fmt, 0);
    assert_eq!(buf.data.len(), 0);
    assert_eq!(buf.frame_count(), 0);
    assert_eq!(buf.duration_us(), 0);
}

#[test]
fn empty_ring_read_returns_error() {
    let fmt = AudioFormat::new(SampleFormat::I16, SampleRate::Hz44100, ChannelLayout::Mono);
    let mut ring = AudioRingBuffer::new(64, fmt);
    let mut out = vec![0u8; 8];
    assert!(ring.read(&mut out).is_err());
}

#[test]
fn ring_buffer_max_capacity_fill() {
    let fmt = AudioFormat::new(SampleFormat::U8, SampleRate::Hz8000, ChannelLayout::Mono);
    let mut ring = AudioRingBuffer::new(256, fmt);

    // Fill to capacity
    let data = vec![0xABu8; 256];
    ring.write(&data).unwrap();
    assert!(ring.is_full());
    assert_eq!(ring.available(), 256);
    assert_eq!(ring.free_space(), 0);

    // Read it all back
    let mut out = vec![0u8; 256];
    let n = ring.read(&mut out).unwrap();
    assert_eq!(n, 256);
    assert_eq!(out, data);
    assert!(ring.is_empty());
}

#[test]
fn serde_roundtrip_all_sample_formats() {
    for fmt in [SampleFormat::I16, SampleFormat::F32, SampleFormat::U8] {
        let json = serde_json::to_string(&fmt).unwrap();
        let back: SampleFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(fmt, back);
    }
}

#[test]
fn serde_roundtrip_all_sample_rates() {
    for rate in [
        SampleRate::Hz8000,
        SampleRate::Hz16000,
        SampleRate::Hz22050,
        SampleRate::Hz44100,
        SampleRate::Hz48000,
        SampleRate::Hz96000,
    ] {
        let json = serde_json::to_string(&rate).unwrap();
        let back: SampleRate = serde_json::from_str(&json).unwrap();
        assert_eq!(rate, back);
    }
}

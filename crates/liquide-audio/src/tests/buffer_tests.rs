use crate::buffer::*;
use crate::format::*;

fn stereo_f32() -> AudioFormat {
    AudioFormat::new(
        SampleFormat::F32,
        SampleRate::Hz48000,
        ChannelLayout::Stereo,
    )
}

// ========== AudioBuffer ==========

#[test]
fn audio_buffer_new() {
    let fmt = stereo_f32();
    let data = vec![1u8; 16];
    let buf = AudioBuffer::new(fmt, data.clone());
    assert_eq!(buf.data, data);
    assert_eq!(buf.format, fmt);
    assert_eq!(buf.timestamp_us, 0);
}

#[test]
fn audio_buffer_from_silence() {
    let fmt = stereo_f32();
    let buf = AudioBuffer::from_silence(fmt, 100);
    // 100 frames * 8 bytes/frame = 800
    assert_eq!(buf.data.len(), 800);
    assert!(buf.data.iter().all(|&b| b == 0));
}

#[test]
fn audio_buffer_frame_count() {
    let fmt = stereo_f32();
    // 8 bytes/frame, 24 bytes total = 3 frames
    let buf = AudioBuffer::new(fmt, vec![0u8; 24]);
    assert_eq!(buf.frame_count(), 3);
}

#[test]
fn audio_buffer_duration() {
    let fmt = stereo_f32();
    // 384000 bytes/sec, 384 bytes = 1000 us (1ms)
    let buf = AudioBuffer::new(fmt, vec![0u8; 384_000]);
    assert_eq!(buf.duration_us(), 1_000_000);
}

#[test]
fn audio_buffer_display() {
    let fmt = stereo_f32();
    let buf = AudioBuffer::from_silence(fmt, 10);
    let s = format!("{buf}");
    assert!(s.contains("AudioBuffer"));
    assert!(s.contains("80 bytes"));
}

// ========== AudioRingBuffer ==========

#[test]
fn ring_buffer_write_read() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(128, fmt);
    let data = vec![42u8; 64];
    let written = ring.write(&data).unwrap();
    assert_eq!(written, 64);
    assert_eq!(ring.available(), 64);
    assert_eq!(ring.free_space(), 64);

    let mut out = vec![0u8; 64];
    let read = ring.read(&mut out).unwrap();
    assert_eq!(read, 64);
    assert_eq!(out, data);
    assert!(ring.is_empty());
}

#[test]
fn ring_buffer_overflow() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(16, fmt);
    let data = vec![1u8; 20];
    let result = ring.write(&data);
    assert!(result.is_err());
}

#[test]
fn ring_buffer_underrun() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(16, fmt);
    let mut out = vec![0u8; 8];
    let result = ring.read(&mut out);
    assert!(result.is_err());
}

#[test]
fn ring_buffer_wrap_around() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(16, fmt);

    // Write 10 bytes, read 10, write 12 (wraps around)
    let data1 = vec![1u8; 10];
    ring.write(&data1).unwrap();
    let mut out = vec![0u8; 10];
    ring.read(&mut out).unwrap();
    assert_eq!(out, data1);

    let data2 = vec![2u8; 12];
    ring.write(&data2).unwrap();
    let mut out2 = vec![0u8; 12];
    ring.read(&mut out2).unwrap();
    assert_eq!(out2, data2);
}

#[test]
fn ring_buffer_clear() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(64, fmt);
    ring.write(&[1u8; 32]).unwrap();
    assert!(!ring.is_empty());
    ring.clear();
    assert!(ring.is_empty());
    assert_eq!(ring.available(), 0);
    assert_eq!(ring.free_space(), 64);
}

#[test]
fn ring_buffer_full_and_empty() {
    let fmt = stereo_f32();
    let mut ring = AudioRingBuffer::new(8, fmt);
    assert!(ring.is_empty());
    assert!(!ring.is_full());

    ring.write(&[1u8; 8]).unwrap();
    assert!(ring.is_full());
    assert!(!ring.is_empty());
}

#[test]
fn ring_buffer_capacity_and_format() {
    let fmt = stereo_f32();
    let ring = AudioRingBuffer::new(256, fmt);
    assert_eq!(ring.capacity(), 256);
    assert_eq!(*ring.format(), fmt);
}

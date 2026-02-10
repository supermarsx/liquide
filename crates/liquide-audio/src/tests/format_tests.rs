use crate::format::*;

// ========== SampleFormat ==========

#[test]
fn sample_format_byte_sizes() {
    assert_eq!(SampleFormat::I16.byte_size(), 2);
    assert_eq!(SampleFormat::F32.byte_size(), 4);
    assert_eq!(SampleFormat::U8.byte_size(), 1);
}

#[test]
fn sample_format_display() {
    assert_eq!(format!("{}", SampleFormat::I16), "I16");
    assert_eq!(format!("{}", SampleFormat::F32), "F32");
    assert_eq!(format!("{}", SampleFormat::U8), "U8");
}

// ========== SampleRate ==========

#[test]
fn sample_rate_hz_values() {
    assert_eq!(SampleRate::Hz8000.hz(), 8_000);
    assert_eq!(SampleRate::Hz16000.hz(), 16_000);
    assert_eq!(SampleRate::Hz22050.hz(), 22_050);
    assert_eq!(SampleRate::Hz44100.hz(), 44_100);
    assert_eq!(SampleRate::Hz48000.hz(), 48_000);
    assert_eq!(SampleRate::Hz96000.hz(), 96_000);
}

#[test]
fn sample_rate_count() {
    assert_eq!(SampleRate::count(), 6);
}

#[test]
fn sample_rate_display() {
    assert_eq!(format!("{}", SampleRate::Hz48000), "48000Hz");
    assert_eq!(format!("{}", SampleRate::Hz44100), "44100Hz");
}

// ========== ChannelLayout ==========

#[test]
fn channel_layout_counts() {
    assert_eq!(ChannelLayout::Mono.channel_count(), 1);
    assert_eq!(ChannelLayout::Stereo.channel_count(), 2);
    assert_eq!(ChannelLayout::Surround51.channel_count(), 6);
}

// ========== AudioFormat ==========

#[test]
fn audio_format_frame_size() {
    let fmt = AudioFormat::new(SampleFormat::F32, SampleRate::Hz48000, ChannelLayout::Stereo);
    // 2 channels * 4 bytes = 8
    assert_eq!(fmt.frame_size(), 8);

    let mono_i16 = AudioFormat::new(SampleFormat::I16, SampleRate::Hz44100, ChannelLayout::Mono);
    // 1 channel * 2 bytes = 2
    assert_eq!(mono_i16.frame_size(), 2);
}

#[test]
fn audio_format_byte_rate() {
    let fmt = AudioFormat::new(SampleFormat::F32, SampleRate::Hz48000, ChannelLayout::Stereo);
    // 8 bytes/frame * 48000 frames/s = 384000
    assert_eq!(fmt.byte_rate(), 384_000);
}

#[test]
fn audio_format_duration_us() {
    let fmt = AudioFormat::new(SampleFormat::F32, SampleRate::Hz48000, ChannelLayout::Stereo);
    // 384000 bytes = 1 second = 1_000_000 us
    assert_eq!(fmt.duration_us(384_000), 1_000_000);

    // Half a second
    assert_eq!(fmt.duration_us(192_000), 500_000);
}

#[test]
fn audio_format_serde_roundtrip() {
    let fmt = AudioFormat::new(SampleFormat::I16, SampleRate::Hz44100, ChannelLayout::Stereo);
    let json = serde_json::to_string(&fmt).unwrap();
    let back: AudioFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(fmt, back);
}

#[test]
fn audio_format_display() {
    let fmt = AudioFormat::new(SampleFormat::F32, SampleRate::Hz48000, ChannelLayout::Stereo);
    let s = format!("{fmt}");
    assert!(s.contains("AudioFormat"));
    assert!(s.contains("F32"));
    assert!(s.contains("48000Hz"));
    assert!(s.contains("Stereo"));
}

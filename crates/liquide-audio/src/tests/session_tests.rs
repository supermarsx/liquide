use crate::codec::PcmCodec;
use crate::format::*;
use crate::session::*;

fn default_format() -> AudioFormat {
    AudioFormat::new(
        SampleFormat::F32,
        SampleRate::Hz48000,
        ChannelLayout::Stereo,
    )
}

#[test]
fn session_create() {
    let session = AudioSession::new(default_format(), Box::new(PcmCodec::new()), 4096);
    assert!(!session.is_active());
    assert_eq!(*session.format(), default_format());
}

#[test]
fn session_start_stop() {
    let mut session = AudioSession::new(default_format(), Box::new(PcmCodec::new()), 4096);
    session.start();
    assert!(session.is_active());
    session.stop();
    assert!(!session.is_active());
}

#[test]
fn session_capture_when_inactive() {
    let mut session = AudioSession::new(default_format(), Box::new(PcmCodec::new()), 4096);
    let result = session.capture(&[1, 2, 3, 4]);
    assert!(result.is_err());
}

#[test]
fn session_capture_playback_flow() {
    let fmt = default_format();
    let mut session = AudioSession::new(fmt, Box::new(PcmCodec::new()), 4096);
    session.start();

    // Capture some data
    let data = vec![42u8; 64];
    session.capture(&data).unwrap();

    // Encode captured data (PCM passthrough)
    let encoded = session.encode_capture().unwrap();
    assert_eq!(encoded.len(), 64);

    // Decode into playback buffer
    session.decode_playback(&encoded).unwrap();

    // Read from playback
    let mut out = vec![0u8; 64];
    let n = session.playback(&mut out).unwrap();
    assert_eq!(n, 64);
    assert_eq!(out, data);
}

#[test]
fn session_encode_decode() {
    let fmt = default_format();
    let mut session = AudioSession::new(fmt, Box::new(PcmCodec::new()), 4096);
    session.start();

    let data = vec![99u8; 128];
    session.capture(&data).unwrap();
    let encoded = session.encode_capture().unwrap();
    assert_eq!(encoded, data);

    session.decode_playback(&encoded).unwrap();
    let mut out = vec![0u8; 128];
    let n = session.playback(&mut out).unwrap();
    assert_eq!(n, 128);
    assert_eq!(out, data);
}

#[test]
fn session_stats_increment() {
    let fmt = default_format();
    let mut session = AudioSession::new(fmt, Box::new(PcmCodec::new()), 4096);
    session.start();

    // Capture 8 bytes = 1 frame (frame_size = 8 for stereo f32)
    session.capture(&[0u8; 8]).unwrap();
    assert_eq!(session.stats().frames_captured, 1);

    // Encode
    let encoded = session.encode_capture().unwrap();
    assert_eq!(session.stats().bytes_encoded, 8);

    // Decode into playback
    session.decode_playback(&encoded).unwrap();
    assert_eq!(session.stats().bytes_decoded, 8);

    // Playback 8 bytes = 1 frame
    let mut out = vec![0u8; 8];
    session.playback(&mut out).unwrap();
    assert_eq!(session.stats().frames_played, 1);
}

#[test]
fn session_stats_overrun_counted() {
    let fmt = default_format();
    // Very small buffer to trigger overflow
    let mut session = AudioSession::new(fmt, Box::new(PcmCodec::new()), 8);
    session.start();

    // First write fills the buffer
    session.capture(&[0u8; 8]).unwrap();
    // Second write overflows
    let result = session.capture(&[0u8; 8]);
    assert!(result.is_err());
    assert_eq!(session.stats().buffer_overruns, 1);
}

#[test]
fn session_format() {
    let fmt = AudioFormat::new(SampleFormat::I16, SampleRate::Hz44100, ChannelLayout::Mono);
    let session = AudioSession::new(fmt, Box::new(PcmCodec::new()), 1024);
    assert_eq!(*session.format(), fmt);
}

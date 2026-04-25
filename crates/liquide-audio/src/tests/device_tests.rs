use crate::device::*;
use crate::format::*;
use crate::stream::*;

#[test]
fn null_device_manager_enumerate() {
    let mgr = NullDeviceManager::new();
    let devices = mgr.enumerate();
    assert_eq!(devices.len(), 2);

    let capture = devices
        .iter()
        .find(|d| d.direction == StreamDirection::Capture);
    assert!(capture.is_some());
    assert!(capture.unwrap().is_default);

    let playback = devices
        .iter()
        .find(|d| d.direction == StreamDirection::Playback);
    assert!(playback.is_some());
    assert!(playback.unwrap().is_default);
}

#[test]
fn null_device_manager_default_capture() {
    let mgr = NullDeviceManager::new();
    let info = mgr.default_capture().unwrap();
    assert_eq!(info.direction, StreamDirection::Capture);
    assert!(info.is_default);
    assert!(!info.supported_formats.is_empty());
}

#[test]
fn null_device_manager_default_playback() {
    let mgr = NullDeviceManager::new();
    let info = mgr.default_playback().unwrap();
    assert_eq!(info.direction, StreamDirection::Playback);
    assert!(info.is_default);
}

#[test]
fn null_device_manager_open_capture_stream() {
    let mut mgr = NullDeviceManager::new();
    let config = StreamConfig {
        format: AudioFormat::new(
            SampleFormat::F32,
            SampleRate::Hz48000,
            ChannelLayout::Stereo,
        ),
        direction: StreamDirection::Capture,
        buffer_size_frames: 512,
    };
    let mut stream = mgr.open_stream("Null Capture", config).unwrap();
    stream.start().unwrap();
    assert_eq!(stream.state(), StreamState::Running);
}

#[test]
fn null_device_manager_open_playback_stream() {
    let mut mgr = NullDeviceManager::new();
    let config = StreamConfig {
        format: AudioFormat::new(
            SampleFormat::F32,
            SampleRate::Hz48000,
            ChannelLayout::Stereo,
        ),
        direction: StreamDirection::Playback,
        buffer_size_frames: 512,
    };
    let mut stream = mgr.open_stream("Null Playback", config).unwrap();
    stream.start().unwrap();
    let written = stream.write(&[0u8; 64]).unwrap();
    assert_eq!(written, 64);
}

#[test]
fn null_device_manager_open_unknown_device() {
    let mut mgr = NullDeviceManager::new();
    let config = StreamConfig {
        format: AudioFormat::new(
            SampleFormat::F32,
            SampleRate::Hz48000,
            ChannelLayout::Stereo,
        ),
        direction: StreamDirection::Playback,
        buffer_size_frames: 512,
    };
    let result = mgr.open_stream("Nonexistent", config);
    assert!(result.is_err());
}

#[test]
fn device_info_serde() {
    let info = DeviceInfo {
        name: "Test Device".to_string(),
        is_default: true,
        supported_formats: vec![AudioFormat::new(
            SampleFormat::I16,
            SampleRate::Hz44100,
            ChannelLayout::Stereo,
        )],
        direction: StreamDirection::Playback,
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "Test Device");
    assert!(back.is_default);
    assert_eq!(back.supported_formats.len(), 1);
    assert_eq!(back.direction, StreamDirection::Playback);
}

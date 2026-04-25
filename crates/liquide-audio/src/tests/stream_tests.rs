use crate::format::*;
use crate::stream::*;

fn make_config(direction: StreamDirection) -> StreamConfig {
    StreamConfig {
        format: AudioFormat::new(
            SampleFormat::F32,
            SampleRate::Hz48000,
            ChannelLayout::Stereo,
        ),
        direction,
        buffer_size_frames: 1024,
    }
}

#[test]
fn memory_stream_initial_state() {
    let stream = MemoryStream::new(make_config(StreamDirection::Playback));
    assert_eq!(stream.state(), StreamState::Stopped);
}

#[test]
fn memory_stream_start_stop() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Playback));
    stream.start().unwrap();
    assert_eq!(stream.state(), StreamState::Running);
    stream.stop().unwrap();
    assert_eq!(stream.state(), StreamState::Stopped);
}

#[test]
fn memory_stream_pause_resume() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Playback));
    stream.start().unwrap();
    stream.pause().unwrap();
    assert_eq!(stream.state(), StreamState::Paused);
    stream.resume().unwrap();
    assert_eq!(stream.state(), StreamState::Running);
}

#[test]
fn memory_stream_write_read() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Playback));
    stream.start().unwrap();

    let data = vec![10u8; 32];
    let written = stream.write(&data).unwrap();
    assert_eq!(written, 32);

    let mut buf = vec![0u8; 32];
    let read = stream.read(&mut buf).unwrap();
    assert_eq!(read, 32);
    assert_eq!(buf, data);
}

#[test]
fn memory_stream_write_when_stopped() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Playback));
    let result = stream.write(&[1, 2, 3]);
    assert!(result.is_err());
}

#[test]
fn memory_stream_read_when_stopped() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Capture));
    let mut buf = vec![0u8; 16];
    let result = stream.read(&mut buf);
    assert!(result.is_err());
}

#[test]
fn memory_stream_config_and_direction() {
    let config = make_config(StreamDirection::Capture);
    let stream = MemoryStream::new(config);
    assert_eq!(stream.config().direction, StreamDirection::Capture);
    assert_eq!(stream.config().buffer_size_frames, 1024);
}

#[test]
fn memory_stream_double_start() {
    let mut stream = MemoryStream::new(make_config(StreamDirection::Playback));
    stream.start().unwrap();
    // Starting again when already running should be fine
    stream.start().unwrap();
    assert_eq!(stream.state(), StreamState::Running);
}

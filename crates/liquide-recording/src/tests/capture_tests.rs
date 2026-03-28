use crate::capture::*;

#[test]
fn test_recording_config_defaults() {
    let cfg = RecordingConfig::new();
    assert_eq!(cfg.framerate, 30);
    assert_eq!(cfg.quality, RecordingQuality::Medium);
    assert!(!cfg.include_audio);
    assert!(cfg.include_cursor);
    assert!(cfg.max_duration_secs.is_none());
    assert_eq!(cfg.output_format, OutputFormat::Mp4);
    assert_eq!(cfg.region, CaptureRegion::FullScreen(0));
}

#[test]
fn test_recording_config_builder() {
    let cfg = RecordingConfig::new()
        .with_region(CaptureRegion::Window(42))
        .with_framerate(60)
        .with_quality(RecordingQuality::High)
        .with_audio(true)
        .with_cursor(false)
        .with_max_duration(120)
        .with_format(OutputFormat::Webm);

    assert_eq!(cfg.region, CaptureRegion::Window(42));
    assert_eq!(cfg.framerate, 60);
    assert_eq!(cfg.quality, RecordingQuality::High);
    assert!(cfg.include_audio);
    assert!(!cfg.include_cursor);
    assert_eq!(cfg.max_duration_secs, Some(120));
    assert_eq!(cfg.output_format, OutputFormat::Webm);
}

#[test]
fn test_capture_region_display() {
    assert!(format!("{}", CaptureRegion::FullScreen(0)).contains("FullScreen"));
    assert!(format!("{}", CaptureRegion::Window(99)).contains("99"));
    let rect = CaptureRegion::Rectangle {
        x: 10,
        y: 20,
        width: 800,
        height: 600,
    };
    assert!(format!("{}", rect).contains("800x600"));
    assert!(format!("{}", CaptureRegion::AllScreens).contains("AllScreens"));
}

#[test]
fn test_output_format_display() {
    assert_eq!(format!("{}", OutputFormat::Mp4), "MP4");
    assert_eq!(format!("{}", OutputFormat::Webm), "WebM");
    assert_eq!(format!("{}", OutputFormat::Gif), "GIF");
    assert_eq!(format!("{}", OutputFormat::RawFrames), "RawFrames");
}

#[test]
fn test_recording_quality_bitrate() {
    let low = RecordingQuality::Low.suggested_bitrate_kbps(1920, 1080);
    let med = RecordingQuality::Medium.suggested_bitrate_kbps(1920, 1080);
    let high = RecordingQuality::High.suggested_bitrate_kbps(1920, 1080);
    let lossless = RecordingQuality::Lossless.suggested_bitrate_kbps(1920, 1080);
    assert!(low < med);
    assert!(med < high);
    assert!(high < lossless);
}

#[test]
fn test_recording_quality_compression() {
    assert!(RecordingQuality::Low.compression_level() > RecordingQuality::High.compression_level());
    assert_eq!(RecordingQuality::Lossless.compression_level(), 0);
}

#[test]
fn test_frame_interval() {
    let cfg = RecordingConfig::new().with_framerate(60);
    assert_eq!(cfg.frame_interval_us(), 16666);
    let cfg_zero = RecordingConfig::new().with_framerate(0);
    assert_eq!(cfg_zero.frame_interval_us(), 0);
}

#[test]
fn test_recording_result() {
    let r = RecordingResult::new(300, 10000, 1_000_000, 5);
    assert_eq!(r.frame_count, 300);
    assert_eq!(r.duration_ms, 10000);
    assert_eq!(r.output_size_bytes, 1_000_000);
    assert_eq!(r.dropped_frames, 5);
    assert!((r.average_fps - 30.0).abs() < 0.01);
}

#[test]
fn test_recording_result_zero_duration() {
    let r = RecordingResult::new(0, 0, 0, 0);
    assert_eq!(r.average_fps, 0.0);
}

#[test]
fn test_recording_config_serde() {
    let cfg = RecordingConfig::new()
        .with_region(CaptureRegion::Rectangle {
            x: -10,
            y: 20,
            width: 640,
            height: 480,
        })
        .with_quality(RecordingQuality::Lossless);
    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: RecordingConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.quality, RecordingQuality::Lossless);
    assert_eq!(deserialized.framerate, 30);
    if let CaptureRegion::Rectangle { x, width, .. } = deserialized.region {
        assert_eq!(x, -10);
        assert_eq!(width, 640);
    } else {
        panic!("expected Rectangle");
    }
}

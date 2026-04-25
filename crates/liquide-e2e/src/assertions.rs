use liquide_app_harness::{AppRunReport, FrameCapture};

pub fn assert_basic_launch_report(report: &AppRunReport) -> &FrameCapture {
    assert!(report.stats.frames >= 1, "expected at least one harness frame");
    assert!(
        report.present_count >= 1,
        "expected at least one presented frame"
    );
    assert_eq!(
        report.window_handles.len(),
        1,
        "expected exactly one top-level window"
    );

    let capture = report
        .last_present
        .as_ref()
        .expect("expected the app run to retain a last-present capture");

    assert!(capture.width > 0, "expected non-zero capture width");
    assert!(capture.height > 0, "expected non-zero capture height");
    assert!(
        !capture.pixels.is_empty(),
        "expected a non-empty presented frame buffer"
    );
    assert_eq!(
        capture.pixels.len(),
        (capture.stride * capture.height) as usize,
        "capture buffer length should match stride * height"
    );

    capture
}

pub fn assert_capture_size(report: &AppRunReport, width: u32, height: u32) -> &FrameCapture {
    let capture = assert_basic_launch_report(report);
    assert_eq!(capture.width, width, "unexpected capture width");
    assert_eq!(capture.height, height, "unexpected capture height");
    capture
}
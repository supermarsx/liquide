use crate::blur::*;

#[test]
fn blur_params_from_radius() {
    let params = BlurParams::from_radius(12.0, BlurQuality::Balanced);
    assert!((params.radius - 12.0).abs() < f32::EPSILON);
    assert!((params.sigma - 4.0).abs() < f32::EPSILON);
    assert_eq!(params.downsample_factor, 2);
}

#[test]
fn blur_params_from_radius_full_quality() {
    let params = BlurParams::from_radius(8.0, BlurQuality::Full);
    assert_eq!(params.downsample_factor, 1);
}

#[test]
fn blur_params_from_radius_performance() {
    let params = BlurParams::from_radius(16.0, BlurQuality::Performance);
    assert_eq!(params.downsample_factor, 4);
}

#[test]
fn blur_disabled_returns_zero_passes() {
    let blur = GpuBlur::new(BlurParams {
        radius: 12.0,
        sigma: 4.0,
        quality: BlurQuality::Disabled,
        downsample_factor: 1,
    });

    let result = blur.compute_blur(1920, 1080);
    assert_eq!(result.passes, 0);
    assert_eq!(result.output_width, 1920);
    assert_eq!(result.output_height, 1080);
}

#[test]
fn blur_full_uses_two_passes() {
    let blur = GpuBlur::new(BlurParams::from_radius(12.0, BlurQuality::Full));
    let result = blur.compute_blur(1920, 1080);
    assert_eq!(result.passes, 2);
}

#[test]
fn blur_balanced_uses_four_passes() {
    // Balanced uses downsample (2 extra passes) + 2 blur passes = 4
    let blur = GpuBlur::new(BlurParams::from_radius(12.0, BlurQuality::Balanced));
    let result = blur.compute_blur(1920, 1080);
    assert_eq!(result.passes, 4);
}

#[test]
fn blur_preserves_output_dimensions() {
    let blur = GpuBlur::new(BlurParams::from_radius(20.0, BlurQuality::Performance));
    let result = blur.compute_blur(2560, 1440);

    assert_eq!(result.output_width, 2560);
    assert_eq!(result.output_height, 1440);
}

#[test]
fn blur_default_params() {
    let blur = GpuBlur::default();
    let params = blur.params();
    assert!((params.radius - 12.0).abs() < f32::EPSILON);
    assert_eq!(params.quality, BlurQuality::Balanced);
}

#[test]
fn blur_set_params() {
    let mut blur = GpuBlur::default();
    let new_params = BlurParams::from_radius(24.0, BlurQuality::Full);
    blur.set_params(new_params);

    assert!((blur.params().radius - 24.0).abs() < f32::EPSILON);
    assert_eq!(blur.params().quality, BlurQuality::Full);
}

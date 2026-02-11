use crate::pipeline::*;

#[test]
fn default_pipeline_has_all_stages() {
    let pipeline = ComputePipeline::default();
    // Default config enables blur, shadows, and cursor.
    assert!(pipeline.stage_count() >= 6);
    assert_eq!(pipeline.frame_count(), 0);
}

#[test]
fn pipeline_with_disabled_blur() {
    let config = PipelineConfig {
        blur_quality: BlurQuality::Disabled,
        shadow_enabled: true,
        max_blur_radius: 0,
        enable_cursor_hw: true,
    };
    let pipeline = ComputePipeline::new(config);
    let stages = pipeline.stages();

    assert!(
        !stages.contains(&PipelineStage::Blur),
        "blur should be excluded when disabled"
    );
}

#[test]
fn pipeline_with_disabled_shadows() {
    let config = PipelineConfig {
        blur_quality: BlurQuality::Balanced,
        shadow_enabled: false,
        max_blur_radius: 64,
        enable_cursor_hw: true,
    };
    let pipeline = ComputePipeline::new(config);
    let stages = pipeline.stages();

    assert!(
        !stages.contains(&PipelineStage::Shadows),
        "shadows should be excluded when disabled"
    );
}

#[test]
fn pipeline_with_disabled_cursor() {
    let config = PipelineConfig {
        blur_quality: BlurQuality::Balanced,
        shadow_enabled: true,
        max_blur_radius: 64,
        enable_cursor_hw: false,
    };
    let pipeline = ComputePipeline::new(config);
    let stages = pipeline.stages();

    assert!(
        !stages.contains(&PipelineStage::Cursor),
        "cursor should be excluded when disabled"
    );
}

#[test]
fn pipeline_always_has_core_stages() {
    let config = PipelineConfig {
        blur_quality: BlurQuality::Disabled,
        shadow_enabled: false,
        max_blur_radius: 0,
        enable_cursor_hw: false,
    };
    let pipeline = ComputePipeline::new(config);
    let stages = pipeline.stages();

    assert!(stages.contains(&PipelineStage::SceneTraversal));
    assert!(stages.contains(&PipelineStage::RoundedRects));
    assert!(stages.contains(&PipelineStage::AlphaComposite));
    assert!(stages.contains(&PipelineStage::Finalize));
}

#[test]
fn execute_frame_increments_count() {
    let mut pipeline = ComputePipeline::default();
    assert_eq!(pipeline.frame_count(), 0);

    let result = pipeline.execute_frame(&[], 1920, 1080);
    assert!(result.is_ok());
    assert_eq!(pipeline.frame_count(), 1);

    let result = pipeline.execute_frame(&[], 1920, 1080);
    assert!(result.is_ok());
    assert_eq!(pipeline.frame_count(), 2);
}

#[test]
fn execute_frame_zero_dimensions_fails() {
    let mut pipeline = ComputePipeline::default();

    let result = pipeline.execute_frame(&[], 0, 1080);
    assert!(result.is_err());

    let result = pipeline.execute_frame(&[], 1920, 0);
    assert!(result.is_err());
}

#[test]
fn frame_result_has_correct_dimensions() {
    let mut pipeline = ComputePipeline::default();
    let result = pipeline.execute_frame(&[], 3840, 2160).unwrap();

    assert_eq!(result.width, 3840);
    assert_eq!(result.height, 2160);
    assert_eq!(result.frame_id, 1);
}

#[test]
fn stage_order_is_deterministic() {
    let p1 = ComputePipeline::default();
    let p2 = ComputePipeline::default();

    assert_eq!(p1.stages(), p2.stages());
}

#[test]
fn pipeline_stage_display() {
    assert_eq!(PipelineStage::SceneTraversal.to_string(), "scene-traversal");
    assert_eq!(PipelineStage::AlphaComposite.to_string(), "alpha-composite");
    assert_eq!(PipelineStage::Finalize.to_string(), "finalize");
}

#[test]
fn pipeline_config_default() {
    let config = PipelineConfig::default();
    assert_eq!(config.blur_quality, BlurQuality::Balanced);
    assert!(config.shadow_enabled);
    assert_eq!(config.max_blur_radius, 64);
    assert!(config.enable_cursor_hw);
}

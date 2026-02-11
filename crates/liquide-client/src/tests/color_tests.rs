use crate::color::{ColorMode, ColorNegotiation, ColorPipeline, DisplayGamut};

#[test]
fn test_default_negotiation_is_sdr() {
    let neg = ColorNegotiation::new();
    assert_eq!(neg.best_mode(), ColorMode::SdrSrgb);
    assert!(!neg.supports_hdr());
}

#[test]
fn test_hdr_negotiation() {
    let neg = ColorNegotiation {
        supported_modes: vec![ColorMode::SdrSrgb, ColorMode::WcgSdr, ColorMode::Hdr],
        display_gamut: DisplayGamut::Bt2020,
        hdr_support: true,
        max_luminance_nits: 1000,
        preferred_bit_depth: 10,
    };
    assert!(neg.supports_hdr());
    assert_eq!(neg.best_mode(), ColorMode::Hdr);
}

#[test]
fn test_wcg_fallback() {
    let neg = ColorNegotiation {
        supported_modes: vec![ColorMode::SdrSrgb, ColorMode::WcgSdr],
        display_gamut: DisplayGamut::P3,
        hdr_support: false,
        max_luminance_nits: 600,
        preferred_bit_depth: 10,
    };
    assert!(!neg.supports_hdr());
    assert_eq!(neg.best_mode(), ColorMode::WcgSdr);
}

#[test]
fn test_pipeline_defaults() {
    let pipeline = ColorPipeline::new();
    assert_eq!(pipeline.active_mode(), ColorMode::SdrSrgb);
    assert!(!pipeline.needs_tone_mapping());
    assert!(!pipeline.needs_gamut_mapping());
}

#[test]
fn test_pipeline_hdr_needs_tone_mapping() {
    let mut pipeline = ColorPipeline::new();
    pipeline.set_mode(ColorMode::Hdr);
    assert!(pipeline.needs_tone_mapping());
}

#[test]
fn test_pipeline_wcg_needs_gamut_mapping() {
    let mut pipeline = ColorPipeline::new();
    pipeline.set_mode(ColorMode::WcgSdr);
    assert!(pipeline.needs_gamut_mapping());
}

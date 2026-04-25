//! Tests for the audio effects processing module.

use crate::effects::*;

// ── db_to_linear / linear_to_db ───────────────────────────────────────

#[test]
fn db_to_linear_zero() {
    let gain = db_to_linear(0.0);
    assert!((gain - 1.0).abs() < 0.001);
}

#[test]
fn db_to_linear_minus_6() {
    let gain = db_to_linear(-6.0);
    assert!((gain - 0.5012).abs() < 0.01);
}

#[test]
fn db_to_linear_plus_6() {
    let gain = db_to_linear(6.0);
    assert!((gain - 1.995).abs() < 0.01);
}

#[test]
fn db_to_linear_very_low() {
    let gain = db_to_linear(-100.0);
    assert!((gain - 0.0).abs() < f32::EPSILON);
}

#[test]
fn db_to_linear_minus_120() {
    let gain = db_to_linear(-120.0);
    assert!((gain - 0.0).abs() < f32::EPSILON);
}

#[test]
fn linear_to_db_unity() {
    let db = linear_to_db(1.0);
    assert!((db - 0.0).abs() < 0.001);
}

#[test]
fn linear_to_db_half() {
    let db = linear_to_db(0.5);
    assert!((db - (-6.02)).abs() < 0.1);
}

#[test]
fn linear_to_db_zero() {
    let db = linear_to_db(0.0);
    assert!(db.is_infinite() && db.is_sign_negative());
}

#[test]
fn db_linear_roundtrip() {
    for &db in &[-20.0, -6.0, 0.0, 3.0, 12.0] {
        let linear = db_to_linear(db);
        let back = linear_to_db(linear);
        assert!((back - db).abs() < 0.01, "roundtrip failed for {db}dB");
    }
}

// ── VolumeEffect ──────────────────────────────────────────────────────

#[test]
fn volume_effect_unity() {
    let mut vol = VolumeEffect::new(0.0, 0.0); // 0dB, instant smoothing
    let input = [0.5f32, -0.3, 0.8];
    let mut output = [0.0f32; 3];
    vol.process(&input, &mut output);

    assert!((output[0] - 0.5).abs() < 0.01);
    assert!((output[1] - (-0.3)).abs() < 0.01);
    assert!((output[2] - 0.8).abs() < 0.01);
}

#[test]
fn volume_effect_minus_6db() {
    let mut vol = VolumeEffect::new(-6.0, 0.0);
    let input = [1.0f32; 4];
    let mut output = [0.0f32; 4];
    vol.process(&input, &mut output);

    // ~0.5 at -6dB
    for &s in &output {
        assert!((s - 0.5).abs() < 0.05);
    }
}

#[test]
fn volume_effect_set_gain() {
    let mut vol = VolumeEffect::new(0.0, 0.0);
    vol.set_gain_db(-12.0);
    assert!((vol.gain_db() - (-12.0)).abs() < f32::EPSILON);
}

#[test]
fn volume_effect_reset() {
    let mut vol = VolumeEffect::new(-6.0, 0.99);
    // Process some samples to move current_gain away from target.
    let input = [1.0f32; 10];
    let mut output = [0.0f32; 10];
    vol.set_gain_db(0.0);
    vol.process(&input, &mut output);

    vol.reset();
    assert!((vol.current_linear_gain() - 1.0).abs() < 0.01);
}

#[test]
fn volume_effect_display() {
    let vol = VolumeEffect::new(-6.0, 0.99);
    let s = format!("{vol}");
    assert!(s.contains("VolumeEffect"));
    assert!(s.contains("-6.0dB"));
}

// ── EqualizerEffect ───────────────────────────────────────────────────

#[test]
fn equalizer_flat() {
    let mut eq = EqualizerEffect::new();
    let input = [0.5f32; 4];
    let mut output = [0.0f32; 4];
    eq.process(&input, &mut output);

    // Flat EQ: average of 5 bands at 0dB (linear 1.0) = 1.0 -> passthrough
    for &s in &output {
        assert!((s - 0.5).abs() < 0.01);
    }
}

#[test]
fn equalizer_set_band() {
    let mut eq = EqualizerEffect::new();
    eq.set_band(0, 6.0);
    assert!((eq.get_band(0) - 6.0).abs() < f32::EPSILON);
}

#[test]
fn equalizer_set_band_out_of_range() {
    let mut eq = EqualizerEffect::new();
    eq.set_band(10, 6.0); // Should be a no-op
    assert!((eq.get_band(10) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn equalizer_reset_bands() {
    let mut eq = EqualizerEffect::new();
    eq.set_band(0, 6.0);
    eq.set_band(4, -3.0);
    eq.reset_bands();
    for i in 0..5 {
        assert!((eq.get_band(i) - 0.0).abs() < f32::EPSILON);
    }
}

#[test]
fn equalizer_frequencies() {
    assert_eq!(EQ_FREQUENCIES.len(), 5);
    assert!((EQ_FREQUENCIES[0] - 60.0).abs() < f32::EPSILON);
    assert!((EQ_FREQUENCIES[4] - 14000.0).abs() < f32::EPSILON);
}

#[test]
fn equalizer_display() {
    let eq = EqualizerEffect::new();
    let s = format!("{eq}");
    assert!(s.contains("EqualizerEffect"));
}

#[test]
fn equalizer_name() {
    let eq = EqualizerEffect::new();
    assert_eq!(eq.name(), "Equalizer");
}

// ── CompressorEffect ─────────────────────────────────────────────────

#[test]
fn compressor_below_threshold() {
    let mut comp = CompressorEffect::new(-20.0, 4.0, 1.0, 10.0, 48000.0);
    // Signal at -40 dBFS (very quiet, below threshold)
    let level = db_to_linear(-40.0);
    let input = vec![level; 1000];
    let mut output = vec![0.0f32; 1000];
    comp.process(&input, &mut output);

    // Below threshold: gain reduction should be negligible, output ~ input
    // (after envelope settles)
    let tail = &output[500..];
    for &s in tail {
        assert!((s - level).abs() < 0.01);
    }
}

#[test]
fn compressor_above_threshold() {
    let mut comp = CompressorEffect::new(-6.0, 4.0, 0.1, 10.0, 48000.0);
    // Signal at 0 dBFS (loud, above -6dB threshold)
    let input = vec![1.0f32; 2000];
    let mut output = vec![0.0f32; 2000];
    comp.process(&input, &mut output);

    // After the compressor settles, output should be less than input
    let tail_peak: f32 = output[1500..].iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!(
        tail_peak < 0.95,
        "compressor should reduce level above threshold, got {tail_peak}"
    );
}

#[test]
fn compressor_reset() {
    let mut comp = CompressorEffect::new(-20.0, 4.0, 1.0, 10.0, 48000.0);
    let input = [1.0f32; 100];
    let mut output = [0.0f32; 100];
    comp.process(&input, &mut output);
    comp.reset();
    // After reset, envelope should be at 0
    let s = format!("{comp}");
    assert!(s.contains("Compressor"));
}

#[test]
fn compressor_display() {
    let comp = CompressorEffect::new(-20.0, 4.0, 5.0, 50.0, 48000.0);
    let s = format!("{comp}");
    assert!(s.contains("threshold=-20.0dB"));
    assert!(s.contains("ratio=4.0:1"));
}

#[test]
fn compressor_name() {
    let comp = CompressorEffect::new(-20.0, 4.0, 5.0, 50.0, 48000.0);
    assert_eq!(comp.name(), "Compressor");
}

// ── EffectChain ───────────────────────────────────────────────────────

#[test]
fn effect_chain_empty_passthrough() {
    let mut chain = EffectChain::new();
    let input = [0.5f32, -0.3, 0.8];
    let mut output = [0.0f32; 3];
    chain.process(&input, &mut output);

    assert!((output[0] - 0.5).abs() < f32::EPSILON);
    assert!((output[1] - (-0.3)).abs() < f32::EPSILON);
    assert!((output[2] - 0.8).abs() < f32::EPSILON);
}

#[test]
fn effect_chain_single_effect() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0)));

    let input = [1.0f32; 4];
    let mut output = [0.0f32; 4];
    chain.process(&input, &mut output);

    for &s in &output {
        assert!((s - 0.5).abs() < 0.05);
    }
}

#[test]
fn effect_chain_two_effects() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0))); // ~0.5x
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0))); // ~0.5x again

    let input = [1.0f32; 8];
    let mut output = [0.0f32; 8];
    chain.process(&input, &mut output);

    // ~0.25 after two -6dB stages
    for &s in &output {
        assert!((s - 0.25).abs() < 0.05);
    }
}

#[test]
fn effect_chain_three_effects() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0)));
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0)));
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.0)));

    let input = [1.0f32; 8];
    let mut output = [0.0f32; 8];
    chain.process(&input, &mut output);

    // ~0.125 after three -6dB stages
    for &s in &output {
        assert!((s - 0.125).abs() < 0.05);
    }
}

#[test]
fn effect_chain_len_is_empty() {
    let mut chain = EffectChain::new();
    assert_eq!(chain.len(), 0);
    assert!(chain.is_empty());

    chain.push(Box::new(VolumeEffect::new(0.0, 0.0)));
    assert_eq!(chain.len(), 1);
    assert!(!chain.is_empty());
}

#[test]
fn effect_chain_pop() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(0.0, 0.0)));
    let popped = chain.pop();
    assert!(popped.is_some());
    assert!(chain.is_empty());
}

#[test]
fn effect_chain_effect_names() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(0.0, 0.0)));
    chain.push(Box::new(EqualizerEffect::new()));

    let names = chain.effect_names();
    assert_eq!(names, &["Volume", "Equalizer"]);
}

#[test]
fn effect_chain_reset() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(-6.0, 0.99)));
    chain.reset(); // Should not panic.
}

#[test]
fn effect_chain_display() {
    let mut chain = EffectChain::new();
    chain.push(Box::new(VolumeEffect::new(0.0, 0.0)));
    let s = format!("{chain}");
    assert!(s.contains("EffectChain"));
    assert!(s.contains("Volume"));
}

#[test]
fn volume_effect_name() {
    let vol = VolumeEffect::new(0.0, 0.0);
    assert_eq!(vol.name(), "Volume");
}

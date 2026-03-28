//! Tests for the audio mixer module.

use crate::mixer::*;

// ── SourceId ──────────────────────────────────────────────────────────

#[test]
fn source_id_equality() {
    assert_eq!(SourceId(1), SourceId(1));
    assert_ne!(SourceId(1), SourceId(2));
}

#[test]
fn source_id_display() {
    assert!(format!("{}", SourceId(42)).contains("42"));
}

// ── MixerInput ────────────────────────────────────────────────────────

#[test]
fn mixer_input_defaults() {
    let input = MixerInput::new(SourceId(1));
    assert!((input.volume - 1.0).abs() < f32::EPSILON);
    assert!((input.pan - 0.0).abs() < f32::EPSILON);
    assert!(!input.muted);
}

#[test]
fn mixer_input_set_volume_clamps() {
    let mut input = MixerInput::new(SourceId(1));
    input.set_volume(3.0);
    assert!((input.volume - 2.0).abs() < f32::EPSILON);
    input.set_volume(-1.0);
    assert!((input.volume - 0.0).abs() < f32::EPSILON);
}

#[test]
fn mixer_input_set_pan_clamps() {
    let mut input = MixerInput::new(SourceId(1));
    input.set_pan(2.0);
    assert!((input.pan - 1.0).abs() < f32::EPSILON);
    input.set_pan(-2.0);
    assert!((input.pan - (-1.0)).abs() < f32::EPSILON);
}

// ── AudioMixer ────────────────────────────────────────────────────────

#[test]
fn mixer_new_empty() {
    let mixer = AudioMixer::new();
    assert_eq!(mixer.input_count(), 0);
    assert!((mixer.master_volume() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn mixer_add_remove_input() {
    let mut mixer = AudioMixer::new();
    let id = mixer.add_input(SourceId(1));
    assert_eq!(mixer.input_count(), 1);
    assert!(mixer.get_input(id).is_some());

    mixer.remove_input(id);
    assert_eq!(mixer.input_count(), 0);
    assert!(mixer.get_input(id).is_none());
}

#[test]
fn mixer_add_input_auto() {
    let mut mixer = AudioMixer::new();
    let id1 = mixer.add_input_auto();
    let id2 = mixer.add_input_auto();
    assert_ne!(id1, id2);
    assert_eq!(mixer.input_count(), 2);
}

#[test]
fn mixer_mix_frame_single_source() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);

    let source = [0.5f32, -0.3, 0.8];
    let mut output = [0.0f32; 3];
    mixer.mix_frame(&[(id, &source[..])], &mut output);

    assert!((output[0] - 0.5).abs() < 0.001);
    assert!((output[1] - (-0.3)).abs() < 0.001);
    assert!((output[2] - 0.8).abs() < 0.001);
}

#[test]
fn mixer_mix_frame_two_sources() {
    let mut mixer = AudioMixer::new();
    let id1 = SourceId(1);
    let id2 = SourceId(2);
    mixer.add_input(id1);
    mixer.add_input(id2);

    let src1 = [0.5f32, 0.3];
    let src2 = [0.2f32, 0.4];
    let mut output = [0.0f32; 2];
    mixer.mix_frame(&[(id1, &src1[..]), (id2, &src2[..])], &mut output);

    assert!((output[0] - 0.7).abs() < 0.001);
    assert!((output[1] - 0.7).abs() < 0.001);
}

#[test]
fn mixer_mix_frame_clamps() {
    let mut mixer = AudioMixer::new();
    let id1 = SourceId(1);
    let id2 = SourceId(2);
    mixer.add_input(id1);
    mixer.add_input(id2);

    let src1 = [0.8f32];
    let src2 = [0.5f32];
    let mut output = [0.0f32; 1];
    mixer.mix_frame(&[(id1, &src1[..]), (id2, &src2[..])], &mut output);

    assert!((output[0] - 1.0).abs() < f32::EPSILON); // Clamped to 1.0
}

#[test]
fn mixer_mix_frame_muted_input() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);
    mixer.get_input_mut(id).unwrap().muted = true;

    let source = [0.5f32; 4];
    let mut output = [0.0f32; 4];
    mixer.mix_frame(&[(id, &source[..])], &mut output);

    for &s in &output {
        assert!((s - 0.0).abs() < f32::EPSILON);
    }
}

#[test]
fn mixer_mix_frame_volume_scaling() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);
    mixer.get_input_mut(id).unwrap().set_volume(0.5);

    let source = [1.0f32; 2];
    let mut output = [0.0f32; 2];
    mixer.mix_frame(&[(id, &source[..])], &mut output);

    assert!((output[0] - 0.5).abs() < 0.001);
}

#[test]
fn mixer_master_volume_scaling() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);
    mixer.set_master_volume(0.5);

    let source = [1.0f32; 2];
    let mut output = [0.0f32; 2];
    mixer.mix_frame(&[(id, &source[..])], &mut output);

    assert!((output[0] - 0.5).abs() < 0.001);
}

#[test]
fn mixer_stereo_center_pan() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);

    let source = [1.0f32; 4];
    let mut output = [0.0f32; 8];
    mixer.mix_stereo(&[(id, &source[..])], &mut output, 4);

    // Center pan: both channels should be roughly equal (~0.707)
    let left = output[0];
    let right = output[1];
    assert!((left - right).abs() < 0.01);
    assert!(left > 0.5);
}

#[test]
fn mixer_stereo_full_left() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);
    mixer.get_input_mut(id).unwrap().set_pan(-1.0);

    let source = [1.0f32; 1];
    let mut output = [0.0f32; 2];
    mixer.mix_stereo(&[(id, &source[..])], &mut output, 1);

    // Full left: left ~= 1.0, right ~= 0.0
    assert!(output[0] > 0.9);
    assert!(output[1] < 0.1);
}

#[test]
fn mixer_stereo_full_right() {
    let mut mixer = AudioMixer::new();
    let id = SourceId(1);
    mixer.add_input(id);
    mixer.get_input_mut(id).unwrap().set_pan(1.0);

    let source = [1.0f32; 1];
    let mut output = [0.0f32; 2];
    mixer.mix_stereo(&[(id, &source[..])], &mut output, 1);

    // Full right: left ~= 0.0, right ~= 1.0
    assert!(output[0] < 0.1);
    assert!(output[1] > 0.9);
}

// ── Free functions ────────────────────────────────────────────────────

#[test]
fn apply_volume_scales() {
    let mut samples = [0.5f32, -0.3, 0.8, 0.0];
    apply_volume(&mut samples, 0.5);
    assert!((samples[0] - 0.25).abs() < 0.001);
    assert!((samples[1] - (-0.15)).abs() < 0.001);
    assert!((samples[2] - 0.4).abs() < 0.001);
    assert!((samples[3] - 0.0).abs() < f32::EPSILON);
}

#[test]
fn apply_volume_zero() {
    let mut samples = [0.5f32, -0.3];
    apply_volume(&mut samples, 0.0);
    for &s in &samples {
        assert!((s - 0.0).abs() < f32::EPSILON);
    }
}

#[test]
fn apply_pan_center() {
    let samples = [1.0f32; 4];
    let mut left = [0.0f32; 4];
    let mut right = [0.0f32; 4];
    apply_pan(&samples, 0.0, &mut left, &mut right);

    // Center pan: both channels roughly equal
    for i in 0..4 {
        assert!((left[i] - right[i]).abs() < 0.01);
    }
}

#[test]
fn compute_peak_normal() {
    let samples = [0.3f32, -0.7, 0.5, 0.1];
    let peak = compute_peak(&samples);
    assert!((peak - 0.7).abs() < f32::EPSILON);
}

#[test]
fn compute_peak_empty() {
    let peak = compute_peak(&[]);
    assert!((peak - 0.0).abs() < f32::EPSILON);
}

#[test]
fn compute_peak_silence() {
    let samples = [0.0f32; 8];
    let peak = compute_peak(&samples);
    assert!((peak - 0.0).abs() < f32::EPSILON);
}

#[test]
fn compute_rms_sine_like() {
    // For a signal of constant 0.5, RMS = 0.5
    let samples = [0.5f32; 100];
    let rms = compute_rms(&samples);
    assert!((rms - 0.5).abs() < 0.001);
}

#[test]
fn compute_rms_empty() {
    let rms = compute_rms(&[]);
    assert!((rms - 0.0).abs() < f32::EPSILON);
}

#[test]
fn mixer_display() {
    let mixer = AudioMixer::new();
    let s = format!("{mixer}");
    assert!(s.contains("AudioMixer"));
}

#[test]
fn mixer_input_display() {
    let input = MixerInput::new(SourceId(1));
    let s = format!("{input}");
    assert!(s.contains("MixerInput"));
}

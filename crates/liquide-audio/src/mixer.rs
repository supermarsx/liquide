//! Audio mixer and routing — mix multiple audio streams into a single output.
//!
//! Provides a software mixer that sums, pans, and volume-scales multiple
//! input sources into a combined stereo (or mono) output buffer.

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for a mixer input source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(pub u64);

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SourceId({})", self.0)
    }
}

/// Configuration for a single mixer input channel.
#[derive(Debug, Clone)]
pub struct MixerInput {
    /// The source identifier.
    pub source_id: SourceId,
    /// Volume scaling factor (0.0 = silence, 1.0 = unity gain).
    pub volume: f32,
    /// Stereo pan position (-1.0 = full left, 0.0 = center, 1.0 = full right).
    pub pan: f32,
    /// Whether this input is muted.
    pub muted: bool,
}

impl MixerInput {
    /// Create a new mixer input at unity gain, center pan, unmuted.
    #[must_use]
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            volume: 1.0,
            pan: 0.0,
            muted: false,
        }
    }

    /// Set the volume, clamping to 0.0..=2.0 (allows boost up to +6 dB).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 2.0);
    }

    /// Set the pan position, clamping to -1.0..=1.0.
    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
    }
}

impl fmt::Display for MixerInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MixerInput({}, vol={:.2}, pan={:.2}{})",
            self.source_id,
            self.volume,
            self.pan,
            if self.muted { ", MUTED" } else { "" },
        )
    }
}

/// Result of a mix operation.
#[derive(Debug, Clone)]
pub struct MixerOutput {
    /// The mixed audio samples (interleaved stereo or mono).
    pub samples: Vec<f32>,
    /// Number of channels in the output (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Peak level of the mixed output (0.0 to 1.0+).
    pub peak: f32,
    /// Whether any clipping occurred (samples exceeded +/- 1.0).
    pub clipped: bool,
}

impl fmt::Display for MixerOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MixerOutput({} samples, {}ch, peak={:.4}{})",
            self.samples.len(),
            self.channels,
            self.peak,
            if self.clipped { ", CLIPPED" } else { "" },
        )
    }
}

/// Audio mixer that combines multiple input sources into a single output.
///
/// Each input can be independently volume-controlled, panned, and muted.
/// The mixer sums all active inputs and clamps the output to prevent overflow.
pub struct AudioMixer {
    inputs: HashMap<SourceId, MixerInput>,
    next_id: u64,
    /// Master output volume.
    master_volume: f32,
}

impl AudioMixer {
    /// Create a new empty audio mixer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inputs: HashMap::new(),
            next_id: 1,
            master_volume: 1.0,
        }
    }

    /// Add a new input source to the mixer.
    ///
    /// Returns the assigned [`SourceId`].
    pub fn add_input(&mut self, source_id: SourceId) -> SourceId {
        let input = MixerInput::new(source_id);
        self.inputs.insert(source_id, input);
        source_id
    }

    /// Add a new input source with an auto-assigned id.
    pub fn add_input_auto(&mut self) -> SourceId {
        let id = SourceId(self.next_id);
        self.next_id += 1;
        self.add_input(id);
        id
    }

    /// Remove an input source from the mixer.
    ///
    /// Returns the removed input, or `None` if not found.
    pub fn remove_input(&mut self, source_id: SourceId) -> Option<MixerInput> {
        self.inputs.remove(&source_id)
    }

    /// Get a reference to an input by source id.
    #[must_use]
    pub fn get_input(&self, source_id: SourceId) -> Option<&MixerInput> {
        self.inputs.get(&source_id)
    }

    /// Get a mutable reference to an input by source id.
    pub fn get_input_mut(&mut self, source_id: SourceId) -> Option<&mut MixerInput> {
        self.inputs.get_mut(&source_id)
    }

    /// Number of active inputs.
    #[must_use]
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Set the master output volume, clamped to 0.0..=2.0.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 2.0);
    }

    /// Get the master output volume.
    #[must_use]
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Mix a single frame of audio from multiple mono input buffers into a mono output.
    ///
    /// All inputs are summed with their respective volume and the result is clamped
    /// to -1.0..=1.0. `inputs` is a slice of `(SourceId, &[f32])` pairs.
    pub fn mix_frame(&self, inputs: &[(SourceId, &[f32])], output: &mut [f32]) {
        // Zero the output buffer.
        for sample in output.iter_mut() {
            *sample = 0.0;
        }

        for (source_id, source_samples) in inputs {
            let config = match self.inputs.get(source_id) {
                Some(c) if !c.muted => c,
                _ => continue,
            };

            let vol = config.volume * self.master_volume;
            let len = source_samples.len().min(output.len());
            for i in 0..len {
                output[i] += source_samples[i] * vol;
            }
        }

        // Clamp output.
        for sample in output.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    /// Mix multiple mono input buffers into an interleaved stereo output,
    /// applying per-input pan.
    ///
    /// `output` must have length `frame_count * 2` (interleaved L/R).
    pub fn mix_stereo(
        &self,
        inputs: &[(SourceId, &[f32])],
        output: &mut [f32],
        frame_count: usize,
    ) {
        let out_len = frame_count * 2;
        for sample in output.iter_mut().take(out_len) {
            *sample = 0.0;
        }

        for (source_id, source_samples) in inputs {
            let config = match self.inputs.get(source_id) {
                Some(c) if !c.muted => c,
                _ => continue,
            };

            let vol = config.volume * self.master_volume;
            let (left_gain, right_gain) = pan_gains(config.pan);

            let len = source_samples.len().min(frame_count);
            for i in 0..len {
                let s = source_samples[i] * vol;
                let out_idx = i * 2;
                if out_idx + 1 < output.len() {
                    output[out_idx] += s * left_gain;
                    output[out_idx + 1] += s * right_gain;
                }
            }
        }

        // Clamp output.
        for sample in output.iter_mut().take(out_len) {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AudioMixer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioMixer({} inputs, master={:.2})",
            self.inputs.len(),
            self.master_volume,
        )
    }
}

// ── Free functions ────────────────────────────────────────────────────

/// Apply a volume scale to a slice of audio samples in place.
pub fn apply_volume(samples: &mut [f32], volume: f32) {
    for sample in samples.iter_mut() {
        *sample *= volume;
    }
}

/// Apply stereo panning to a mono source, writing to separate left and right buffers.
///
/// Uses constant-power panning (sine/cosine law) for perceptually even levels.
pub fn apply_pan(samples: &[f32], pan: f32, left: &mut [f32], right: &mut [f32]) {
    let (left_gain, right_gain) = pan_gains(pan);
    let len = samples.len().min(left.len()).min(right.len());
    for i in 0..len {
        left[i] = samples[i] * left_gain;
        right[i] = samples[i] * right_gain;
    }
}

/// Compute the peak absolute sample value in a buffer.
#[must_use]
pub fn compute_peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |max, &s| max.max(s.abs()))
}

/// Compute RMS (root mean square) level for a buffer.
#[must_use]
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

/// Compute constant-power pan gains for left and right channels.
///
/// `pan` is in -1.0 (full left) to 1.0 (full right).
/// Returns `(left_gain, right_gain)`.
#[must_use]
fn pan_gains(pan: f32) -> (f32, f32) {
    let pan = pan.clamp(-1.0, 1.0);
    // Map pan from [-1, 1] to [0, pi/2] for sine/cosine law.
    let angle = (pan + 1.0) * 0.25 * std::f32::consts::PI;
    let left_gain = angle.cos();
    let right_gain = angle.sin();
    (left_gain, right_gain)
}

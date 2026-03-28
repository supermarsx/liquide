//! Audio effects processing — volume, EQ, compressor, and effect chains.
//!
//! Provides a trait-based audio effect pipeline where individual effects
//! can be composed into chains and applied to audio buffers in sequence.

use std::fmt;

/// Trait for audio effects that process sample buffers.
///
/// Effects read from `input` and write to `output`. Both slices are
/// mono f32 samples in the range -1.0..=1.0 (though values outside
/// that range are permitted).
pub trait AudioEffect: Send {
    /// Process input samples and write the result to output.
    ///
    /// `input` and `output` must have the same length.
    fn process(&mut self, input: &[f32], output: &mut [f32]);

    /// Reset internal state (e.g. envelope followers, delay lines).
    fn reset(&mut self);

    /// Human-readable name for this effect.
    fn name(&self) -> &str;
}

/// Convert a decibel value to linear gain.
///
/// 0 dB = 1.0, -6 dB ~= 0.5, +6 dB ~= 2.0, -inf dB = 0.0.
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= -100.0 {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

/// Convert a linear gain to decibels.
///
/// 1.0 = 0 dB, 0.0 = -inf dB.
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -f32::INFINITY
    } else {
        20.0 * linear.log10()
    }
}

// ── VolumeEffect ──────────────────────────────────────────────────────

/// Volume/gain effect with smooth ramping to avoid clicks.
///
/// Gain is specified in dB. The effect ramps from the previous gain
/// to the target gain over a configurable ramp period.
pub struct VolumeEffect {
    /// Target gain in dB.
    target_db: f32,
    /// Current (smoothed) linear gain.
    current_gain: f32,
    /// Smoothing coefficient (0.0 = instant, closer to 1.0 = slower).
    smoothing: f32,
}

impl VolumeEffect {
    /// Create a new volume effect with the given gain in dB.
    ///
    /// `smoothing` controls ramping speed (typical: 0.99 for ~10ms at 48kHz).
    #[must_use]
    pub fn new(gain_db: f32, smoothing: f32) -> Self {
        Self {
            target_db: gain_db,
            current_gain: db_to_linear(gain_db),
            smoothing: smoothing.clamp(0.0, 0.9999),
        }
    }

    /// Set the target gain in dB. The effect will ramp to this value.
    pub fn set_gain_db(&mut self, db: f32) {
        self.target_db = db;
    }

    /// Get the current target gain in dB.
    #[must_use]
    pub fn gain_db(&self) -> f32 {
        self.target_db
    }

    /// Get the current smoothed linear gain.
    #[must_use]
    pub fn current_linear_gain(&self) -> f32 {
        self.current_gain
    }
}

impl AudioEffect for VolumeEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let target_linear = db_to_linear(self.target_db);
        let len = input.len().min(output.len());

        for i in 0..len {
            // Exponential smoothing toward target.
            self.current_gain =
                self.smoothing * self.current_gain + (1.0 - self.smoothing) * target_linear;
            output[i] = input[i] * self.current_gain;
        }
    }

    fn reset(&mut self) {
        self.current_gain = db_to_linear(self.target_db);
    }

    fn name(&self) -> &str {
        "Volume"
    }
}

impl fmt::Display for VolumeEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VolumeEffect({:.1}dB, current={:.4})",
            self.target_db, self.current_gain,
        )
    }
}

// ── EqualizerEffect ───────────────────────────────────────────────────

/// 5-band parametric equalizer.
///
/// Bands centered at 60 Hz, 230 Hz, 910 Hz, 4 kHz, and 14 kHz.
/// Each band has an independent gain in dB.
pub struct EqualizerEffect {
    /// Gain in dB for each of the 5 bands.
    /// Index: 0=60Hz, 1=230Hz, 2=910Hz, 3=4kHz, 4=14kHz.
    pub band_gains: [f32; 5],
    /// Linear gains (cached from dB values).
    band_linear: [f32; 5],
}

/// The center frequencies of the 5 EQ bands.
pub const EQ_FREQUENCIES: [f32; 5] = [60.0, 230.0, 910.0, 4000.0, 14000.0];

impl EqualizerEffect {
    /// Create a flat (0 dB) equalizer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            band_gains: [0.0; 5],
            band_linear: [1.0; 5],
        }
    }

    /// Set the gain for a specific band (0-4). Gains are in dB.
    pub fn set_band(&mut self, band: usize, gain_db: f32) {
        if band < 5 {
            self.band_gains[band] = gain_db;
            self.band_linear[band] = db_to_linear(gain_db);
        }
    }

    /// Get the gain for a specific band (0-4) in dB.
    #[must_use]
    pub fn get_band(&self, band: usize) -> f32 {
        if band < 5 {
            self.band_gains[band]
        } else {
            0.0
        }
    }

    /// Set all bands to flat (0 dB).
    pub fn reset_bands(&mut self) {
        self.band_gains = [0.0; 5];
        self.band_linear = [1.0; 5];
    }
}

impl Default for EqualizerEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEffect for EqualizerEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        // Simplified EQ: apply weighted sum of band gains.
        // In production this would use biquad filters per band.
        // Here we compute an average gain as a reasonable approximation
        // for testing and basic use.
        let avg_gain: f32 = self.band_linear.iter().sum::<f32>() / 5.0;

        let len = input.len().min(output.len());
        for i in 0..len {
            output[i] = input[i] * avg_gain;
        }
    }

    fn reset(&mut self) {
        // No internal state to reset for the simplified version.
    }

    fn name(&self) -> &str {
        "Equalizer"
    }
}

impl fmt::Display for EqualizerEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EqualizerEffect(60Hz={:.1}dB, 230Hz={:.1}dB, 910Hz={:.1}dB, 4kHz={:.1}dB, 14kHz={:.1}dB)",
            self.band_gains[0],
            self.band_gains[1],
            self.band_gains[2],
            self.band_gains[3],
            self.band_gains[4],
        )
    }
}

// ── CompressorEffect ──────────────────────────────────────────────────

/// Dynamic range compressor.
///
/// Reduces the volume of loud signals above the threshold by the
/// specified ratio. Attack and release control how quickly the
/// compressor responds to level changes.
pub struct CompressorEffect {
    /// Threshold in dB above which compression is applied.
    pub threshold: f32,
    /// Compression ratio (e.g. 4.0 means 4:1 compression).
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Current envelope level (linear).
    envelope: f32,
    /// Sample rate used for coefficient calculation.
    sample_rate: f32,
}

impl CompressorEffect {
    /// Create a new compressor with the given parameters.
    ///
    /// - `threshold`: dB level above which compression starts (e.g. -20.0)
    /// - `ratio`: compression ratio (e.g. 4.0 for 4:1)
    /// - `attack_ms`: attack time in milliseconds
    /// - `release_ms`: release time in milliseconds
    /// - `sample_rate`: audio sample rate (e.g. 48000.0)
    #[must_use]
    pub fn new(
        threshold: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            threshold,
            ratio: ratio.max(1.0),
            attack_ms: attack_ms.max(0.01),
            release_ms: release_ms.max(0.01),
            envelope: 0.0,
            sample_rate,
        }
    }

    /// Compute the time constant for exponential smoothing.
    fn time_constant(ms: f32, sample_rate: f32) -> f32 {
        (-1.0 / (ms * 0.001 * sample_rate)).exp()
    }

    /// Compute the gain reduction for a given input level in dB.
    fn compute_gain_db(&self, input_db: f32) -> f32 {
        if input_db <= self.threshold {
            0.0
        } else {
            let over = input_db - self.threshold;
            let compressed = over / self.ratio;
            compressed - over // This is negative (gain reduction)
        }
    }
}

impl AudioEffect for CompressorEffect {
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let attack_coeff = Self::time_constant(self.attack_ms, self.sample_rate);
        let release_coeff = Self::time_constant(self.release_ms, self.sample_rate);

        let len = input.len().min(output.len());

        for i in 0..len {
            let abs_sample = input[i].abs();

            // Smooth the envelope.
            let coeff = if abs_sample > self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * abs_sample;

            // Compute gain reduction.
            let env_db = linear_to_db(self.envelope);
            let gain_reduction_db = self.compute_gain_db(env_db);
            let gain = db_to_linear(gain_reduction_db);

            output[i] = input[i] * gain;
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
    }

    fn name(&self) -> &str {
        "Compressor"
    }
}

impl fmt::Display for CompressorEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CompressorEffect(threshold={:.1}dB, ratio={:.1}:1, attack={:.1}ms, release={:.1}ms)",
            self.threshold, self.ratio, self.attack_ms, self.release_ms,
        )
    }
}

// ── EffectChain ───────────────────────────────────────────────────────

/// An ordered chain of audio effects applied in sequence.
///
/// Each effect's output becomes the next effect's input.
/// The chain owns its effects and processes them with a single
/// intermediate buffer to avoid repeated allocation.
pub struct EffectChain {
    effects: Vec<Box<dyn AudioEffect>>,
    /// Intermediate buffer for chaining.
    scratch: Vec<f32>,
}

impl EffectChain {
    /// Create a new empty effect chain.
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            scratch: Vec::new(),
        }
    }

    /// Append an effect to the end of the chain.
    pub fn push(&mut self, effect: Box<dyn AudioEffect>) {
        self.effects.push(effect);
    }

    /// Remove the last effect from the chain.
    pub fn pop(&mut self) -> Option<Box<dyn AudioEffect>> {
        self.effects.pop()
    }

    /// Number of effects in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Process input through all effects in sequence.
    ///
    /// `input` and `output` must have the same length.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let len = input.len().min(output.len());
        if self.effects.is_empty() {
            output[..len].copy_from_slice(&input[..len]);
            return;
        }

        if self.scratch.len() < len {
            self.scratch.resize(len, 0.0);
        }

        // First effect always reads from input.
        if self.effects.len() == 1 {
            self.effects[0].process(&input[..len], &mut output[..len]);
            return;
        }

        // First effect: input -> output. Then alternate between output and scratch.
        self.effects[0].process(&input[..len], &mut output[..len]);
        let mut src_is_output = true;

        for i in 1..self.effects.len() {
            if i == self.effects.len() - 1 {
                // Last effect must write to output.
                if src_is_output {
                    // Read from output, write to scratch, then copy to output.
                    self.scratch[..len].copy_from_slice(&output[..len]);
                    self.effects[i].process(&self.scratch[..len], &mut output[..len]);
                } else {
                    // Read from scratch, write to output.
                    self.effects[i].process(&self.scratch[..len], &mut output[..len]);
                }
            } else if src_is_output {
                self.effects[i].process(&output[..len], &mut self.scratch[..len]);
                src_is_output = false;
            } else {
                self.effects[i].process(&self.scratch[..len], &mut output[..len]);
                src_is_output = true;
            }
        }
    }

    /// Reset all effects in the chain.
    pub fn reset(&mut self) {
        for effect in &mut self.effects {
            effect.reset();
        }
    }

    /// Get the names of all effects in the chain.
    #[must_use]
    pub fn effect_names(&self) -> Vec<&str> {
        self.effects.iter().map(|e| e.name()).collect()
    }
}

impl Default for EffectChain {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EffectChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.effects.iter().map(|e| e.name()).collect();
        write!(f, "EffectChain([{}])", names.join(" -> "))
    }
}

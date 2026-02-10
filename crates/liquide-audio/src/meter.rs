//! Audio level metering — peak and RMS measurement with decay.

use std::fmt;

/// Real-time audio level meter with peak and RMS tracking.
pub struct AudioMeter {
    peak: f32,
    rms: f32,
    decay_rate: f32,
    sample_count: u64,
}

impl AudioMeter {
    /// Create a new audio meter with the given decay rate (e.g. 0.95).
    ///
    /// The decay rate controls how quickly peak and RMS values fall off
    /// when new samples are fed. A value closer to 1.0 means slower decay.
    #[must_use]
    pub fn new(decay_rate: f32) -> Self {
        Self {
            peak: 0.0,
            rms: 0.0,
            decay_rate,
            sample_count: 0,
        }
    }

    /// Feed 32-bit float samples into the meter.
    pub fn feed_samples_f32(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let mut max_abs: f32 = 0.0;
        let mut sum_sq: f64 = 0.0;

        for &s in samples {
            let abs = s.abs();
            if abs > max_abs {
                max_abs = abs;
            }
            sum_sq += (s as f64) * (s as f64);
        }

        let new_rms = (sum_sq / samples.len() as f64).sqrt() as f32;

        // Apply decay to existing values, then take max with new measurement
        self.peak = (self.peak * self.decay_rate).max(max_abs);
        self.rms = (self.rms * self.decay_rate).max(new_rms);
        self.sample_count += samples.len() as u64;
    }

    /// Feed 16-bit signed integer samples into the meter.
    pub fn feed_samples_i16(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        let mut max_abs: f32 = 0.0;
        let mut sum_sq: f64 = 0.0;

        for &s in samples {
            let normalized = s as f32 / i16::MAX as f32;
            let abs = normalized.abs();
            if abs > max_abs {
                max_abs = abs;
            }
            sum_sq += (normalized as f64) * (normalized as f64);
        }

        let new_rms = (sum_sq / samples.len() as f64).sqrt() as f32;

        self.peak = (self.peak * self.decay_rate).max(max_abs);
        self.rms = (self.rms * self.decay_rate).max(new_rms);
        self.sample_count += samples.len() as u64;
    }

    /// Current peak level (linear, 0.0 to 1.0+).
    #[must_use]
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// Current RMS level (linear).
    #[must_use]
    pub fn rms(&self) -> f32 {
        self.rms
    }

    /// Current peak level in decibels (dBFS).
    #[must_use]
    pub fn peak_db(&self) -> f32 {
        if self.peak == 0.0 {
            -f32::INFINITY
        } else {
            20.0 * self.peak.log10()
        }
    }

    /// Current RMS level in decibels (dBFS).
    #[must_use]
    pub fn rms_db(&self) -> f32 {
        if self.rms == 0.0 {
            -f32::INFINITY
        } else {
            20.0 * self.rms.log10()
        }
    }

    /// Reset the meter to silence.
    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.rms = 0.0;
        self.sample_count = 0;
    }
}

impl fmt::Display for AudioMeter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioMeter(peak={:.4} ({:.1}dB), rms={:.4} ({:.1}dB), samples={})",
            self.peak,
            self.peak_db(),
            self.rms,
            self.rms_db(),
            self.sample_count,
        )
    }
}

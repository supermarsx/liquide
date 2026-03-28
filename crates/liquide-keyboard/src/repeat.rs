//! Key repeat timing configuration.
//!
//! When a key is held down, there is an initial delay before repeat starts,
//! then characters are emitted at a steady rate.

/// Key repeat timing parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRepeat {
    /// Initial delay before repeat starts, in milliseconds.
    pub delay_ms: u32,
    /// Interval between repeated key events, in milliseconds.
    pub rate_ms: u32,
}

impl KeyRepeat {
    /// Create a new key repeat configuration.
    pub fn new(delay_ms: u32, rate_ms: u32) -> Self {
        Self { delay_ms, rate_ms }
    }

    /// Repeat rate in keys per second.
    pub fn rate_hz(&self) -> f32 {
        if self.rate_ms == 0 {
            return 0.0;
        }
        1000.0 / self.rate_ms as f32
    }

    /// Whether key repeat is disabled (rate_ms == 0).
    pub fn is_disabled(&self) -> bool {
        self.rate_ms == 0
    }

    /// Slow repeat preset: 660ms delay, 50ms rate (~20 keys/sec).
    pub fn slow() -> Self {
        Self {
            delay_ms: 660,
            rate_ms: 50,
        }
    }

    /// Fast repeat preset: 300ms delay, 20ms rate (~50 keys/sec).
    pub fn fast() -> Self {
        Self {
            delay_ms: 300,
            rate_ms: 20,
        }
    }

    /// Disabled: no repeat at all.
    pub fn disabled() -> Self {
        Self {
            delay_ms: 0,
            rate_ms: 0,
        }
    }
}

impl Default for KeyRepeat {
    /// Default: 500ms delay, 33ms rate (~30 keys/sec).
    fn default() -> Self {
        Self {
            delay_ms: 500,
            rate_ms: 33,
        }
    }
}

/// Tracks the repeat state for a single key.
#[derive(Debug, Clone)]
pub struct KeyRepeatTracker {
    /// The scancode of the key being held, if any.
    held_scancode: Option<u32>,
    /// Microseconds elapsed since the key was first pressed.
    elapsed_us: u64,
    /// Microseconds elapsed since the last repeat event.
    since_last_repeat_us: u64,
    /// Whether the initial delay has passed.
    repeating: bool,
    /// Repeat configuration.
    config: KeyRepeat,
}

impl KeyRepeatTracker {
    /// Create a new tracker with the given repeat config.
    pub fn new(config: KeyRepeat) -> Self {
        Self {
            held_scancode: None,
            elapsed_us: 0,
            since_last_repeat_us: 0,
            repeating: false,
            config,
        }
    }

    /// Notify that a key was pressed.
    pub fn key_down(&mut self, scancode: u32) {
        self.held_scancode = Some(scancode);
        self.elapsed_us = 0;
        self.since_last_repeat_us = 0;
        self.repeating = false;
    }

    /// Notify that a key was released.
    pub fn key_up(&mut self, scancode: u32) {
        if self.held_scancode == Some(scancode) {
            self.held_scancode = None;
            self.repeating = false;
        }
    }

    /// Advance time by `delta_us` microseconds. Returns the number of repeat
    /// events that should be emitted.
    pub fn tick(&mut self, delta_us: u64) -> u32 {
        if self.config.is_disabled() {
            return 0;
        }

        let scancode = match self.held_scancode {
            Some(sc) => sc,
            None => return 0,
        };
        let _ = scancode; // used only to check we have a held key

        self.elapsed_us += delta_us;

        if !self.repeating {
            let delay_us = self.config.delay_ms as u64 * 1000;
            if self.elapsed_us >= delay_us {
                self.repeating = true;
                self.since_last_repeat_us = self.elapsed_us - delay_us;
                // Count how many repeats fit in the overflow.
                let rate_us = self.config.rate_ms as u64 * 1000;
                if rate_us == 0 {
                    return 0;
                }
                let repeats = 1 + (self.since_last_repeat_us / rate_us) as u32;
                self.since_last_repeat_us %= rate_us;
                return repeats;
            }
            return 0;
        }

        // Already repeating — count elapsed intervals.
        self.since_last_repeat_us += delta_us;
        let rate_us = self.config.rate_ms as u64 * 1000;
        if rate_us == 0 {
            return 0;
        }
        let repeats = (self.since_last_repeat_us / rate_us) as u32;
        self.since_last_repeat_us %= rate_us;
        repeats
    }

    /// The scancode currently held, if any.
    pub fn held_scancode(&self) -> Option<u32> {
        self.held_scancode
    }

    /// Whether we are in the repeating phase (past the initial delay).
    pub fn is_repeating(&self) -> bool {
        self.repeating
    }
}

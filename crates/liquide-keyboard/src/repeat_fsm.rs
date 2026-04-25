//! Key repeat state machine with configurable delay and interval.
//!
//! This module provides a higher-level repeat system compared to `repeat.rs`,
//! with a full state machine (Idle/WaitingForDelay/Repeating), modifier
//! filtering, and `RepeatAction` events suitable for event-driven architectures.

use crate::xkb::XkbKeymap;

/// Key repeat configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatConfig {
    /// Initial delay before repeat starts, in milliseconds.
    pub delay_ms: u32,
    /// Interval between repeated key events, in milliseconds.
    pub interval_ms: u32,
}

impl Default for RepeatConfig {
    /// Default: 500ms delay, 33ms interval (~30 Hz).
    fn default() -> Self {
        Self {
            delay_ms: 500,
            interval_ms: 33,
        }
    }
}

impl RepeatConfig {
    /// Create a custom repeat configuration.
    pub fn new(delay_ms: u32, interval_ms: u32) -> Self {
        Self {
            delay_ms,
            interval_ms,
        }
    }

    /// Whether repeat is effectively disabled (zero interval).
    pub fn is_disabled(&self) -> bool {
        self.interval_ms == 0
    }
}

/// Actions produced by the repeat state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatAction {
    /// A key was pressed and the delay timer has started.
    StartDelay(u32),
    /// A repeat event for the given keycode.
    Repeat(u32),
    /// Repeat was cancelled (key released or new key pressed).
    Cancel,
}

/// Internal repeat state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No key is being repeated.
    Idle,
    /// Waiting for the initial delay to elapse.
    WaitingForDelay,
    /// Actively repeating.
    Repeating,
}

/// Key repeat state machine.
///
/// Tracks which key is held, manages the delay-before-repeat and repeat
/// interval timers, and emits `RepeatAction` events. Modifier keys are
/// excluded from repeat when an `XkbKeymap` reference is provided.
#[derive(Debug, Clone)]
pub struct RepeatState {
    config: RepeatConfig,
    phase: Phase,
    /// The keycode currently being repeated (valid when not Idle).
    keycode: u32,
    /// Accumulated time in the current phase, in milliseconds.
    elapsed_ms: u32,
}

impl RepeatState {
    /// Create a new repeat state machine with the given configuration.
    pub fn new(config: RepeatConfig) -> Self {
        Self {
            config,
            phase: Phase::Idle,
            keycode: 0,
            elapsed_ms: 0,
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &RepeatConfig {
        &self.config
    }

    /// Update the repeat configuration. Resets state to Idle.
    pub fn set_config(&mut self, config: RepeatConfig) {
        self.config = config;
        self.phase = Phase::Idle;
        self.keycode = 0;
        self.elapsed_ms = 0;
    }

    /// Whether the state machine is currently in the Idle phase.
    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// Whether the state machine is actively repeating.
    pub fn is_repeating(&self) -> bool {
        self.phase == Phase::Repeating
    }

    /// The keycode being tracked, if not idle.
    pub fn active_keycode(&self) -> Option<u32> {
        if self.phase == Phase::Idle {
            None
        } else {
            Some(self.keycode)
        }
    }

    /// Notify that a key was pressed. If the keycode is a modifier (checked
    /// against `keymap`, if provided), it is ignored for repeat purposes.
    ///
    /// Returns `Some(RepeatAction::StartDelay)` if repeat tracking started,
    /// or `Some(RepeatAction::Cancel)` if a previous repeat was cancelled.
    pub fn key_down(&mut self, keycode: u32, keymap: Option<&XkbKeymap>) -> Option<RepeatAction> {
        // Modifier keys don't repeat.
        if let Some(km) = keymap {
            if km.is_modifier(keycode) {
                return None;
            }
        }

        if self.config.is_disabled() {
            return None;
        }

        let had_active = self.phase != Phase::Idle;

        self.phase = Phase::WaitingForDelay;
        self.keycode = keycode;
        self.elapsed_ms = 0;

        if had_active {
            // Implicitly cancelled the previous key.
            Some(RepeatAction::StartDelay(keycode))
        } else {
            Some(RepeatAction::StartDelay(keycode))
        }
    }

    /// Notify that a key was released. If this is the key being tracked,
    /// repeat is cancelled.
    pub fn key_up(&mut self, keycode: u32) -> Option<RepeatAction> {
        if self.phase != Phase::Idle && self.keycode == keycode {
            self.phase = Phase::Idle;
            self.keycode = 0;
            self.elapsed_ms = 0;
            Some(RepeatAction::Cancel)
        } else {
            None
        }
    }

    /// Advance time and generate repeat events.
    ///
    /// `elapsed_ms` is the number of milliseconds since the last tick.
    /// Returns a list of repeat actions (may be empty, or contain multiple
    /// `Repeat` events if a large time step was provided).
    pub fn tick(&mut self, elapsed_ms: u32) -> Vec<RepeatAction> {
        if self.phase == Phase::Idle || self.config.is_disabled() {
            return Vec::new();
        }

        self.elapsed_ms += elapsed_ms;
        let mut actions = Vec::new();

        match self.phase {
            Phase::WaitingForDelay => {
                if self.elapsed_ms >= self.config.delay_ms {
                    // Transition to repeating.
                    self.phase = Phase::Repeating;
                    let overflow = self.elapsed_ms - self.config.delay_ms;
                    self.elapsed_ms = overflow;
                    // Emit the first repeat.
                    actions.push(RepeatAction::Repeat(self.keycode));
                    // Check for additional repeats in the overflow.
                    if self.config.interval_ms > 0 {
                        while self.elapsed_ms >= self.config.interval_ms {
                            self.elapsed_ms -= self.config.interval_ms;
                            actions.push(RepeatAction::Repeat(self.keycode));
                        }
                    }
                }
            }
            Phase::Repeating => {
                if self.config.interval_ms > 0 {
                    while self.elapsed_ms >= self.config.interval_ms {
                        self.elapsed_ms -= self.config.interval_ms;
                        actions.push(RepeatAction::Repeat(self.keycode));
                    }
                }
            }
            Phase::Idle => unreachable!(),
        }

        actions
    }
}

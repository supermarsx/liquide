//! Keyboard accessibility features following freedesktop / XKB AccessX.
//!
//! Implements:
//! - **StickyKeys**: Modifier acts as one-shot — press once, applies to next key.
//! - **SlowKeys**: Key must be held for a minimum duration to register.
//! - **BounceKeys**: Ignore rapid repeated presses within a debounce window.
//! - **MouseKeys**: Numeric keypad controls the pointer (movement, click).
//!
//! All features can be independently enabled/disabled via `AccessibilityConfig`.

use std::collections::HashMap;

use crate::xkb::ModifierMask;

/// Decision made by the accessibility processor for a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyDecision {
    /// Accept this key event normally.
    Accept,
    /// Reject this key event (filter it out).
    Reject,
    /// Key must be held for the given number of milliseconds (SlowKeys).
    Delay(u32),
    /// A modifier was made sticky (StickyKeys latched it).
    ModifierSticky(ModifierMask),
}

/// Configuration for all accessibility features.
#[derive(Debug, Clone)]
pub struct AccessibilityConfig {
    /// Enable StickyKeys.
    pub sticky_keys_enabled: bool,
    /// Enable SlowKeys.
    pub slow_keys_enabled: bool,
    /// SlowKeys acceptance threshold in milliseconds.
    pub slow_keys_threshold_ms: u32,
    /// Enable BounceKeys.
    pub bounce_keys_enabled: bool,
    /// BounceKeys debounce interval in milliseconds.
    pub bounce_keys_interval_ms: u32,
    /// Enable MouseKeys.
    pub mouse_keys_enabled: bool,
    /// MouseKeys movement step in pixels.
    pub mouse_keys_step: u32,
    /// MouseKeys maximum speed in pixels per tick.
    pub mouse_keys_max_speed: u32,
    /// MouseKeys acceleration delay in ticks before reaching max speed.
    pub mouse_keys_accel_delay: u32,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            sticky_keys_enabled: false,
            slow_keys_enabled: false,
            slow_keys_threshold_ms: 300,
            bounce_keys_enabled: false,
            bounce_keys_interval_ms: 300,
            mouse_keys_enabled: false,
            mouse_keys_step: 10,
            mouse_keys_max_speed: 40,
            mouse_keys_accel_delay: 20,
        }
    }
}

// ── StickyKeys ──────────────────────────────────────────────────────────

/// StickyKeys state: press a modifier once, it applies to the next key.
///
/// In the XKB model this is similar to modifier latching. When a modifier
/// key is pressed and released alone (without another key), it becomes
/// "sticky" (latched). The next non-modifier key press consumes the
/// latched modifier.
#[derive(Debug, Clone)]
pub struct StickyKeys {
    /// Modifiers currently latched (sticky).
    latched: ModifierMask,
    /// Track whether a modifier was used (pressed with another key).
    modifier_used: bool,
    /// The modifier currently being held (if any), to detect standalone press.
    held_modifier: Option<ModifierMask>,
}

impl StickyKeys {
    /// Create a new StickyKeys state.
    pub fn new() -> Self {
        Self {
            latched: ModifierMask::empty(),
            modifier_used: false,
            held_modifier: None,
        }
    }

    /// Get currently latched modifiers.
    pub fn latched(&self) -> ModifierMask {
        self.latched
    }

    /// Notify of a modifier key press.
    pub fn modifier_down(&mut self, mask: ModifierMask) {
        self.held_modifier = Some(mask);
        self.modifier_used = false;
    }

    /// Notify of a modifier key release.
    ///
    /// If the modifier was pressed and released without any other key,
    /// it becomes latched (sticky). Returns `Some(mask)` if latched.
    pub fn modifier_up(&mut self, mask: ModifierMask) -> Option<ModifierMask> {
        if self.held_modifier == Some(mask) && !self.modifier_used {
            // Standalone modifier press — latch it.
            if self.latched.contains(mask) {
                // Already latched — lock it (double-tap to lock, tap again to clear).
                // For simplicity, we just toggle.
                self.latched.remove(mask);
                self.held_modifier = None;
                None
            } else {
                self.latched.insert(mask);
                self.held_modifier = None;
                Some(mask)
            }
        } else {
            self.held_modifier = None;
            None
        }
    }

    /// Notify of a non-modifier key press. Consumes all latched modifiers.
    ///
    /// Returns the modifiers that were active (and now consumed).
    pub fn consume_on_key(&mut self) -> ModifierMask {
        self.modifier_used = true;
        let consumed = self.latched;
        self.latched = ModifierMask::empty();
        consumed
    }

    /// Reset all sticky state.
    pub fn reset(&mut self) {
        self.latched = ModifierMask::empty();
        self.modifier_used = false;
        self.held_modifier = None;
    }
}

impl Default for StickyKeys {
    fn default() -> Self {
        Self::new()
    }
}

// ── SlowKeys ────────────────────────────────────────────────────────────

/// SlowKeys: a key must be held for a minimum duration before it registers.
///
/// Filters out accidental brief key presses. The key_down event is held
/// until the threshold is reached; if the key is released before then,
/// the event is discarded.
#[derive(Debug, Clone)]
pub struct SlowKeys {
    /// Threshold in milliseconds.
    threshold_ms: u32,
    /// Keys currently held and their accumulated hold time.
    pending: HashMap<u32, u32>,
    /// Keys that have passed the threshold and been accepted.
    accepted: HashMap<u32, bool>,
}

impl SlowKeys {
    /// Create a new SlowKeys filter.
    pub fn new(threshold_ms: u32) -> Self {
        Self {
            threshold_ms,
            pending: HashMap::new(),
            accepted: HashMap::new(),
        }
    }

    /// Get the threshold.
    pub fn threshold_ms(&self) -> u32 {
        self.threshold_ms
    }

    /// Set a new threshold.
    pub fn set_threshold(&mut self, ms: u32) {
        self.threshold_ms = ms;
    }

    /// Notify of a key press. Returns `Delay` if the key needs more hold time,
    /// or `Accept` if threshold is 0.
    pub fn key_down(&mut self, keycode: u32) -> KeyDecision {
        if self.threshold_ms == 0 {
            self.accepted.insert(keycode, true);
            return KeyDecision::Accept;
        }
        self.pending.insert(keycode, 0);
        KeyDecision::Delay(self.threshold_ms)
    }

    /// Notify of a key release. Returns `Accept` if the key was accepted,
    /// `Reject` if it was still pending (released too early).
    pub fn key_up(&mut self, keycode: u32) -> KeyDecision {
        self.pending.remove(&keycode);
        if self.accepted.remove(&keycode).is_some() {
            KeyDecision::Accept
        } else {
            KeyDecision::Reject
        }
    }

    /// Advance time for all pending keys. Returns keycodes that have now
    /// passed the threshold and should be accepted.
    pub fn tick(&mut self, elapsed_ms: u32) -> Vec<u32> {
        let mut newly_accepted = Vec::new();
        let threshold = self.threshold_ms;

        self.pending.retain(|&keycode, held_ms| {
            *held_ms += elapsed_ms;
            if *held_ms >= threshold {
                newly_accepted.push(keycode);
                false // remove from pending
            } else {
                true
            }
        });

        for &kc in &newly_accepted {
            self.accepted.insert(kc, true);
        }

        newly_accepted
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.accepted.clear();
    }
}

// ── BounceKeys ──────────────────────────────────────────────────────────

/// BounceKeys: ignore rapid repeated presses of the same key within a
/// debounce window. Prevents key bounce from registering as multiple
/// keypresses.
#[derive(Debug, Clone)]
pub struct BounceKeys {
    /// Debounce interval in milliseconds.
    interval_ms: u32,
    /// Time since the last release of each keycode.
    last_release: HashMap<u32, u32>,
}

impl BounceKeys {
    /// Create a new BounceKeys filter.
    pub fn new(interval_ms: u32) -> Self {
        Self {
            interval_ms,
            last_release: HashMap::new(),
        }
    }

    /// Get the debounce interval.
    pub fn interval_ms(&self) -> u32 {
        self.interval_ms
    }

    /// Set a new debounce interval.
    pub fn set_interval(&mut self, ms: u32) {
        self.interval_ms = ms;
    }

    /// Check whether a key_down event should be accepted or rejected.
    ///
    /// Rejects the event if the same key was released less than
    /// `interval_ms` ago.
    pub fn key_down(&mut self, keycode: u32) -> KeyDecision {
        if let Some(&time_since) = self.last_release.get(&keycode) {
            if time_since < self.interval_ms {
                return KeyDecision::Reject;
            }
        }
        // Accept — remove the entry so we don't block future presses.
        self.last_release.remove(&keycode);
        KeyDecision::Accept
    }

    /// Notify of a key release. Starts the debounce timer for this keycode.
    pub fn key_up(&mut self, keycode: u32) {
        self.last_release.insert(keycode, 0);
    }

    /// Advance time for all debounce timers.
    pub fn tick(&mut self, elapsed_ms: u32) {
        for time in self.last_release.values_mut() {
            *time = time.saturating_add(elapsed_ms);
        }
        // Clean up entries that have passed the interval (no longer needed).
        let interval = self.interval_ms;
        self.last_release.retain(|_, t| *t < interval + 1000);
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.last_release.clear();
    }
}

// ── MouseKeys ───────────────────────────────────────────────────────────

/// Mouse button for MouseKeys emulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Action produced by MouseKeys.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseKeyAction {
    /// Move pointer by (dx, dy) pixels.
    Move(i32, i32),
    /// Press a mouse button.
    ButtonPress(MouseButton),
    /// Release a mouse button.
    ButtonRelease(MouseButton),
    /// Not a mouse key.
    None,
}

/// Numpad keycodes used for mouse control.
const MK_UP: u32 = 72; // KP_8
const MK_DOWN: u32 = 80; // KP_2
const MK_LEFT: u32 = 75; // KP_4
const MK_RIGHT: u32 = 77; // KP_6
const MK_UP_LEFT: u32 = 71; // KP_7
const MK_UP_RIGHT: u32 = 73; // KP_9
const MK_DOWN_LEFT: u32 = 79; // KP_1
const MK_DOWN_RIGHT: u32 = 81; // KP_3
const MK_CLICK: u32 = 76; // KP_5
const MK_BUTTON_SELECT: u32 = 82; // KP_0 (toggle button)

/// MouseKeys: use the numeric keypad to control the pointer.
///
/// KP_8/2/4/6 move in cardinal directions, KP_7/9/1/3 move diagonally,
/// KP_5 clicks, KP_0 selects button. Movement accelerates over time.
#[derive(Debug, Clone)]
pub struct MouseKeys {
    /// Base movement step in pixels.
    step: u32,
    /// Maximum speed in pixels per tick.
    max_speed: u32,
    /// Number of ticks before reaching max speed.
    accel_delay: u32,
    /// Currently selected mouse button.
    selected_button: MouseButton,
    /// Held direction keys and their tick count for acceleration.
    held_directions: HashMap<u32, u32>,
}

impl MouseKeys {
    /// Create with the given parameters.
    pub fn new(step: u32, max_speed: u32, accel_delay: u32) -> Self {
        Self {
            step,
            max_speed,
            accel_delay,
            selected_button: MouseButton::Left,
            held_directions: HashMap::new(),
        }
    }

    /// The currently selected mouse button.
    pub fn selected_button(&self) -> MouseButton {
        self.selected_button
    }

    /// Process a key press. Returns the mouse action, or `None` if not a
    /// mouse key.
    pub fn key_down(&mut self, keycode: u32) -> MouseKeyAction {
        match keycode {
            MK_UP | MK_DOWN | MK_LEFT | MK_RIGHT | MK_UP_LEFT | MK_UP_RIGHT | MK_DOWN_LEFT
            | MK_DOWN_RIGHT => {
                self.held_directions.insert(keycode, 0);
                let step = self.step as i32;
                self.direction_delta(keycode, step)
            }
            MK_CLICK => MouseKeyAction::ButtonPress(self.selected_button),
            MK_BUTTON_SELECT => {
                // Cycle through buttons: Left -> Middle -> Right -> Left.
                self.selected_button = match self.selected_button {
                    MouseButton::Left => MouseButton::Middle,
                    MouseButton::Middle => MouseButton::Right,
                    MouseButton::Right => MouseButton::Left,
                };
                MouseKeyAction::None
            }
            _ => MouseKeyAction::None,
        }
    }

    /// Process a key release.
    pub fn key_up(&mut self, keycode: u32) -> MouseKeyAction {
        self.held_directions.remove(&keycode);
        if keycode == MK_CLICK {
            MouseKeyAction::ButtonRelease(self.selected_button)
        } else {
            MouseKeyAction::None
        }
    }

    /// Advance time and generate movement for held direction keys.
    ///
    /// Returns the accumulated (dx, dy) movement, or `None` if no keys held.
    pub fn tick(&mut self) -> Option<(i32, i32)> {
        if self.held_directions.is_empty() {
            return None;
        }

        // Capture acceleration parameters to avoid borrow conflict.
        let step = self.step;
        let max_speed = self.max_speed;
        let accel_delay = self.accel_delay;

        let mut total_dx = 0i32;
        let mut total_dy = 0i32;

        for (&keycode, ticks) in self.held_directions.iter_mut() {
            *ticks += 1;
            let speed = compute_speed_static(step, max_speed, accel_delay, *ticks);
            let (dx, dy) = match keycode {
                MK_UP => (0, -(speed as i32)),
                MK_DOWN => (0, speed as i32),
                MK_LEFT => (-(speed as i32), 0),
                MK_RIGHT => (speed as i32, 0),
                MK_UP_LEFT => (-(speed as i32), -(speed as i32)),
                MK_UP_RIGHT => (speed as i32, -(speed as i32)),
                MK_DOWN_LEFT => (-(speed as i32), speed as i32),
                MK_DOWN_RIGHT => (speed as i32, speed as i32),
                _ => (0, 0),
            };
            total_dx += dx;
            total_dy += dy;
        }

        if total_dx != 0 || total_dy != 0 {
            Some((total_dx, total_dy))
        } else {
            None
        }
    }

    /// Whether a keycode is a MouseKeys key.
    pub fn is_mouse_key(keycode: u32) -> bool {
        matches!(
            keycode,
            MK_UP
                | MK_DOWN
                | MK_LEFT
                | MK_RIGHT
                | MK_UP_LEFT
                | MK_UP_RIGHT
                | MK_DOWN_LEFT
                | MK_DOWN_RIGHT
                | MK_CLICK
                | MK_BUTTON_SELECT
        )
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.held_directions.clear();
        self.selected_button = MouseButton::Left;
    }

    fn direction_delta(&self, keycode: u32, step: i32) -> MouseKeyAction {
        let (dx, dy) = match keycode {
            MK_UP => (0, -step),
            MK_DOWN => (0, step),
            MK_LEFT => (-step, 0),
            MK_RIGHT => (step, 0),
            MK_UP_LEFT => (-step, -step),
            MK_UP_RIGHT => (step, -step),
            MK_DOWN_LEFT => (-step, step),
            MK_DOWN_RIGHT => (step, step),
            _ => (0, 0),
        };
        if dx != 0 || dy != 0 {
            MouseKeyAction::Move(dx, dy)
        } else {
            MouseKeyAction::None
        }
    }
}

/// Compute mouse key speed with acceleration (free function to avoid borrow conflicts).
fn compute_speed_static(step: u32, max_speed: u32, accel_delay: u32, ticks: u32) -> u32 {
    if accel_delay == 0 {
        return max_speed;
    }
    let progress = (ticks as f64 / accel_delay as f64).min(1.0);
    let range = max_speed.saturating_sub(step);
    step + (range as f64 * progress) as u32
}

// ── Unified processor ───────────────────────────────────────────────────

/// Process a key event through all enabled accessibility features.
///
/// This is the main entry point that applies StickyKeys, SlowKeys,
/// BounceKeys, and MouseKeys in the correct order. The caller provides
/// the keycode, whether it's a press, whether it's a modifier, and the
/// optional modifier mask (for StickyKeys).
pub fn process_key(
    keycode: u32,
    pressed: bool,
    is_modifier: bool,
    modifier_mask: Option<ModifierMask>,
    config: &AccessibilityConfig,
    sticky: &mut StickyKeys,
    slow: &mut SlowKeys,
    bounce: &mut BounceKeys,
) -> KeyDecision {
    // MouseKeys intercepts numpad keys — handled separately by the caller.

    if pressed {
        // BounceKeys: reject if too soon after last release.
        if config.bounce_keys_enabled {
            let decision = bounce.key_down(keycode);
            if decision == KeyDecision::Reject {
                return KeyDecision::Reject;
            }
        }

        // SlowKeys: require hold duration.
        if config.slow_keys_enabled && !is_modifier {
            let decision = slow.key_down(keycode);
            if let KeyDecision::Delay(ms) = decision {
                return KeyDecision::Delay(ms);
            }
        }

        // StickyKeys: handle modifier or consume on non-modifier.
        if config.sticky_keys_enabled {
            if is_modifier {
                if let Some(mask) = modifier_mask {
                    sticky.modifier_down(mask);
                }
                return KeyDecision::Accept;
            } else {
                let consumed = sticky.consume_on_key();
                if !consumed.is_empty() {
                    return KeyDecision::ModifierSticky(consumed);
                }
            }
        }

        KeyDecision::Accept
    } else {
        // Key release.
        if config.bounce_keys_enabled {
            bounce.key_up(keycode);
        }

        if config.slow_keys_enabled && !is_modifier {
            return slow.key_up(keycode);
        }

        if config.sticky_keys_enabled && is_modifier {
            if let Some(mask) = modifier_mask {
                if let Some(latched) = sticky.modifier_up(mask) {
                    return KeyDecision::ModifierSticky(latched);
                }
            }
        }

        KeyDecision::Accept
    }
}

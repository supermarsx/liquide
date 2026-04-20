use crate::easing::EasingFunction;
use std::time::{Instant, Duration};

/// Rectangle
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, width: w, height: h }
    }

    pub fn lerp(&self, other: &Rect, t: f32) -> Rect {
        Rect {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            width: self.width + (other.width - self.width) * t,
            height: self.height + (other.height - self.height) * t,
        }
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Window effect types
#[derive(Debug, Clone)]
pub enum WindowEffect {
    /// Window appearing (open, unminimize)
    Open {
        window_id: u64,
        from: Rect,
        to: Rect,
        opacity_from: f32,
        opacity_to: f32,
    },
    /// Window disappearing (close, minimize)
    Close {
        window_id: u64,
        from: Rect,
        to: Rect,
        opacity_from: f32,
        opacity_to: f32,
    },
    /// Window moving/resizing (maximize, restore, tile)
    Transform {
        window_id: u64,
        from: Rect,
        to: Rect,
    },
    /// Window focus highlight (subtle scale pulse)
    FocusIn {
        window_id: u64,
        bounds: Rect,
    },
    /// Fullscreen transition
    Fullscreen {
        window_id: u64,
        from: Rect,
        to: Rect,
    },
}

/// State of an active effect
#[derive(Debug)]
pub struct EffectState {
    pub effect: WindowEffect,
    pub easing: EasingFunction,
    pub duration: Duration,
    pub started_at: Instant,
    pub progress: f32,  // 0.0 to 1.0
    pub finished: bool,
}

impl EffectState {
    pub fn new(effect: WindowEffect, easing: EasingFunction, duration: Duration) -> Self {
        Self {
            effect,
            easing,
            duration,
            started_at: Instant::now(),
            progress: 0.0,
            finished: false,
        }
    }

    /// Create an effect state with an explicit start time for deterministic testing.
    pub fn new_with_start(
        effect: WindowEffect,
        easing: EasingFunction,
        duration: Duration,
        start: Instant,
    ) -> Self {
        Self {
            effect,
            easing,
            duration,
            started_at: start,
            progress: 0.0,
            finished: false,
        }
    }

    /// Update progress based on elapsed time. Returns current interpolated values.
    pub fn update(&mut self) -> EffectFrame {
        self.update_with_now(Instant::now())
    }

    /// Update progress using an explicit `now` instant, enabling deterministic testing.
    pub fn update_with_now(&mut self, now: Instant) -> EffectFrame {
        let elapsed = now.duration_since(self.started_at);
        let raw_t = if self.duration.as_secs_f32() > 0.0 {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        } else {
            1.0
        };

        self.progress = raw_t.min(1.0);
        let eased_t = self.easing.eval(self.progress);

        if self.progress >= 1.0 {
            self.finished = true;
        }

        match &self.effect {
            WindowEffect::Open { window_id, from, to, opacity_from, opacity_to } => {
                EffectFrame {
                    window_id: *window_id,
                    bounds: from.lerp(to, eased_t),
                    opacity: opacity_from + (opacity_to - opacity_from) * eased_t,
                    scale: 1.0,
                    finished: self.finished,
                }
            }
            WindowEffect::Close { window_id, from, to, opacity_from, opacity_to } => {
                EffectFrame {
                    window_id: *window_id,
                    bounds: from.lerp(to, eased_t),
                    opacity: opacity_from + (opacity_to - opacity_from) * eased_t,
                    scale: 1.0,
                    finished: self.finished,
                }
            }
            WindowEffect::Transform { window_id, from, to } => {
                EffectFrame {
                    window_id: *window_id,
                    bounds: from.lerp(to, eased_t),
                    opacity: 1.0,
                    scale: 1.0,
                    finished: self.finished,
                }
            }
            WindowEffect::FocusIn { window_id, bounds } => {
                // Subtle scale pulse: 1.0 -> 1.02 -> 1.0
                let scale = if eased_t < 0.5 {
                    1.0 + 0.02 * (eased_t * 2.0)
                } else {
                    1.02 - 0.02 * ((eased_t - 0.5) * 2.0)
                };
                EffectFrame {
                    window_id: *window_id,
                    bounds: *bounds,
                    opacity: 1.0,
                    scale,
                    finished: self.finished,
                }
            }
            WindowEffect::Fullscreen { window_id, from, to } => {
                EffectFrame {
                    window_id: *window_id,
                    bounds: from.lerp(to, eased_t),
                    opacity: 1.0,
                    scale: 1.0,
                    finished: self.finished,
                }
            }
        }
    }
}

/// Current frame output of an effect
#[derive(Debug, Clone)]
pub struct EffectFrame {
    pub window_id: u64,
    pub bounds: Rect,
    pub opacity: f32,
    pub scale: f32,
    pub finished: bool,
}

/// Manages all active window effects
pub struct EffectManager {
    active_effects: Vec<EffectState>,
    /// Reduce motion preference
    reduce_motion: bool,
    /// Default durations
    open_duration: Duration,
    close_duration: Duration,
    transform_duration: Duration,
    focus_duration: Duration,
}

impl EffectManager {
    pub fn new() -> Self {
        Self {
            active_effects: Vec::new(),
            reduce_motion: false,
            open_duration: Duration::from_millis(200),
            close_duration: Duration::from_millis(150),
            transform_duration: Duration::from_millis(250),
            focus_duration: Duration::from_millis(150),
        }
    }

    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.reduce_motion = reduce;
    }

    /// Start a window open effect
    pub fn open_window(&mut self, window_id: u64, target: Rect) {
        if self.reduce_motion {
            return; // skip animation
        }

        // Scale from 95% size + fade in
        let (cx, cy) = target.center();
        let from = Rect::new(
            cx - target.width * 0.475,
            cy - target.height * 0.475,
            target.width * 0.95,
            target.height * 0.95,
        );

        let effect = WindowEffect::Open {
            window_id,
            from,
            to: target,
            opacity_from: 0.0,
            opacity_to: 1.0,
        };

        self.cancel_effects_for(window_id);
        self.active_effects.push(EffectState::new(effect, EasingFunction::EaseOutCubic, self.open_duration));
    }

    /// Start a window close effect
    pub fn close_window(&mut self, window_id: u64, current: Rect) {
        if self.reduce_motion {
            return;
        }

        let (cx, cy) = current.center();
        let to = Rect::new(
            cx - current.width * 0.475,
            cy - current.height * 0.475,
            current.width * 0.95,
            current.height * 0.95,
        );

        let effect = WindowEffect::Close {
            window_id,
            from: current,
            to,
            opacity_from: 1.0,
            opacity_to: 0.0,
        };

        self.cancel_effects_for(window_id);
        self.active_effects.push(EffectState::new(effect, EasingFunction::EaseIn, self.close_duration));
    }

    /// Start a window transform effect (move/resize/maximize/restore)
    pub fn transform_window(&mut self, window_id: u64, from: Rect, to: Rect) {
        if self.reduce_motion {
            return;
        }

        let effect = WindowEffect::Transform { window_id, from, to };
        self.cancel_effects_for(window_id);
        self.active_effects.push(EffectState::new(effect, EasingFunction::EaseOutCubic, self.transform_duration));
    }

    /// Start a focus highlight effect
    pub fn focus_window(&mut self, window_id: u64, bounds: Rect) {
        if self.reduce_motion {
            return;
        }

        let effect = WindowEffect::FocusIn { window_id, bounds };
        self.active_effects.push(EffectState::new(effect, EasingFunction::EaseInOut, self.focus_duration));
    }

    /// Update all active effects. Returns frames for each active effect.
    pub fn tick(&mut self) -> Vec<EffectFrame> {
        let mut frames = Vec::new();

        for state in &mut self.active_effects {
            frames.push(state.update());
        }

        // Remove finished effects
        self.active_effects.retain(|s| !s.finished);

        frames
    }

    /// Cancel all effects for a window
    pub fn cancel_effects_for(&mut self, window_id: u64) {
        self.active_effects.retain(|s| {
            let id = match &s.effect {
                WindowEffect::Open { window_id, .. } => *window_id,
                WindowEffect::Close { window_id, .. } => *window_id,
                WindowEffect::Transform { window_id, .. } => *window_id,
                WindowEffect::FocusIn { window_id, .. } => *window_id,
                WindowEffect::Fullscreen { window_id, .. } => *window_id,
            };
            id != window_id
        });
    }

    /// Are any effects active?
    pub fn has_active_effects(&self) -> bool {
        !self.active_effects.is_empty()
    }

    /// Number of active effects
    pub fn active_count(&self) -> usize {
        self.active_effects.len()
    }

    /// Is a specific window currently animating?
    pub fn is_animating(&self, window_id: u64) -> bool {
        self.active_effects.iter().any(|s| {
            match &s.effect {
                WindowEffect::Open { window_id: id, .. } => *id == window_id,
                WindowEffect::Close { window_id: id, .. } => *id == window_id,
                WindowEffect::Transform { window_id: id, .. } => *id == window_id,
                WindowEffect::FocusIn { window_id: id, .. } => *id == window_id,
                WindowEffect::Fullscreen { window_id: id, .. } => *id == window_id,
            }
        })
    }
}

impl Default for EffectManager {
    fn default() -> Self { Self::new() }
}

//! Cursor rendering modes, prediction, and smoothing.

use std::fmt;

/// How the cursor is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    LocalPredict,
    ServerRendered,
    HiddenLocal,
    Dual,
}

impl fmt::Display for CursorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::LocalPredict => "LocalPredict",
            Self::ServerRendered => "ServerRendered",
            Self::HiddenLocal => "HiddenLocal",
            Self::Dual => "Dual",
        };
        f.write_str(label)
    }
}

/// Smoothing algorithm for cursor correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothingStrategy {
    Linear,
    Spring,
    Bezier,
    None,
}

impl fmt::Display for SmoothingStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Linear => "Linear",
            Self::Spring => "Spring",
            Self::Bezier => "Bezier",
            Self::None => "None",
        };
        f.write_str(label)
    }
}

/// A 2-D cursor position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

impl CursorPosition {
    /// Euclidean distance to another position.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// Snapshot of cursor state at a given instant.
#[derive(Debug, Clone)]
pub struct CursorState {
    pub mode: CursorMode,
    pub local_position: CursorPosition,
    pub server_position: CursorPosition,
    pub visible: bool,
    pub idle_timer_ms: u64,
    pub shape_hash: u64,
}

/// Predicts cursor position locally and smoothly corrects towards the
/// authoritative server position.
pub struct CursorPredictor {
    local_pos: CursorPosition,
    server_pos: CursorPosition,
    correction_frames: u32,
    frame_count: u32,
    smoothing: SmoothingStrategy,
    max_correction_distance: f64,
}

impl CursorPredictor {
    /// Build a new predictor.
    #[must_use]
    pub fn new(correction_frames: u32, smoothing: SmoothingStrategy) -> Self {
        Self {
            local_pos: CursorPosition { x: 0.0, y: 0.0 },
            server_pos: CursorPosition { x: 0.0, y: 0.0 },
            correction_frames,
            frame_count: 0,
            smoothing,
            max_correction_distance: 50.0,
        }
    }

    /// Update with a new local (client-side) position.
    pub fn update_local(&mut self, x: f64, y: f64) {
        self.local_pos = CursorPosition { x, y };
    }

    /// Update with a new authoritative server position.
    pub fn update_server(&mut self, x: f64, y: f64) {
        self.server_pos = CursorPosition { x, y };
        self.frame_count = 0;
    }

    /// Compute the predicted cursor position for the current frame.
    #[must_use]
    pub fn predicted_position(&self) -> CursorPosition {
        if !self.needs_correction() || self.correction_frames == 0 {
            return self.local_pos;
        }

        let t = (self.frame_count as f64 / self.correction_frames as f64).min(1.0);
        let factor = self.smoothing_factor(t);

        CursorPosition {
            x: self.local_pos.x + (self.server_pos.x - self.local_pos.x) * factor,
            y: self.local_pos.y + (self.server_pos.y - self.local_pos.y) * factor,
        }
    }

    /// The last known server position.
    #[must_use]
    pub fn server_position(&self) -> CursorPosition {
        self.server_pos
    }

    /// Whether the local position has drifted far enough from the server
    /// position to warrant a correction.
    #[must_use]
    pub fn needs_correction(&self) -> bool {
        self.local_pos.distance_to(&self.server_pos) > self.max_correction_distance
    }

    /// Advance one frame of correction interpolation.
    pub fn apply_correction(&mut self) {
        if self.frame_count < self.correction_frames {
            self.frame_count += 1;
            let pos = self.predicted_position();
            self.local_pos = pos;
        }
    }

    /// Reset the predictor to origin.
    pub fn reset(&mut self) {
        self.local_pos = CursorPosition { x: 0.0, y: 0.0 };
        self.server_pos = CursorPosition { x: 0.0, y: 0.0 };
        self.frame_count = 0;
    }

    /// Compute the interpolation factor for the given normalised time `t`.
    fn smoothing_factor(&self, t: f64) -> f64 {
        match self.smoothing {
            SmoothingStrategy::Linear => t,
            SmoothingStrategy::Spring => {
                // Simple spring-style ease-out.
                1.0 - (1.0 - t).powi(3)
            }
            SmoothingStrategy::Bezier => {
                // Cubic ease-in-out approximation.
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            SmoothingStrategy::None => 1.0,
        }
    }
}

/// State for the dual-cursor mode (local dot + server cursor).
#[derive(Debug, Clone)]
pub struct DualCursorState {
    pub local_dot_pos: CursorPosition,
    pub server_cursor_pos: CursorPosition,
    pub dot_size: u32,
    pub dot_opacity: f32,
}

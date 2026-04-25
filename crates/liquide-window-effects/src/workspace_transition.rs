use crate::easing::EasingFunction;
use std::time::{Duration, Instant};

/// Workspace transition direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionDirection {
    Left,
    Right,
    Up,
    Down,
    FadeOnly,
}

/// Workspace transition state
pub struct WorkspaceTransition {
    pub active: bool,
    pub direction: TransitionDirection,
    pub from_workspace: u32,
    pub to_workspace: u32,
    pub progress: f32,
    pub easing: EasingFunction,
    pub duration: Duration,
    started_at: Option<Instant>,
}

impl WorkspaceTransition {
    pub fn new() -> Self {
        Self {
            active: false,
            direction: TransitionDirection::Left,
            from_workspace: 0,
            to_workspace: 0,
            progress: 0.0,
            easing: EasingFunction::EaseOutCubic,
            duration: Duration::from_millis(300),
            started_at: None,
        }
    }

    /// Start a workspace transition
    pub fn start(&mut self, from: u32, to: u32, direction: TransitionDirection) {
        self.active = true;
        self.from_workspace = from;
        self.to_workspace = to;
        self.direction = direction;
        self.progress = 0.0;
        self.started_at = Some(Instant::now());
    }

    /// Update the transition. Returns the offset to apply to workspace rendering.
    pub fn update(&mut self) -> TransitionFrame {
        if !self.active {
            return TransitionFrame::default();
        }

        let elapsed = self.started_at.map(|s| s.elapsed()).unwrap_or_default();
        let raw_t = if self.duration.as_secs_f32() > 0.0 {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        } else {
            1.0
        };

        self.progress = raw_t.min(1.0);
        let eased = self.easing.eval(self.progress);

        if self.progress >= 1.0 {
            self.active = false;
            return TransitionFrame {
                offset_x: 0.0,
                offset_y: 0.0,
                old_opacity: 0.0,
                new_opacity: 1.0,
                finished: true,
            };
        }

        let (ox, oy) = match self.direction {
            TransitionDirection::Left => (-eased, 0.0),
            TransitionDirection::Right => (eased, 0.0),
            TransitionDirection::Up => (0.0, -eased),
            TransitionDirection::Down => (0.0, eased),
            TransitionDirection::FadeOnly => (0.0, 0.0),
        };

        TransitionFrame {
            offset_x: ox,
            offset_y: oy,
            old_opacity: 1.0 - eased,
            new_opacity: eased,
            finished: false,
        }
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.started_at = None;
    }
}

impl Default for WorkspaceTransition {
    fn default() -> Self {
        Self::new()
    }
}

/// Output frame for workspace transition
#[derive(Debug, Clone, Default)]
pub struct TransitionFrame {
    /// Horizontal offset (-1.0 to 1.0, as fraction of screen width)
    pub offset_x: f32,
    /// Vertical offset (-1.0 to 1.0, as fraction of screen height)
    pub offset_y: f32,
    /// Opacity of the old workspace (fading out)
    pub old_opacity: f32,
    /// Opacity of the new workspace (fading in)
    pub new_opacity: f32,
    pub finished: bool,
}

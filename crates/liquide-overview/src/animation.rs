use crate::layout::{OverviewRect, OverviewSlot};

/// Phase of the overview animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewPhase {
    /// Windows are scaling down from their original positions to the grid.
    Entering,
    /// The overview grid is fully visible and interactive.
    Active,
    /// Windows are scaling back to their original positions.
    Exiting,
    /// The overview is not visible.
    Hidden,
}

/// A single slot with its current interpolated position and opacity.
#[derive(Debug, Clone)]
pub struct AnimatedSlot {
    pub slot: OverviewSlot,
    pub current: OverviewRect,
    pub opacity: f32,
}

/// Drives the enter / exit animation for the overview.
pub struct OverviewAnimator {
    pub enter_duration_ms: f32,
    pub exit_duration_ms: f32,
    pub phase: OverviewPhase,
    pub progress: f32,
    slots: Vec<AnimatedSlot>,
    /// Original rects keyed by window id (for enter/exit interpolation).
    originals: Vec<(u64, OverviewRect)>,
    /// When true, uses a linear curve (prefers-reduced-motion).
    reduced_motion: bool,
    /// When true, uses an over-damped spring curve instead of ease_out_cubic.
    spring: bool,
}

impl OverviewAnimator {
    pub fn new() -> Self {
        Self {
            enter_duration_ms: 300.0,
            exit_duration_ms: 250.0,
            phase: OverviewPhase::Hidden,
            progress: 0.0,
            slots: Vec::new(),
            originals: Vec::new(),
            reduced_motion: false,
            spring: false,
        }
    }

    /// Begin the enter animation: windows move from their original positions to
    /// the computed grid slots.
    pub fn begin_enter(&mut self, slots: Vec<OverviewSlot>, originals: &[(u64, OverviewRect)]) {
        self.originals = originals.to_vec();
        self.slots = slots
            .into_iter()
            .map(|slot| {
                let orig = find_original(&self.originals, slot.window_id).unwrap_or(slot.target);
                AnimatedSlot {
                    slot,
                    current: orig,
                    opacity: 0.0,
                }
            })
            .collect();
        self.phase = OverviewPhase::Entering;
        self.progress = 0.0;
    }

    /// Begin the exit animation: windows move back to their original positions.
    ///
    /// If called while `Entering`, snapshots the current interpolated rect so
    /// the exit animation starts from the visible on-screen position — not
    /// from the fully-enlarged grid slot. This prevents a visual "snap".
    pub fn begin_exit(&mut self) {
        if self.phase == OverviewPhase::Hidden {
            return;
        }

        // Snapshot current interpolated rects BEFORE flipping phase.
        // The exit interpolation uses `anim.slot.target` as its source, so we
        // override that with the current rect. Originals are left intact so
        // windows still land at their restored positions.
        if self.phase == OverviewPhase::Entering {
            for anim in &mut self.slots {
                anim.slot.target = anim.current;
            }
        }

        self.phase = OverviewPhase::Exiting;
        self.progress = 0.0;
    }

    /// Use a critically-damped spring curve instead of ease-out-cubic.
    ///
    /// When `reduced_motion` is true the animator will use a linear curve
    /// (respecting `prefers-reduced-motion`).
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// Enable the spring-option (critically damped).
    pub fn set_spring(&mut self, spring: bool) {
        self.spring = spring;
    }

    fn ease(&self, t: f32) -> f32 {
        if self.reduced_motion {
            t.clamp(0.0, 1.0)
        } else if self.spring {
            spring_out(t)
        } else {
            ease_out_cubic(t)
        }
    }

    /// Advance the animation by `dt_ms` milliseconds.
    ///
    /// Returns `true` while the animation is still in progress.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        match self.phase {
            OverviewPhase::Entering => {
                self.progress += dt_ms / self.enter_duration_ms;
                if self.progress >= 1.0 {
                    self.progress = 1.0;
                    self.phase = OverviewPhase::Active;
                }
                let t = self.ease(self.progress);
                self.interpolate_enter(t);
                self.phase != OverviewPhase::Active
            }
            OverviewPhase::Exiting => {
                self.progress += dt_ms / self.exit_duration_ms;
                if self.progress >= 1.0 {
                    self.progress = 1.0;
                    self.phase = OverviewPhase::Hidden;
                }
                let t = self.ease(self.progress);
                self.interpolate_exit(t);
                self.phase != OverviewPhase::Hidden
            }
            _ => false,
        }
    }

    /// Access the current animated slot positions.
    pub fn animated_slots(&self) -> &[AnimatedSlot] {
        &self.slots
    }

    /// Whether the overview has any visible content.
    pub fn is_visible(&self) -> bool {
        self.phase != OverviewPhase::Hidden
    }

    // ── internals ───────────────────────────────────────────────

    fn interpolate_enter(&mut self, t: f32) {
        for anim in &mut self.slots {
            let orig =
                find_original(&self.originals, anim.slot.window_id).unwrap_or(anim.slot.target);
            anim.current = lerp_rect(&orig, &anim.slot.target, t);
            anim.opacity = t;
        }
    }

    fn interpolate_exit(&mut self, t: f32) {
        for anim in &mut self.slots {
            let orig =
                find_original(&self.originals, anim.slot.window_id).unwrap_or(anim.slot.target);
            anim.current = lerp_rect(&anim.slot.target, &orig, t);
            anim.opacity = 1.0 - t;
        }
    }
}

fn find_original(originals: &[(u64, OverviewRect)], id: u64) -> Option<OverviewRect> {
    originals
        .iter()
        .find(|(wid, _)| *wid == id)
        .map(|(_, r)| *r)
}

/// Cubic ease-out: `1 - (1-t)^3`.
pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// Over-damped spring approximation in `[0, 1]` reaching 1 at t=1.
///
/// Uses `1 - exp(-6t)` normalized so that `f(0) = 0` and `f(1) = 1`.
pub fn spring_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let k = 6.0_f32;
    let raw = 1.0 - (-k * t).exp();
    let norm = 1.0 - (-k).exp();
    (raw / norm).clamp(0.0, 1.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_rect(a: &OverviewRect, b: &OverviewRect, t: f32) -> OverviewRect {
    OverviewRect {
        x: lerp(a.x, b.x, t),
        y: lerp(a.y, b.y, t),
        width: lerp(a.width, b.width, t),
        height: lerp(a.height, b.height, t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::OverviewSlot;

    fn make_slot(id: u64) -> OverviewSlot {
        OverviewSlot {
            window_id: id,
            target: OverviewRect::new(200.0, 200.0, 400.0, 300.0),
            scale: 0.5,
            label_y: 504.0,
        }
    }

    fn make_original(id: u64) -> (u64, OverviewRect) {
        (id, OverviewRect::new(0.0, 0.0, 800.0, 600.0))
    }

    #[test]
    fn enter_at_t0_is_original() {
        let mut anim = OverviewAnimator::new();
        let slots = vec![make_slot(1)];
        let originals = vec![make_original(1)];
        anim.begin_enter(slots, &originals);
        // At progress=0 the current position should equal the original.
        let cur = &anim.animated_slots()[0].current;
        assert!((cur.x - 0.0).abs() < 0.01);
        assert!((cur.y - 0.0).abs() < 0.01);
        assert!((cur.width - 800.0).abs() < 0.01);
    }

    #[test]
    fn enter_at_t1_is_target() {
        let mut anim = OverviewAnimator::new();
        let slots = vec![make_slot(1)];
        let originals = vec![make_original(1)];
        anim.begin_enter(slots, &originals);
        // Tick past the full duration.
        anim.tick(anim.enter_duration_ms + 10.0);
        let cur = &anim.animated_slots()[0].current;
        assert!((cur.x - 200.0).abs() < 0.5);
        assert!((cur.width - 400.0).abs() < 0.5);
        assert_eq!(anim.phase, OverviewPhase::Active);
    }

    #[test]
    fn exit_reversal() {
        let mut anim = OverviewAnimator::new();
        let slots = vec![make_slot(1)];
        let originals = vec![make_original(1)];
        anim.begin_enter(slots, &originals);
        anim.tick(anim.enter_duration_ms + 10.0);
        assert_eq!(anim.phase, OverviewPhase::Active);

        anim.begin_exit();
        assert_eq!(anim.phase, OverviewPhase::Exiting);
        anim.tick(anim.exit_duration_ms + 10.0);
        assert_eq!(anim.phase, OverviewPhase::Hidden);
        // Should be back near the original position.
        let cur = &anim.animated_slots()[0].current;
        assert!((cur.x - 0.0).abs() < 0.5);
        assert!((cur.width - 800.0).abs() < 0.5);
    }

    #[test]
    fn ease_out_cubic_boundaries() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 0.001);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn ease_out_cubic_midpoint() {
        let mid = ease_out_cubic(0.5);
        // Should be > 0.5 (ease-out is front-loaded).
        assert!(mid > 0.5);
        assert!(mid < 1.0);
    }

    #[test]
    fn ease_out_cubic_monotonic() {
        let mut prev = 0.0f32;
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let v = ease_out_cubic(t);
            assert!(v >= prev - 0.0001);
            prev = v;
        }
    }

    #[test]
    fn tick_returns_false_when_hidden() {
        let mut anim = OverviewAnimator::new();
        assert!(!anim.tick(16.0));
    }

    #[test]
    fn is_visible_tracks_phase() {
        let mut anim = OverviewAnimator::new();
        assert!(!anim.is_visible());
        anim.begin_enter(vec![make_slot(1)], &[make_original(1)]);
        assert!(anim.is_visible());
    }

    #[test]
    fn enter_partial_progress() {
        let mut anim = OverviewAnimator::new();
        anim.begin_enter(vec![make_slot(1)], &[make_original(1)]);
        // Tick half-way.
        anim.tick(anim.enter_duration_ms / 2.0);
        assert_eq!(anim.phase, OverviewPhase::Entering);
        let cur = &anim.animated_slots()[0].current;
        // Should be between original (0) and target (200).
        assert!(cur.x > 0.0);
        assert!(cur.x < 200.0);
    }

    #[test]
    fn opacity_during_enter() {
        let mut anim = OverviewAnimator::new();
        anim.begin_enter(vec![make_slot(1)], &[make_original(1)]);
        assert!((anim.animated_slots()[0].opacity - 0.0).abs() < 0.01);
        anim.tick(anim.enter_duration_ms + 10.0);
        assert!((anim.animated_slots()[0].opacity - 1.0).abs() < 0.01);
    }

    #[test]
    fn begin_exit_from_hidden_is_noop() {
        let mut anim = OverviewAnimator::new();
        anim.begin_exit();
        assert_eq!(anim.phase, OverviewPhase::Hidden);
    }

    #[test]
    fn exit_during_enter_starts_from_current() {
        let mut anim = OverviewAnimator::new();
        anim.begin_enter(vec![make_slot(1)], &[make_original(1)]);
        // Tick part-way through enter: 30% of 300 ms = 90 ms.
        anim.tick(anim.enter_duration_ms * 0.3);
        let mid = anim.animated_slots()[0].current;
        assert!(mid.x > 0.0 && mid.x < 200.0, "enter partial: {:?}", mid);

        // Interrupt — exit should start from this interpolated position.
        anim.begin_exit();
        let start = anim.animated_slots()[0].current;
        assert!(
            (start.x - mid.x).abs() < 0.01,
            "exit begin must snapshot current"
        );
        // At very small dt the position should be still near `mid`, not snap to grid.
        anim.tick(1.0);
        let after = anim.animated_slots()[0].current;
        let drift = (after.x - mid.x).abs();
        assert!(drift < 20.0, "no snap — drift {} after 1 ms", drift);
    }

    #[test]
    fn reduced_motion_uses_linear() {
        let mut anim = OverviewAnimator::new();
        anim.set_reduced_motion(true);
        anim.begin_enter(vec![make_slot(1)], &[make_original(1)]);
        anim.tick(anim.enter_duration_ms * 0.5);
        // At 50% progress with linear curve, x should be midpoint of 0..200 = 100.
        let cur = anim.animated_slots()[0].current;
        assert!((cur.x - 100.0).abs() < 5.0, "linear at 50%: x={}", cur.x);
    }

    #[test]
    fn spring_out_endpoints() {
        assert!((spring_out(0.0) - 0.0).abs() < 0.001);
        assert!((spring_out(1.0) - 1.0).abs() < 0.001);
    }
}

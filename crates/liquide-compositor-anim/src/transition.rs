use crate::easing::{self, EasingFunction};
use crate::keyframe::{AnimValue, lerp_value};

/// The state of a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionState {
    /// Waiting for delay to elapse.
    Pending,
    /// Actively interpolating.
    Running,
    /// Reached the target value.
    Complete,
}

/// A CSS-style transition: a simple interpolation from one value to another
/// over a specified duration with an easing function.
pub struct Transition {
    /// The property name being transitioned.
    pub property: String,
    /// Starting value.
    pub from: AnimValue,
    /// Target value.
    pub to: AnimValue,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Delay before the transition begins, in milliseconds.
    pub delay_ms: f32,
    /// Easing function.
    pub easing: EasingFunction,
    /// Total elapsed time since creation.
    pub elapsed_ms: f32,
    /// Current state.
    pub state: TransitionState,
}

impl Transition {
    /// Create a new transition. Starts in `Pending` state.
    pub fn new(
        property: String,
        from: AnimValue,
        to: AnimValue,
        duration_ms: f32,
        easing: EasingFunction,
    ) -> Self {
        Self {
            property,
            from,
            to,
            duration_ms,
            delay_ms: 0.0,
            easing,
            elapsed_ms: 0.0,
            state: TransitionState::Pending,
        }
    }

    /// Advance the transition by `dt_ms` milliseconds.
    ///
    /// Returns `true` if the transition is still running, `false` if complete.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        if self.state == TransitionState::Complete {
            return false;
        }

        self.elapsed_ms += dt_ms;

        if self.elapsed_ms < self.delay_ms {
            self.state = TransitionState::Pending;
            return true;
        }

        let active = self.elapsed_ms - self.delay_ms;
        if self.duration_ms <= 0.0 || active >= self.duration_ms {
            self.state = TransitionState::Complete;
            return false;
        }

        self.state = TransitionState::Running;
        true
    }

    /// Get the current interpolated value.
    pub fn current_value(&self) -> AnimValue {
        if self.state == TransitionState::Complete {
            return self.to;
        }

        let active = self.elapsed_ms - self.delay_ms;
        if active <= 0.0 {
            return self.from;
        }

        if self.duration_ms <= 0.0 {
            return self.to;
        }

        let raw_t = (active / self.duration_ms).clamp(0.0, 1.0);
        let eased_t = easing::evaluate(&self.easing, raw_t);
        lerp_value(&self.from, &self.to, eased_t)
    }

    /// Whether the transition has completed.
    pub fn is_complete(&self) -> bool {
        self.state == TransitionState::Complete
    }

    /// Retarget the transition mid-flight to a new target value.
    ///
    /// The current interpolated value becomes the new `from`, the remaining
    /// duration is preserved, and the elapsed time resets to continue smoothly.
    pub fn retarget(&mut self, new_to: AnimValue) {
        let current = self.current_value();
        self.from = current;
        self.to = new_to;
        // Reset elapsed to start from current position with full duration.
        self.elapsed_ms = self.delay_ms;
        self.state = TransitionState::Running;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.02;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn simple_transition() -> Transition {
        Transition::new(
            "opacity".to_string(),
            AnimValue::Float(0.0),
            AnimValue::Float(1.0),
            200.0,
            EasingFunction::Linear,
        )
    }

    #[test]
    fn starts_pending() {
        let tr = simple_transition();
        assert_eq!(tr.state, TransitionState::Pending);
    }

    #[test]
    fn tick_runs() {
        let mut tr = simple_transition();
        assert!(tr.tick(50.0));
        assert_eq!(tr.state, TransitionState::Running);
    }

    #[test]
    fn tick_completes() {
        let mut tr = simple_transition();
        assert!(!tr.tick(300.0));
        assert_eq!(tr.state, TransitionState::Complete);
        assert!(tr.is_complete());
    }

    #[test]
    fn current_value_midpoint() {
        let mut tr = simple_transition();
        tr.tick(100.0);
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.5), "midpoint: {v}"),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn current_value_at_start() {
        let tr = simple_transition();
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn current_value_at_end() {
        let mut tr = simple_transition();
        tr.tick(300.0);
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 1.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn delay_handling() {
        let mut tr = simple_transition();
        tr.delay_ms = 100.0;
        tr.tick(50.0);
        assert_eq!(tr.state, TransitionState::Pending);
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.0), "during delay: {v}"),
            _ => panic!("expected Float"),
        }
        tr.tick(100.0); // 150ms total, 50ms active
        assert_eq!(tr.state, TransitionState::Running);
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.25), "after delay + 50ms: {v}"),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn retarget_mid_flight() {
        let mut tr = simple_transition();
        tr.tick(100.0); // at 50% → current = 0.5
        tr.retarget(AnimValue::Float(0.0));
        // After retarget, from=0.5, to=0.0, elapsed reset.
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.5), "right after retarget: {v}"),
            _ => panic!("expected Float"),
        }
        // Advance halfway through new transition.
        tr.tick(100.0);
        match tr.current_value() {
            AnimValue::Float(v) => assert!(approx(v, 0.25), "midpoint of retarget: {v}"),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn retarget_restarts_state() {
        let mut tr = simple_transition();
        tr.tick(300.0);
        assert!(tr.is_complete());
        tr.retarget(AnimValue::Float(0.5));
        assert_eq!(tr.state, TransitionState::Running);
        assert!(!tr.is_complete());
    }

    #[test]
    fn zero_duration_completes_immediately() {
        let mut tr = Transition::new(
            "opacity".to_string(),
            AnimValue::Float(0.0),
            AnimValue::Float(1.0),
            0.0,
            EasingFunction::Linear,
        );
        assert!(!tr.tick(1.0));
        assert!(tr.is_complete());
    }
}

//! Transition and animation types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
    Steps(u32, StepPosition),
}

impl Default for TimingFunction {
    fn default() -> Self {
        TimingFunction::Ease
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub property: String,
    pub duration_ms: f32,
    pub timing_function: TimingFunction,
    pub delay_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationDef {
    pub name: String,
    pub duration_ms: f32,
    pub timing_function: TimingFunction,
    pub delay_ms: f32,
    pub iteration_count: AnimationIterationCount,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub play_state: AnimationPlayState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnimationIterationCount {
    Finite(f32),
    Infinite,
}

impl Default for AnimationIterationCount {
    fn default() -> Self {
        AnimationIterationCount::Finite(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

impl Default for AnimationDirection {
    fn default() -> Self {
        AnimationDirection::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationFillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

impl Default for AnimationFillMode {
    fn default() -> Self {
        AnimationFillMode::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationPlayState {
    Running,
    Paused,
}

impl Default for AnimationPlayState {
    fn default() -> Self {
        AnimationPlayState::Running
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationComposition {
    Replace,
    Add,
    Accumulate,
}
impl Default for AnimationComposition {
    fn default() -> Self {
        AnimationComposition::Replace
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionBehavior {
    Normal,
    AllowDiscrete,
}
impl Default for TransitionBehavior {
    fn default() -> Self {
        TransitionBehavior::Normal
    }
}

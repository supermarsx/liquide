pub mod effects;
pub mod easing;
pub mod snap_preview;
pub mod workspace_transition;
pub mod snap;
pub mod preview;
pub mod shake;
pub mod resize;
pub mod minimize;

pub use effects::{WindowEffect, EffectState, EffectManager, Rect, EffectFrame};
pub use easing::EasingFunction;
pub use snap_preview::SnapPreview;
pub use workspace_transition::WorkspaceTransition;
pub use snap::{EdgeSnapper, SnapConfig, SnapEdge, SnapResult, Side};
pub use preview::{DragPreview, TilePreview, TileZone};
pub use shake::{ShakeDetector, ShakeAction, detect_shake_gesture};
pub use resize::{ResizeHandle, ResizeConstraints, LiveResize, constrain_resize, resize_cursor};
pub use minimize::{MinimizeAnimation, ease_in_out_quad};

#[cfg(test)]
mod tests;

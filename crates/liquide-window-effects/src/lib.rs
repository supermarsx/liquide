pub mod easing;
pub mod effects;
pub mod minimize;
pub mod preview;
pub mod resize;
pub mod shake;
pub mod snap;
pub mod snap_preview;
pub mod workspace_transition;

pub use easing::EasingFunction;
pub use effects::{EffectFrame, EffectManager, EffectState, Rect, WindowEffect};
pub use minimize::{MinimizeAnimation, ease_in_out_quad};
pub use preview::{DragPreview, TilePreview, TileZone};
pub use resize::{LiveResize, ResizeConstraints, ResizeHandle, constrain_resize, resize_cursor};
pub use shake::{ShakeAction, ShakeDetector, detect_shake_gesture};
pub use snap::{EdgeSnapper, Side, SnapConfig, SnapEdge, SnapResult};
pub use snap_preview::SnapPreview;
pub use workspace_transition::WorkspaceTransition;

/// Re-export of the canonical CSS easing module from `liquide-animation`.
/// Use this when CSS-standard easing curves are needed instead of window-effect-specific ones.
pub use liquide_animation::easing as css_easing;

#[cfg(test)]
mod tests;

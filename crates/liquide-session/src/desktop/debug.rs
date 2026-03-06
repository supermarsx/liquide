//! Debug helper functions for scene node inspection.

use liquide_compositor::scene::SceneNodeKind;

/// Short human-readable name for a scene node kind (for debug logging).
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub(super) fn scene_node_kind_name(kind: &SceneNodeKind) -> &'static str {
    match kind {
        SceneNodeKind::Root => "Root",
        SceneNodeKind::Background { .. } => "Background",
        SceneNodeKind::Surface { .. } => "Surface",
        SceneNodeKind::ChildSurface { .. } => "ChildSurface",
        SceneNodeKind::Glass(_) => "Glass",
        SceneNodeKind::Tint { .. } => "Tint",
        SceneNodeKind::Shadow { .. } => "Shadow",
        SceneNodeKind::Decoration { .. } => "Decoration",
        SceneNodeKind::BlurBackdrop => "BlurBackdrop",
        SceneNodeKind::BlurCache => "BlurCache",
        SceneNodeKind::Content => "Content",
        SceneNodeKind::Overlay => "Overlay",
        SceneNodeKind::ShellLayer => "ShellLayer",
        SceneNodeKind::Cursor { .. } => "Cursor",
        SceneNodeKind::Text { .. } => "Text",
        SceneNodeKind::Icon { .. } => "Icon",
        SceneNodeKind::LockScreen => "LockScreen",
        SceneNodeKind::CrashScreen => "CrashScreen",
        SceneNodeKind::Workspace { .. } => "Workspace",
        SceneNodeKind::RenderLayer { .. } => "RenderLayer",
        SceneNodeKind::ClipPath { .. } => "ClipPath",
        SceneNodeKind::Filter { .. } => "Filter",
        SceneNodeKind::Image { .. } => "Image",
        SceneNodeKind::GradientFill { .. } => "GradientFill",
        SceneNodeKind::SvgPath { .. } => "SvgPath",
        SceneNodeKind::BackdropFilter { .. } => "BackdropFilter",
        SceneNodeKind::BackgroundFill { .. } => "BackgroundFill",
        SceneNodeKind::Outline { .. } => "Outline",
        SceneNodeKind::BoxShadows { .. } => "BoxShadows",
        SceneNodeKind::Mask { .. } => "Mask",
        SceneNodeKind::Border { .. } => "Border",
        SceneNodeKind::BorderImage { .. } => "BorderImage",
        SceneNodeKind::TextCaret { .. } => "TextCaret",
        SceneNodeKind::SelectionOverlay { .. } => "SelectionOverlay",
    }
}

/// Extract color info from a scene node kind for debug logging.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub(super) fn scene_node_color_str(kind: &SceneNodeKind) -> String {
    match kind {
        SceneNodeKind::Background { color } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Glass(params) => {
            let c = &params.tint_color;
            format!(
                "tint({},{},{},{}) blur={}",
                c.r, c.g, c.b, c.a, params.blur_radius
            )
        }
        SceneNodeKind::Tint { color } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Shadow { color, .. } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Decoration { background, .. } => {
            format!(
                "bg({},{},{},{})",
                background.r, background.g, background.b, background.a
            )
        }
        _ => "-".to_string(),
    }
}

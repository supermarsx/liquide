//! Scene graph node types for the compositor.
//!
//! The scene graph is a hierarchical tree of nodes, each representing a visual
//! element on the desktop. The compositor walks the tree, flattens it into a
//! z-sorted list of visible leaf nodes, and hands that list to the renderer.

use std::sync::Arc;

use crate::geometry::{Affine2D, Rect};
use crate::pixel::{Color, PixelFormat};
use serde::{Deserialize, Serialize};

/// Unique identifier for a scene graph node.
pub type NodeId = u64;

/// Properties carried by every scene graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProperties {
    /// Bounding rectangle in parent-relative coordinates.
    pub bounds: Rect,
    /// Opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Local transform applied before rendering.
    pub transform: Affine2D,
    /// Optional clip rectangle (in parent coordinates).
    pub clip: Option<Rect>,
    /// Whether the node is visible.
    pub visible: bool,
    /// Z-order within the parent (higher = on top).
    pub z_order: u32,
}

impl NodeProperties {
    /// Create default properties for the given bounds.
    #[must_use]
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            opacity: 1.0,
            transform: Affine2D::identity(),
            clip: None,
            visible: true,
            z_order: 0,
        }
    }

    /// Set the opacity.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Set the local transform.
    #[must_use]
    pub fn with_transform(mut self, transform: Affine2D) -> Self {
        self.transform = transform;
        self
    }

    /// Set the clip rectangle.
    #[must_use]
    pub fn with_clip(mut self, clip: Rect) -> Self {
        self.clip = Some(clip);
        self
    }

    /// Set the z-order.
    #[must_use]
    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z_order = z;
        self
    }

    /// Set visibility.
    #[must_use]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// Glass surface parameters for the Liquid Glass effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassParams {
    /// Blur radius in pixels for the backdrop.
    pub blur_radius: u32,
    /// Tint color applied over the blurred backdrop.
    pub tint_color: Color,
    /// Whether to draw an inner glow border.
    pub inner_glow: bool,
    /// Whether parallax is enabled (background shifts slightly on scroll).
    pub parallax: bool,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            tint_color: Color::new(255, 255, 255, 40),
            inner_glow: true,
            parallax: false,
        }
    }
}

/// Kind of clip path for `SceneNodeKind::ClipPath`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipPathKind {
    /// Circular clip.
    Circle {
        center_x: f32,
        center_y: f32,
        radius: f32,
    },
    /// Rounded rectangle clip.
    RoundedRect { corner_radius: f32 },
    /// Ellipse clip.
    Ellipse {
        center_x: f32,
        center_y: f32,
        rx: f32,
        ry: f32,
    },
    /// Polygon clip (list of vertices).
    Polygon { points: Vec<(f32, f32)> },
}

/// Post-processing filter specification for `SceneNodeKind::Filter`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterSpec {
    /// Gaussian blur.
    Blur { radius: f32 },
    /// Brightness adjustment (1.0 = normal).
    Brightness(f32),
    /// Contrast adjustment (1.0 = normal).
    Contrast(f32),
    /// Saturation adjustment (0.0 = grayscale, 1.0 = normal).
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Grayscale conversion (0.0 = none, 1.0 = full).
    Grayscale(f32),
    /// Sepia tone (0.0 = none, 1.0 = full).
    Sepia(f32),
    /// Color inversion (0.0 = none, 1.0 = full).
    Invert(f32),
    /// Drop shadow.
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        color: Color,
    },
    /// Opacity (multiplies existing alpha).
    Opacity(f32),
    /// Custom SVG filter reference.
    Url(String),
}

/// Backdrop filter specification (applied to the area behind an element).
///
/// Mirrors CSS `backdrop-filter` — each variant maps to one CSS filter function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackdropFilterSpec {
    Blur { radius: f32 },
    Brightness(f32),
    Contrast(f32),
    Saturate(f32),
    HueRotate(f32),
    Grayscale(f32),
    Sepia(f32),
    Invert(f32),
    Opacity(f32),
}

/// Text decoration specification (CSS text-decoration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDecoration {
    pub line: TextDecorationLine,
    pub style: TextDecorationStyle,
    pub color: Option<Color>,
    pub thickness: f32,
}

/// Which line(s) to render for text-decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationLine {
    None,
    Underline,
    Overline,
    LineThrough,
    /// Underline + Overline
    UnderlineOverline,
}

/// Visual style of the text decoration line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationStyle {
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Text shadow specification (CSS text-shadow — multiple allowed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: Color,
}

/// Box shadow specification with inset support (CSS box-shadow — multiple allowed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxShadowSpec {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
    pub inset: bool,
}

/// Outline specification (CSS outline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineSpec {
    pub width: f32,
    pub style: OutlineStyle,
    pub color: Color,
    pub offset: f32,
}

/// Outline line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// CSS overflow behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

/// CSS mask specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaskSpec {
    /// Mask using an image (URL or image data).
    Image { image_id: u64, mode: MaskMode },
    /// Mask using a gradient (luminance or alpha).
    Gradient {
        gradient: GradientSpec,
        mode: MaskMode,
    },
}

/// How the mask source is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskMode {
    /// Use the luminance of the mask.
    Luminance,
    /// Use the alpha channel of the mask.
    Alpha,
    /// Match the mask source type.
    MatchSource,
}

/// CSS background specification (for background-image + related properties).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundSpec {
    pub color: Option<Color>,
    pub image: Option<BackgroundImage>,
    pub size: BackgroundSize,
    pub position: (f32, f32),
    pub repeat: BackgroundRepeat,
}

/// Background image source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundImage {
    /// URL to image resource.
    Url(String),
    /// Image data ID.
    ImageId(u64),
    /// Gradient fill.
    Gradient(GradientSpec),
}

/// CSS background-size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackgroundSize {
    Auto,
    Cover,
    Contain,
    Explicit { width: f32, height: f32 },
}

/// CSS background-repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundRepeat {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
    Space,
    Round,
}

/// CSS border-image specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderImageSpec {
    pub source: BackgroundImage,
    pub slice: (f32, f32, f32, f32),
    pub width: (f32, f32, f32, f32),
    pub outset: (f32, f32, f32, f32),
    pub repeat: BorderImageRepeat,
}

/// Repeat mode for border-image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderImageRepeat {
    Stretch,
    Repeat,
    Round,
    Space,
}

/// Per-side border specification for CSS box model borders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderSides {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

/// Single border side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderSideStyle,
    pub color: Color,
}

/// Border side line style (CSS border-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderSideStyle {
    None,
    Hidden,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderSideStyle::None,
            color: Color::new(0, 0, 0, 0),
        }
    }
}

impl Default for BorderSides {
    fn default() -> Self {
        Self {
            top: BorderSide::default(),
            right: BorderSide::default(),
            bottom: BorderSide::default(),
            left: BorderSide::default(),
        }
    }
}

impl Default for BackgroundRepeat {
    fn default() -> Self {
        Self::Repeat
    }
}

impl Default for BackgroundSize {
    fn default() -> Self {
        Self::Auto
    }
}

impl Default for Overflow {
    fn default() -> Self {
        Self::Visible
    }
}

impl Default for TextDecorationLine {
    fn default() -> Self {
        Self::None
    }
}

impl Default for TextDecorationStyle {
    fn default() -> Self {
        Self::Solid
    }
}

/// Image fit mode for `SceneNodeKind::Image`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFit {
    /// Scale to fill bounds, preserving aspect ratio (may crop).
    Cover,
    /// Scale to fit within bounds, preserving aspect ratio (may letterbox).
    Contain,
    /// Stretch to exactly fill bounds (may distort).
    Fill,
    /// No scaling — display at natural size.
    None,
}

/// Gradient specification for `SceneNodeKind::GradientFill`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GradientSpec {
    /// Linear gradient from start to end point (normalized 0..1).
    Linear {
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        stops: Vec<(f32, Color)>,
    },
    /// Radial gradient from center outward.
    Radial {
        center_x: f32,
        center_y: f32,
        radius: f32,
        stops: Vec<(f32, Color)>,
    },
    /// Conic (sweep) gradient around a center point.
    Conic {
        center_x: f32,
        center_y: f32,
        start_angle: f32,
        stops: Vec<(f32, Color)>,
    },
    /// Mesh gradient using a grid of color patches.
    Mesh {
        rows: u32,
        cols: u32,
        colors: Vec<Color>,
    },
}

/// Window decoration button visibility state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationButtons {
    /// Whether the close button is visible.
    pub close: bool,
    /// Whether the maximize button is visible.
    pub maximize: bool,
    /// Whether the minimize button is visible.
    pub minimize: bool,
    /// Whether the always-on-top (pin) button is visible.
    pub always_on_top: bool,
    /// Whether the window is currently pinned as always-on-top.
    pub is_topmost: bool,
    /// Whether the close button is currently hovered.
    pub close_hovered: bool,
    /// Whether the maximize button is currently hovered.
    pub maximize_hovered: bool,
    /// Whether the minimize button is currently hovered.
    pub minimize_hovered: bool,
    /// Whether the always-on-top button is currently hovered.
    pub always_on_top_hovered: bool,
}

/// Colors for window decoration buttons, resolved from CSS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationColors {
    /// Close button background.
    pub close_bg: Color,
    /// Close button background when hovered.
    pub close_bg_hover: Color,
    /// Close button icon color.
    pub close_icon: Color,
    /// Maximize button background.
    pub maximize_bg: Color,
    /// Maximize button background when hovered.
    pub maximize_bg_hover: Color,
    /// Maximize button icon color.
    pub maximize_icon: Color,
    /// Minimize button background.
    pub minimize_bg: Color,
    /// Minimize button background when hovered.
    pub minimize_bg_hover: Color,
    /// Minimize button icon color.
    pub minimize_icon: Color,
    /// Always-on-top button background (inactive).
    pub pin_bg: Color,
    /// Always-on-top button background when hovered (inactive).
    pub pin_bg_hover: Color,
    /// Always-on-top button background (active / topmost).
    pub pin_bg_active: Color,
    /// Always-on-top button background when hovered (active).
    pub pin_bg_active_hover: Color,
    /// Pin icon color (inactive).
    pub pin_icon: Color,
    /// Pin icon color (active / topmost).
    pub pin_icon_active: Color,
}

/// Layout dimensions for window decoration buttons, resolved from CSS.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecorationLayout {
    /// Title bar height in pixels.
    pub title_bar_height: f32,
    /// Button width in pixels (click target).
    pub button_width: f32,
    /// Button height in pixels (click target).
    pub button_height: f32,
    /// Right margin before first button (px).
    pub button_right_margin: f32,
    /// Corner radius on button backgrounds (px).
    pub button_corner_radius: f32,
}

impl Default for DecorationLayout {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            button_width: 32.0,
            button_height: 22.0,
            button_right_margin: 4.0,
            button_corner_radius: 3.0,
        }
    }
}

impl Default for DecorationColors {
    fn default() -> Self {
        Self {
            close_bg: Color::new(232, 17, 35, 220),
            close_bg_hover: Color::new(241, 60, 70, 255),
            close_icon: Color::new(255, 255, 255, 240),
            maximize_bg: Color::new(255, 255, 255, 20),
            maximize_bg_hover: Color::new(255, 255, 255, 60),
            maximize_icon: Color::new(220, 220, 220, 240),
            minimize_bg: Color::new(255, 255, 255, 20),
            minimize_bg_hover: Color::new(255, 255, 255, 60),
            minimize_icon: Color::new(220, 220, 220, 240),
            pin_bg: Color::new(255, 255, 255, 20),
            pin_bg_hover: Color::new(255, 255, 255, 60),
            pin_bg_active: Color::new(60, 130, 220, 180),
            pin_bg_active_hover: Color::new(80, 150, 240, 220),
            pin_icon: Color::new(220, 220, 220, 240),
            pin_icon_active: Color::new(255, 255, 255, 255),
        }
    }
}

impl Default for DecorationButtons {
    fn default() -> Self {
        Self {
            close: true,
            maximize: true,
            minimize: true,
            always_on_top: true,
            is_topmost: false,
            close_hovered: false,
            maximize_hovered: false,
            minimize_hovered: false,
            always_on_top_hovered: false,
        }
    }
}

/// A reference to pixel data from a Wayland client surface.
#[derive(Debug, Clone)]
pub struct SurfaceBuffer {
    /// Raw pixel data (shared via `Arc` to avoid cloning megabytes during
    /// scene flattening — cloning an `Arc` is just an atomic increment).
    pub pixels: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    /// Bytes per row (may include padding).
    pub stride: u32,
    pub format: PixelFormat,
}

// Re-export cursor types from liquide-cursor crate
pub use liquide_cursor::{CursorShape as NewCursorShape, ResizeDirection};

/// Legacy cursor shape enum for backward compatibility.
///
/// **Deprecated**: Use `liquide_cursor::CursorShape` directly.
/// This enum provides compatibility with existing code but will be removed in a future version.
#[deprecated(since = "0.1.0", note = "use liquide_cursor::CursorShape instead")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegacyCursorShape {
    Arrow,
    Move,
    ResizeNS,
    ResizeEW,
    ResizeNWSE,
    ResizeNESW,
    Pointer,
    Text,
    NotAllowed,
    Wait,
    Progress,
    Help,
    Crosshair,
    Grab,
    Grabbing,
    ZoomIn,
    ZoomOut,
    ContextMenu,
    Alias,
    Copy,
    NoDrop,
    Cell,
    VerticalText,
    AllScroll,
    ExpandH,
    ExpandV,
}

/// Current cursor shape type alias.
/// Points to the new unified cursor type from liquide-cursor crate.
pub type CursorShape = NewCursorShape;

/// Convert legacy cursor shape to new format.
impl From<LegacyCursorShape> for NewCursorShape {
    fn from(legacy: LegacyCursorShape) -> Self {
        match legacy {
            LegacyCursorShape::Arrow => NewCursorShape::Arrow,
            LegacyCursorShape::Move => NewCursorShape::Move,
            LegacyCursorShape::ResizeNS => NewCursorShape::Resize(ResizeDirection::North),
            LegacyCursorShape::ResizeEW => NewCursorShape::Resize(ResizeDirection::East),
            LegacyCursorShape::ResizeNWSE => NewCursorShape::Resize(ResizeDirection::NorthWest),
            LegacyCursorShape::ResizeNESW => NewCursorShape::Resize(ResizeDirection::NorthEast),
            LegacyCursorShape::Pointer => NewCursorShape::Pointer,
            LegacyCursorShape::Text => NewCursorShape::Text,
            LegacyCursorShape::NotAllowed => NewCursorShape::NotAllowed,
            LegacyCursorShape::Wait => NewCursorShape::Wait,
            LegacyCursorShape::Progress => NewCursorShape::Progress,
            LegacyCursorShape::Help => NewCursorShape::Help,
            LegacyCursorShape::Crosshair => NewCursorShape::Crosshair,
            LegacyCursorShape::Grab => NewCursorShape::Grab,
            LegacyCursorShape::Grabbing => NewCursorShape::Grabbing,
            LegacyCursorShape::ZoomIn => NewCursorShape::ZoomIn,
            LegacyCursorShape::ZoomOut => NewCursorShape::ZoomOut,
            LegacyCursorShape::ContextMenu => NewCursorShape::ContextMenu,
            LegacyCursorShape::Alias => NewCursorShape::Alias,
            LegacyCursorShape::Copy => NewCursorShape::Copy,
            LegacyCursorShape::NoDrop => NewCursorShape::NoDrop,
            LegacyCursorShape::Cell => NewCursorShape::Cell,
            LegacyCursorShape::VerticalText => NewCursorShape::VerticalText,
            LegacyCursorShape::AllScroll => NewCursorShape::AllScroll,
            LegacyCursorShape::ExpandH => NewCursorShape::ColResize,
            LegacyCursorShape::ExpandV => NewCursorShape::RowResize,
        }
    }
}

impl Default for LegacyCursorShape {
    fn default() -> Self {
        Self::Arrow
    }
}

/// The type-specific payload of a scene graph node.
#[derive(Debug, Clone)]
pub enum SceneNodeKind {
    /// Root of the scene tree.
    Root,
    /// Desktop wallpaper / solid background.
    Background { color: Color },
    /// Pre-blurred wallpaper cache.
    BlurCache,
    /// A workspace container (only the active workspace is visible).
    Workspace { index: u32 },
    /// A toplevel Wayland client surface.
    Surface {
        surface_id: u64,
        buffer: Option<SurfaceBuffer>,
    },
    /// Drop shadow behind a surface.
    Shadow {
        spread: f32,
        blur_radius: f32,
        color: Color,
    },
    /// Server-side window decoration (title bar, borders).
    Decoration {
        title: Option<String>,
        title_color: Color,
        background: Color,
        border_color: Color,
        border_width: f32,
        corner_radius: f32,
        button_state: DecorationButtons,
        button_colors: DecorationColors,
        button_layout: DecorationLayout,
    },
    /// Child surface (subsurface, popup).
    ChildSurface {
        surface_id: u64,
        buffer: Option<SurfaceBuffer>,
    },
    /// Transient overlay (tooltip, menu, drag-and-drop feedback).
    Overlay,
    /// Glass panel (dock, status bar, notification).
    Glass(GlassParams),
    /// Blurred backdrop region behind glass.
    BlurBackdrop,
    /// Color tint overlay for glass.
    Tint { color: Color },
    /// Content rendered on a glass surface (text, icons, widgets).
    Content,
    /// Shell layer (layer-shell surfaces).
    ShellLayer,
    /// Software cursor with context-sensitive shape.
    Cursor { shape: CursorShape },
    /// Text label rendered with the font system.
    Text {
        text: String,
        color: Color,
        /// Legacy scale factor (1 = 16px base). Used when font_family is empty.
        scale: u32,
        /// Font family name (e.g. "Manrope", "Inter"). Empty = bitmap fallback.
        font_family: String,
        /// Font size in logical pixels (e.g. 14.0). 0 = use scale-based sizing.
        font_size: f32,
        /// Font weight (100–900, 400 = Regular, 700 = Bold).
        font_weight: u16,
        /// Whether the text is italic.
        font_style_italic: bool,
        /// Letter-spacing adjustment in pixels.
        letter_spacing: f32,
        /// Word-spacing adjustment in pixels.
        word_spacing: f32,
        /// Line-height in pixels.
        line_height: f32,
        /// Text alignment: 0=start/left, 1=center, 2=right/end, 3=justify.
        text_align: u8,
        /// Text transform: 0=none, 1=capitalize, 2=uppercase, 3=lowercase.
        text_transform: u8,
        /// Text overflow: 0=clip, 1=ellipsis.
        text_overflow: u8,
        /// White-space handling: 0=normal, 1=nowrap, 2=pre, 3=pre-wrap, 4=pre-line, 5=break-spaces.
        white_space: u8,
        /// Text indent in pixels (first line).
        text_indent: f32,
        /// Optional text decoration (underline/strikethrough etc.).
        text_decoration: Option<TextDecoration>,
        /// Optional text shadows.
        text_shadows: Vec<TextShadow>,
    },
    /// Built-in vector icon rendered at the node bounds.
    Icon { icon_id: u32, color: Color },
    /// Isolated render layer with custom blend mode (for compositing groups).
    RenderLayer {
        blend_mode: crate::pixel::BlendMode,
        isolate: bool,
    },
    /// Arbitrary clip path (circle, rounded rect, or polygon).
    ClipPath { clip_kind: ClipPathKind },
    /// Post-processing filter chain applied to children.
    Filter { filters: Vec<FilterSpec> },
    /// Backdrop filter chain (blur/brightness/etc. behind element).
    BackdropFilter { filters: Vec<BackdropFilterSpec> },
    /// Decoded image content (PNG, BMP, etc.).
    Image {
        image_id: u64,
        width: u32,
        height: u32,
        fit: ImageFit,
    },
    /// Gradient fill across the node bounds.
    GradientFill { gradient: GradientSpec },
    /// Full background specification (color + image + gradients).
    BackgroundFill { background: BackgroundSpec },
    /// Outline (rendered outside the border box).
    Outline { outline: OutlineSpec },
    /// Multiple box shadows (CSS box-shadow, supports inset).
    BoxShadows { shadows: Vec<BoxShadowSpec> },
    /// Mask applied to children (CSS mask / mask-image).
    Mask { mask: MaskSpec },
    /// Border with per-side styling.
    Border {
        sides: BorderSides,
        radius: (f32, f32, f32, f32), // top-left, top-right, bottom-right, bottom-left
    },
    /// Border image (CSS border-image).
    BorderImage { spec: BorderImageSpec },
    /// Lock screen overlay.
    LockScreen,
    /// Emergency crash overlay.
    CrashScreen,
}

/// A node in the compositor's scene graph.
#[derive(Debug, Clone)]
pub struct SceneNode {
    /// Unique node identifier.
    pub id: NodeId,
    /// The type-specific payload.
    pub kind: SceneNodeKind,
    /// Common visual properties.
    pub properties: NodeProperties,
    /// Child nodes (rendered in z-order).
    pub children: Vec<SceneNode>,
}

impl SceneNode {
    /// Create a new scene node with no children.
    #[must_use]
    pub fn new(id: NodeId, kind: SceneNodeKind, properties: NodeProperties) -> Self {
        Self {
            id,
            kind,
            properties,
            children: Vec::new(),
        }
    }

    /// Append a child node.
    pub fn add_child(&mut self, child: SceneNode) {
        self.children.push(child);
    }

    /// Walk the tree depth-first in z-order, calling the visitor on each node
    /// with the accumulated absolute transform.
    pub fn walk<F: FnMut(&SceneNode, &Affine2D)>(&self, visitor: &mut F) {
        self.walk_inner(&Affine2D::identity(), visitor);
    }

    fn walk_inner<F: FnMut(&SceneNode, &Affine2D)>(
        &self,
        parent_transform: &Affine2D,
        visitor: &mut F,
    ) {
        if !self.properties.visible {
            return;
        }

        // Compose: translation from bounds origin + local transform
        let local = Affine2D::translation(self.properties.bounds.x, self.properties.bounds.y)
            .then(&self.properties.transform);
        let absolute = local.then(parent_transform);

        visitor(self, &absolute);

        // Sort children by z-order before walking.
        // Use a stack-allocated array for small child counts to avoid
        // heap allocation on every node traversal.
        let n = self.children.len();
        if n <= 1 {
            // 0 or 1 children — no sorting needed
            for child in &self.children {
                child.walk_inner(&absolute, visitor);
            }
        } else if n <= 16 {
            // Small child count — use stack array
            let mut indices = [0u16; 16];
            for i in 0..n {
                indices[i] = i as u16;
            }
            indices[..n].sort_by_key(|&i| self.children[i as usize].properties.z_order);
            for &i in &indices[..n] {
                self.children[i as usize].walk_inner(&absolute, visitor);
            }
        } else {
            // Fallback to heap-allocated sort for large child counts
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by_key(|&i| self.children[i].properties.z_order);
            for &i in &sorted_indices {
                self.children[i].walk_inner(&absolute, visitor);
            }
        }
    }

    /// Find a node by ID using depth-first search.
    #[must_use]
    pub fn find(&self, id: NodeId) -> Option<&SceneNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by ID (mutable) using depth-first search.
    pub fn find_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// Remove a direct or nested child by ID, returning it if found.
    pub fn remove_child(&mut self, id: NodeId) -> Option<SceneNode> {
        // Check direct children first
        if let Some(pos) = self.children.iter().position(|c| c.id == id) {
            return Some(self.children.remove(pos));
        }
        // Recurse into children
        for child in &mut self.children {
            if let Some(removed) = child.remove_child(id) {
                return Some(removed);
            }
        }
        None
    }

    /// Replace a node by ID with a new node, returning the old node if found.
    pub fn replace_child(&mut self, id: NodeId, new: SceneNode) -> Option<SceneNode> {
        // Check direct children
        for child in &mut self.children {
            if child.id == id {
                let old = std::mem::replace(child, new);
                return Some(old);
            }
        }
        // Recurse
        for child in &mut self.children {
            if let Some(old) = child.replace_child(id, new.clone()) {
                return Some(old);
            }
        }
        None
    }

    /// Move a child node to new bounds.
    pub fn move_child(&mut self, id: NodeId, new_bounds: Rect) {
        if let Some(node) = self.find_mut(id) {
            node.properties.bounds = new_bounds;
        }
    }

    /// Set the opacity of a node by ID.
    pub fn set_opacity(&mut self, id: NodeId, opacity: f32) {
        if let Some(node) = self.find_mut(id) {
            node.properties.opacity = opacity;
        }
    }

    /// List all descendant node IDs (depth-first order, excludes self).
    #[must_use]
    pub fn descendants(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        for child in &self.children {
            result.push(child.id);
            result.extend(child.descendants());
        }
        result
    }

    /// Compute the depth of the subtree (0 for a leaf, 1+ for internal nodes).
    #[must_use]
    pub fn depth(&self) -> u32 {
        if self.children.is_empty() {
            return 0;
        }
        self.children
            .iter()
            .map(|c| c.depth() + 1)
            .max()
            .unwrap_or(0)
    }

    /// Total number of descendants (recursive child count, excludes self).
    #[must_use]
    pub fn child_count(&self) -> usize {
        let mut count = self.children.len();
        for child in &self.children {
            count += child.child_count();
        }
        count
    }

    /// Walk the tree depth-first in z-order with mutable access,
    /// calling the visitor on each visible node.
    pub fn walk_mut<F: FnMut(&mut SceneNode)>(&mut self, visitor: &mut F) {
        if !self.properties.visible {
            return;
        }
        visitor(self);
        // Sort children indices by z-order before walking.
        let n = self.children.len();
        if n <= 1 {
            for child in &mut self.children {
                child.walk_mut(visitor);
            }
        } else if n <= 16 {
            let mut indices = [0u16; 16];
            for i in 0..n {
                indices[i] = i as u16;
            }
            indices[..n].sort_by_key(|&i| self.children[i as usize].properties.z_order);
            for &i in &indices[..n] {
                self.children[i as usize].walk_mut(visitor);
            }
        } else {
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by_key(|&i| self.children[i].properties.z_order);
            for &i in &sorted_indices {
                self.children[i].walk_mut(visitor);
            }
        }
    }

    /// Flatten the tree into a z-sorted list of visible leaf nodes with
    /// computed absolute bounds and transforms.
    #[must_use]
    pub fn flatten(&self) -> Vec<FlatNode> {
        let mut result = Vec::new();
        self.walk(&mut |node, abs_transform| {
            // Skip non-visual structural nodes (Root, Workspace containers)
            let is_visual = !matches!(
                node.kind,
                SceneNodeKind::Root | SceneNodeKind::Workspace { .. }
            );

            if is_visual {
                let abs_bounds = abs_transform.transform_rect(Rect::new(
                    0.0,
                    0.0,
                    node.properties.bounds.width,
                    node.properties.bounds.height,
                ));

                result.push(FlatNode {
                    id: node.id,
                    kind: node.kind.clone(),
                    absolute_bounds: abs_bounds,
                    absolute_transform: *abs_transform,
                    clip: node.properties.clip,
                    opacity: node.properties.opacity,
                    z_order: node.properties.z_order,
                });
            }
        });
        result
    }
}

/// A flattened scene node after tree walking, ready for rendering.
#[derive(Debug, Clone)]
pub struct FlatNode {
    /// The node's unique identifier.
    pub id: NodeId,
    /// The type-specific payload.
    pub kind: SceneNodeKind,
    /// Bounding rectangle in absolute (screen) coordinates.
    pub absolute_bounds: Rect,
    /// Accumulated absolute transform.
    pub absolute_transform: Affine2D,
    /// Clip rectangle in absolute coordinates (if any).
    pub clip: Option<Rect>,
    /// Effective opacity (not yet multiplied with parent).
    pub opacity: f32,
    /// Z-order within parent.
    pub z_order: u32,
}

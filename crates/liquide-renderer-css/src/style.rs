//! Core render style data structures.
//!
//! These structures represent fully-resolved styles that can be directly
//! consumed by the renderer, eliminating the need for CSS queries during
//! the render loop.

use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{
    BackdropFilterSpec, BackgroundSpec,
    BorderImageSpec, BorderSides, BoxShadowSpec, MaskSpec, OutlineSpec, Overflow,
    TextDecoration, TextShadow,
};
use serde::{Deserialize, Serialize};

use crate::glass::GlassStyle;
use crate::shadow::ShadowStyle;
use crate::transform::TransformStyle;

/// Comprehensive styling for a rendered element.
///
/// This structure contains all visual properties that can be derived from
/// CSS, organized for efficient renderer consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderStyle {
    // ── Colors ──────────────────────────────────────────────────────────
    pub background_color: Option<Color>,
    pub foreground_color: Option<Color>,
    pub border_color: Option<Color>,

    // ── Dimensions ─────────────────────────────────────────────────────
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub padding: Padding,
    pub margin: Margin,

    // ── Border ──────────────────────────────────────────────────────────
    pub border: BorderStyle,
    pub border_radius: f32,
    /// Per-corner radii: (top-left, top-right, bottom-right, bottom-left).
    /// When set, overrides `border_radius`.
    pub border_radii: Option<(f32, f32, f32, f32)>,
    /// Per-side border (when sides differ).
    pub border_sides: Option<BorderSides>,
    /// CSS border-image.
    pub border_image: Option<BorderImageSpec>,

    // ── Effects ─────────────────────────────────────────────────────────
    pub opacity: f32,
    pub glass: Option<GlassStyle>,
    pub shadow: Option<ShadowStyle>,
    /// Multiple box shadows (CSS box-shadow).
    pub box_shadows: Vec<BoxShadowSpec>,
    pub transform: TransformStyle,
    /// CSS mix-blend-mode.
    pub mix_blend_mode: BlendMode,
    /// CSS isolation (creates a new stacking context when true).
    pub isolation: bool,

    // ── Text ────────────────────────────────────────────────────────────
    pub text_color: Option<Color>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub font_style: FontStyle,
    pub letter_spacing: Option<f32>,
    pub line_height: Option<f32>,
    pub text_align: TextAlign,
    pub text_decoration: Option<TextDecoration>,
    pub text_shadow: Vec<TextShadow>,
    pub text_overflow: TextOverflow,
    pub text_transform: TextTransform,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,

    // ── Layout ──────────────────────────────────────────────────────────
    pub z_index: i32,
    pub visibility: bool,
    pub display: Display,
    pub position: Position,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,

    // ── Flexbox ─────────────────────────────────────────────────────────
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Option<f32>,
    pub gap: Option<f32>,

    // ── Grid ────────────────────────────────────────────────────────────
    pub grid_template_columns: Option<String>,
    pub grid_template_rows: Option<String>,
    pub grid_column: Option<String>,
    pub grid_row: Option<String>,

    // ── Background ──────────────────────────────────────────────────────
    /// Full background specification (image + gradient + position + size + repeat).
    pub background: Option<BackgroundSpec>,

    // ── Advanced filters & effects ──────────────────────────────────────
    pub blur_radius: Option<u32>,
    pub backdrop_filter: Option<BackdropFilterOld>,
    /// Full CSS backdrop-filter chain.
    pub backdrop_filters: Vec<BackdropFilterSpec>,
    /// CSS filter chain applied to the element.
    pub filter: Vec<liquide_compositor::scene::FilterSpec>,
    /// CSS mask.
    pub mask: Option<MaskSpec>,
    /// CSS outline.
    pub outline: Option<OutlineSpec>,

    // ── Cursor & pointer ────────────────────────────────────────────────
    pub cursor: Option<String>,
    pub pointer_events: PointerEvents,

    // ── Transition & animation ──────────────────────────────────────────
    pub transition: Option<TransitionSpec>,
    pub animation: Option<AnimationSpec>,
}

/// Transition specification (CSS transition shorthand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSpec {
    pub property: String,
    pub duration_ms: f32,
    pub timing_function: TimingFunction,
    pub delay_ms: f32,
}

/// CSS animation specification (CSS animation shorthand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationSpec {
    pub name: String,
    pub duration_ms: f32,
    pub timing_function: TimingFunction,
    pub delay_ms: f32,
    pub iteration_count: AnimationIterationCount,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub play_state: AnimationPlayState,
}

/// CSS animation-timing-function / transition-timing-function.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
    Steps(u32, StepPosition),
}

/// Step position for steps() timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

/// Animation iteration count.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnimationIterationCount {
    Finite(f32),
    Infinite,
}

/// CSS animation-direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// CSS animation-fill-mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationFillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

/// CSS animation-play-state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationPlayState {
    Running,
    Paused,
}

/// CSS font-style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

/// CSS text-align.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

/// CSS text-overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

/// CSS text-transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextTransform {
    None,
    Capitalize,
    Uppercase,
    Lowercase,
}

/// CSS white-space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

/// CSS word-break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordBreak {
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

/// CSS display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    None,
    Contents,
}

/// CSS position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// CSS flex-direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// CSS flex-wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

/// CSS justify-content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS align-items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

/// CSS align-self.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignSelf {
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

/// CSS pointer-events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerEvents {
    Auto,
    None,
}

/// Border styling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BorderStyle {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

/// Border line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderLineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

/// Padding dimensions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Margin dimensions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Legacy backdrop filter enum (kept for backward compatibility).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BackdropFilterOld {
    Blur { radius: u32 },
    Brightness { amount: f32 },
    Contrast { amount: f32 },
    Saturate { amount: f32 },
}

// ============================================================================
// Default implementations
// ============================================================================

impl Default for FontStyle {
    fn default() -> Self { Self::Normal }
}

impl Default for TextAlign {
    fn default() -> Self { Self::Start }
}

impl Default for TextOverflow {
    fn default() -> Self { Self::Clip }
}

impl Default for TextTransform {
    fn default() -> Self { Self::None }
}

impl Default for WhiteSpace {
    fn default() -> Self { Self::Normal }
}

impl Default for WordBreak {
    fn default() -> Self { Self::Normal }
}

impl Default for Display {
    fn default() -> Self { Self::Block }
}

impl Default for Position {
    fn default() -> Self { Self::Static }
}

impl Default for FlexDirection {
    fn default() -> Self { Self::Row }
}

impl Default for FlexWrap {
    fn default() -> Self { Self::NoWrap }
}

impl Default for JustifyContent {
    fn default() -> Self { Self::FlexStart }
}

impl Default for AlignItems {
    fn default() -> Self { Self::Stretch }
}

impl Default for AlignSelf {
    fn default() -> Self { Self::Auto }
}

impl Default for PointerEvents {
    fn default() -> Self { Self::Auto }
}

impl Default for TimingFunction {
    fn default() -> Self { Self::Ease }
}

impl Default for AnimationDirection {
    fn default() -> Self { Self::Normal }
}

impl Default for AnimationFillMode {
    fn default() -> Self { Self::None }
}

impl Default for AnimationPlayState {
    fn default() -> Self { Self::Running }
}

impl Default for StepPosition {
    fn default() -> Self { Self::JumpEnd }
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            foreground_color: None,
            border_color: None,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            padding: Padding::default(),
            margin: Margin::default(),
            border: BorderStyle::default(),
            border_radius: 0.0,
            border_radii: None,
            border_sides: None,
            border_image: None,
            opacity: 1.0,
            glass: None,
            shadow: None,
            box_shadows: Vec::new(),
            transform: TransformStyle::default(),
            mix_blend_mode: BlendMode::SrcOver,
            isolation: false,
            text_color: None,
            font_family: None,
            font_size: None,
            font_weight: None,
            font_style: FontStyle::default(),
            letter_spacing: None,
            line_height: None,
            text_align: TextAlign::default(),
            text_decoration: None,
            text_shadow: Vec::new(),
            text_overflow: TextOverflow::default(),
            text_transform: TextTransform::default(),
            white_space: WhiteSpace::default(),
            word_break: WordBreak::default(),
            z_index: 0,
            visibility: true,
            display: Display::default(),
            position: Position::default(),
            overflow_x: Overflow::default(),
            overflow_y: Overflow::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_self: AlignSelf::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: None,
            gap: None,
            grid_template_columns: None,
            grid_template_rows: None,
            grid_column: None,
            grid_row: None,
            background: None,
            blur_radius: None,
            backdrop_filter: None,
            backdrop_filters: Vec::new(),
            filter: Vec::new(),
            mask: None,
            outline: None,
            cursor: None,
            pointer_events: PointerEvents::default(),
            transition: None,
            animation: None,
        }
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color::new(0, 0, 0, 0),
        }
    }
}

impl RenderStyle {
    /// Create a new default render style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set background color.
    pub fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Set foreground/text color.
    pub fn with_foreground(mut self, color: Color) -> Self {
        self.foreground_color = Some(color);
        self
    }

    /// Set opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set border.
    pub fn with_border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set border radius.
    pub fn with_border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set glass effect.
    pub fn with_glass(mut self, glass: GlassStyle) -> Self {
        self.glass = Some(glass);
        self
    }

    /// Set box shadow.
    pub fn with_shadow(mut self, shadow: ShadowStyle) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Set mix-blend-mode.
    pub fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.mix_blend_mode = mode;
        self
    }

    /// Set display mode.
    pub fn with_display(mut self, display: Display) -> Self {
        self.display = display;
        self
    }

    /// Get effective background color (considering glass tint).
    pub fn effective_background(&self) -> Color {
        if let Some(glass) = &self.glass {
            glass.tint_color
        } else if let Some(bg) = self.background_color {
            bg
        } else {
            Color::new(0, 0, 0, 0)
        }
    }

    /// Check if element should be rendered (visible and has content).
    pub fn should_render(&self) -> bool {
        self.visibility && self.opacity > 0.0 && self.display != Display::None
    }

    /// Get computed z-order.
    pub fn z_order(&self) -> u32 {
        self.z_index.max(0) as u32
    }

    /// Check if this element requires compositing isolation.
    pub fn needs_compositing_layer(&self) -> bool {
        self.mix_blend_mode != BlendMode::SrcOver
            || self.isolation
            || !self.filter.is_empty()
            || !self.backdrop_filters.is_empty()
            || self.mask.is_some()
            || self.opacity < 1.0
    }
}

impl Padding {
    /// Create uniform padding.
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create padding from (vertical, horizontal).
    pub fn symmetric(vert: f32, horiz: f32) -> Self {
        Self {
            top: vert,
            bottom: vert,
            left: horiz,
            right: horiz,
        }
    }
}

impl Margin {
    /// Create uniform margin.
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create margin from (vertical, horizontal).
    pub fn symmetric(vert: f32, horiz: f32) -> Self {
        Self {
            top: vert,
            bottom: vert,
            left: horiz,
            right: horiz,
        }
    }
}

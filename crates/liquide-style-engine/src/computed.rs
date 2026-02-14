//! ComputedStyle — the fully resolved style for a single element.

use serde::{Deserialize, Serialize};

use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{
    BackdropFilterSpec, BackgroundSpec, BorderImageSpec, BoxShadowSpec, FilterSpec, MaskSpec,
    OutlineSpec, Overflow, TextDecoration, TextShadow,
};

use crate::dimension::{Corners, Dimension, Sides, Size};

// ── Display & Position ──────────────────────────────────────────────────────

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

impl Default for Display {
    fn default() -> Self {
        Display::Block
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

impl Default for Position {
    fn default() -> Self {
        Position::Static
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

impl Default for BoxSizing {
    fn default() -> Self {
        BoxSizing::ContentBox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Visible
    }
}

// ── Flexbox ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl Default for FlexDirection {
    fn default() -> Self {
        FlexDirection::Row
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl Default for FlexWrap {
    fn default() -> Self {
        FlexWrap::NoWrap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Default for JustifyContent {
    fn default() -> Self {
        JustifyContent::FlexStart
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

impl Default for AlignItems {
    fn default() -> Self {
        AlignItems::Stretch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignSelf {
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

impl Default for AlignSelf {
    fn default() -> Self {
        AlignSelf::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    Stretch,
}

impl Default for AlignContent {
    fn default() -> Self {
        AlignContent::Stretch
    }
}

// ── Grid ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrackSize {
    Px(f32),
    Percent(f32),
    Fr(f32),
    MinContent,
    MaxContent,
    Auto,
    MinMax(Box<TrackSize>, Box<TrackSize>),
    FitContent(f32),
}

impl Default for TrackSize {
    fn default() -> Self {
        TrackSize::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPlacement {
    pub start: GridLine,
    pub end: GridLine,
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self {
            start: GridLine::Auto,
            end: GridLine::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridLine {
    Auto,
    Line(i32),
    Span(u32),
}

impl Default for GridLine {
    fn default() -> Self {
        GridLine::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridAutoFlow {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl Default for GridAutoFlow {
    fn default() -> Self {
        GridAutoFlow::Row
    }
}

// ── Typography ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LineHeight {
    Normal,
    Number(f32),
    Px(f32),
}

impl Default for LineHeight {
    fn default() -> Self {
        LineHeight::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextTransform {
    None,
    Capitalize,
    Uppercase,
    Lowercase,
}

impl Default for TextTransform {
    fn default() -> Self {
        TextTransform::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

impl Default for TextOverflow {
    fn default() -> Self {
        TextOverflow::Clip
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

impl Default for WhiteSpace {
    fn default() -> Self {
        WhiteSpace::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordBreak {
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

impl Default for WordBreak {
    fn default() -> Self {
        WordBreak::Normal
    }
}

// ── Cursor & Pointer ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cursor {
    Auto,
    Default,
    Pointer,
    Text,
    Move,
    Crosshair,
    Wait,
    Help,
    NotAllowed,
    Grab,
    Grabbing,
    ColResize,
    RowResize,
    EResize,
    WResize,
    NResize,
    SResize,
    NeResize,
    NwResize,
    SeResize,
    SwResize,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerEvents {
    Auto,
    None,
}

impl Default for PointerEvents {
    fn default() -> Self {
        PointerEvents::Auto
    }
}

// ── Effects ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Isolation {
    Auto,
    Isolate,
}

impl Default for Isolation {
    fn default() -> Self {
        Isolation::Auto
    }
}

// ── Border ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderLineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
    Hidden,
}

impl Default for BorderLineStyle {
    fn default() -> Self {
        BorderLineStyle::None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        }
    }
}

// ── Transforms ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32),
    Skew(f32, f32),
    Matrix(f32, f32, f32, f32, f32, f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformOrigin {
    pub x: Dimension,
    pub y: Dimension,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self {
            x: Dimension::Percent(50.0),
            y: Dimension::Percent(50.0),
        }
    }
}

// ── Transition & Animation ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub property: String,
    pub duration_ms: f32,
    pub timing_function: TimingFunction,
    pub delay_ms: f32,
}

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

// ═══════════════════════════════════════════════════════════════════════════
// The ComputedStyle
// ═══════════════════════════════════════════════════════════════════════════

/// Fully resolved style for a single element.
///
/// All values are concrete — no `auto`, no `inherit` (those are resolved
/// during style computation). Option<> is used only for genuinely optional
/// features (background-image, outline, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedStyle {
    // ── Box model ──
    pub display: Display,
    pub position: Position,
    pub box_sizing: BoxSizing,
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub max_width: Dimension,
    pub min_height: Dimension,
    pub max_height: Dimension,
    pub margin: Sides<Dimension>,
    pub padding: Sides<Dimension>,
    pub border_width: Sides<f32>,
    pub border_style: Sides<BorderLineStyle>,
    pub border_color: Sides<Color>,
    pub border_radius: Corners<f32>,
    pub border_image: Option<BorderImageSpec>,

    // ── Flexbox ──
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub align_content: AlignContent,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub gap: Size<Dimension>,
    pub order: i32,

    // ── Grid ──
    pub grid_template_columns: Vec<TrackSize>,
    pub grid_template_rows: Vec<TrackSize>,
    pub grid_column: GridPlacement,
    pub grid_row: GridPlacement,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_auto_columns: TrackSize,
    pub grid_auto_rows: TrackSize,

    // ── Positioning ──
    pub top: Dimension,
    pub right: Dimension,
    pub bottom: Dimension,
    pub left: Dimension,
    pub z_index: Option<i32>,

    // ── Typography (inherited) ──
    pub color: Color,
    pub font_family: Vec<String>,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_style: FontStyle,
    pub line_height: LineHeight,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub text_align: TextAlign,
    pub text_decoration: Option<TextDecoration>,
    pub text_transform: TextTransform,
    pub text_overflow: TextOverflow,
    pub text_shadow: Vec<TextShadow>,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,
    pub text_indent: f32,

    // ── Visual ──
    pub background_color: Color,
    pub background: Option<BackgroundSpec>,
    pub box_shadow: Vec<BoxShadowSpec>,
    pub opacity: f32,
    pub visibility: Visibility,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub cursor: Cursor,
    pub pointer_events: PointerEvents,

    // ── Effects ──
    pub transform: Vec<Transform>,
    pub transform_origin: TransformOrigin,
    pub filter: Vec<FilterSpec>,
    pub backdrop_filter: Vec<BackdropFilterSpec>,
    pub mix_blend_mode: BlendMode,
    pub isolation: Isolation,
    pub mask: Option<MaskSpec>,
    pub outline: Option<OutlineSpec>,

    // ── Transitions & animations ──
    pub transition: Vec<TransitionDef>,
    pub animation: Vec<AnimationDef>,

    // ── Shell custom extensions ──
    // Non-standard CSS properties for LiquiDE desktop chrome.
    /// `backdrop-blur-radius` — shorthand for `backdrop-filter: blur(Npx)`.
    pub x_blur_radius: f32,
    /// `glass-tint` — tint color for frosted-glass surfaces.
    pub x_glass_tint: Option<Color>,
    /// Generic custom properties bag (for `--var` consumption).
    pub x_custom: Vec<(String, String)>,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            // Box model
            display: Display::default(),
            position: Position::default(),
            box_sizing: BoxSizing::default(),
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_width: Dimension::Auto,
            max_width: Dimension::None,
            min_height: Dimension::Auto,
            max_height: Dimension::None,
            margin: Sides::all(Dimension::Zero),
            padding: Sides::all(Dimension::Zero),
            border_width: Sides::all(0.0),
            border_style: Sides::all(BorderLineStyle::None),
            border_color: Sides::all(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }),
            border_radius: Corners::all(0.0),
            border_image: None,

            // Flex
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_self: AlignSelf::default(),
            align_content: AlignContent::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            gap: Size::default(),
            order: 0,

            // Grid
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column: GridPlacement::default(),
            grid_row: GridPlacement::default(),
            grid_auto_flow: GridAutoFlow::default(),
            grid_auto_columns: TrackSize::Auto,
            grid_auto_rows: TrackSize::Auto,

            // Positioning
            top: Dimension::Auto,
            right: Dimension::Auto,
            bottom: Dimension::Auto,
            left: Dimension::Auto,
            z_index: None,

            // Typography
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_family: vec!["sans-serif".to_string()],
            font_size: 16.0,
            font_weight: 400,
            font_style: FontStyle::default(),
            line_height: LineHeight::default(),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            text_align: TextAlign::default(),
            text_decoration: None,
            text_transform: TextTransform::default(),
            text_overflow: TextOverflow::default(),
            text_shadow: Vec::new(),
            white_space: WhiteSpace::default(),
            word_break: WordBreak::default(),
            text_indent: 0.0,

            // Visual
            background_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            background: None,
            box_shadow: Vec::new(),
            opacity: 1.0,
            visibility: Visibility::default(),
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            cursor: Cursor::default(),
            pointer_events: PointerEvents::default(),

            // Effects
            transform: Vec::new(),
            transform_origin: TransformOrigin::default(),
            filter: Vec::new(),
            backdrop_filter: Vec::new(),
            mix_blend_mode: BlendMode::SrcOver,
            isolation: Isolation::default(),
            mask: None,
            outline: None,

            // Transitions & animations
            transition: Vec::new(),
            animation: Vec::new(),

            // Shell custom extensions
            x_blur_radius: 0.0,
            x_glass_tint: None,
            x_custom: Vec::new(),
        }
    }
}

impl ComputedStyle {
    /// Inherit inherited properties from a parent style.
    pub fn inherit_from(&mut self, parent: &ComputedStyle) {
        // Typography (inherited)
        self.color = parent.color;
        self.font_family = parent.font_family.clone();
        self.font_size = parent.font_size;
        self.font_weight = parent.font_weight;
        self.font_style = parent.font_style;
        self.line_height = parent.line_height.clone();
        self.letter_spacing = parent.letter_spacing;
        self.word_spacing = parent.word_spacing;
        self.text_align = parent.text_align;
        self.text_transform = parent.text_transform;
        self.white_space = parent.white_space;
        self.word_break = parent.word_break;
        self.text_indent = parent.text_indent;
        // Visibility & cursor
        self.visibility = parent.visibility;
        self.cursor = parent.cursor;
    }

    /// Does this element establish a new stacking context?
    pub fn creates_stacking_context(&self) -> bool {
        self.z_index.is_some()
            || self.opacity < 1.0
            || !self.transform.is_empty()
            || !self.filter.is_empty()
            || !self.backdrop_filter.is_empty()
            || self.mix_blend_mode != BlendMode::SrcOver
            || self.isolation == Isolation::Isolate
            || self.position == Position::Fixed
            || self.position == Position::Sticky
    }

    /// Is this element visible?
    pub fn is_visible(&self) -> bool {
        self.display != Display::None
            && self.visibility == Visibility::Visible
            && self.opacity > 0.0
    }

    /// Is this element a flex container?
    pub fn is_flex_container(&self) -> bool {
        matches!(self.display, Display::Flex | Display::InlineFlex)
    }

    /// Is this element a grid container?
    pub fn is_grid_container(&self) -> bool {
        matches!(self.display, Display::Grid | Display::InlineGrid)
    }

    /// Is this element positioned (not static)?
    pub fn is_positioned(&self) -> bool {
        self.position != Position::Static
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let s = ComputedStyle::default();
        assert_eq!(s.display, Display::Block);
        assert_eq!(s.position, Position::Static);
        assert_eq!(s.opacity, 1.0);
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.font_weight, 400);
    }

    #[test]
    fn inherit_from_parent() {
        let mut parent = ComputedStyle::default();
        parent.color = Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        parent.font_size = 20.0;
        parent.cursor = Cursor::Pointer;

        let mut child = ComputedStyle::default();
        child.inherit_from(&parent);

        assert_eq!(child.color, parent.color);
        assert_eq!(child.font_size, 20.0);
        assert_eq!(child.cursor, Cursor::Pointer);
        // Non-inherited properties should NOT be copied
        assert_eq!(child.display, Display::Block); // default, not parent's
    }

    #[test]
    fn stacking_context() {
        let mut s = ComputedStyle::default();
        assert!(!s.creates_stacking_context());

        s.opacity = 0.5;
        assert!(s.creates_stacking_context());
    }
}

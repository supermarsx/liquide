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
    Table,
    TableRow,
    TableCell,
    TableRowGroup,
    TableCaption,
    None,
    Contents,
    /// Establishes a new block formatting context (prevents margin collapse).
    FlowRoot,
    /// Block with a list marker (outside or inside).
    ListItem,
    /// Ruby annotation container.
    Ruby,
    /// Ruby text container.
    RubyText,
    /// Run-in box (collapses into following block if possible).
    RunIn,
    /// Table header group.
    TableHeaderGroup,
    /// Table footer group.
    TableFooterGroup,
    /// Table column.
    TableColumn,
    /// Table column group.
    TableColumnGroup,
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
    /// CSS Subgrid — inherits tracks from parent grid.
    Subgrid,
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

// ── Float & Clear ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Float {
    None,
    Left,
    Right,
    InlineStart,
    InlineEnd,
}

impl Default for Float {
    fn default() -> Self {
        Float::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clear {
    None,
    Left,
    Right,
    Both,
    InlineStart,
    InlineEnd,
}

impl Default for Clear {
    fn default() -> Self {
        Clear::None
    }
}

// ── Writing mode & direction ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritingMode {
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
}

impl Default for WritingMode {
    fn default() -> Self {
        WritingMode::HorizontalTb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Ltr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnicodeBidi {
    Normal,
    Embed,
    Isolate,
    BidiOverride,
    IsolateOverride,
    Plaintext,
}

impl Default for UnicodeBidi {
    fn default() -> Self {
        UnicodeBidi::Normal
    }
}

// ── Containment ─────────────────────────────────────────────────────────────

/// CSS contain property (bitflags style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contain {
    pub size: bool,
    pub layout: bool,
    pub style: bool,
    pub paint: bool,
    pub inline_size: bool,
}

impl Default for Contain {
    fn default() -> Self {
        Self {
            size: false,
            layout: false,
            style: false,
            paint: false,
            inline_size: false,
        }
    }
}

impl Contain {
    pub fn none() -> Self {
        Self::default()
    }
    pub fn strict() -> Self {
        Self {
            size: true,
            layout: true,
            style: true,
            paint: true,
            inline_size: false,
        }
    }
    pub fn content() -> Self {
        Self {
            size: false,
            layout: true,
            style: true,
            paint: true,
            inline_size: false,
        }
    }
}

// ── Resize ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resize {
    None,
    Both,
    Horizontal,
    Vertical,
    Block,
    Inline,
}

impl Default for Resize {
    fn default() -> Self {
        Resize::None
    }
}

// ── User interaction ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserSelect {
    Auto,
    None,
    Text,
    All,
    Contain,
}

impl Default for UserSelect {
    fn default() -> Self {
        UserSelect::Auto
    }
}

// ── Scroll behavior ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollBehavior {
    Auto,
    Smooth,
}

impl Default for ScrollBehavior {
    fn default() -> Self {
        ScrollBehavior::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverscrollBehavior {
    Auto,
    Contain,
    None,
}

impl Default for OverscrollBehavior {
    fn default() -> Self {
        OverscrollBehavior::Auto
    }
}

// ── Object fit (for images/replaced elements) ───────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectFit {
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

impl Default for ObjectFit {
    fn default() -> Self {
        ObjectFit::Fill
    }
}

// ── List styling ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListStyleType {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    DecimalLeadingZero,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
    LowerLatin,
    UpperLatin,
}

impl Default for ListStyleType {
    fn default() -> Self {
        ListStyleType::Disc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListStylePosition {
    Outside,
    Inside,
}

impl Default for ListStylePosition {
    fn default() -> Self {
        ListStylePosition::Outside
    }
}

// ── Table layout ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableLayout {
    Auto,
    Fixed,
}

impl Default for TableLayout {
    fn default() -> Self {
        TableLayout::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderCollapse {
    Separate,
    Collapse,
}

impl Default for BorderCollapse {
    fn default() -> Self {
        BorderCollapse::Separate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmptyCells {
    Show,
    Hide,
}

impl Default for EmptyCells {
    fn default() -> Self {
        EmptyCells::Show
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptionSide {
    Top,
    Bottom,
}

impl Default for CaptionSide {
    fn default() -> Self {
        CaptionSide::Top
    }
}

// ── Vertical alignment ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerticalAlign {
    Baseline,
    Sub,
    Super,
    Top,
    TextTop,
    Middle,
    Bottom,
    TextBottom,
    Length(f32),
}

impl Default for VerticalAlign {
    fn default() -> Self {
        VerticalAlign::Baseline
    }
}

// ── Justify items/self ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyItems {
    Normal,
    Stretch,
    Center,
    Start,
    End,
    FlexStart,
    FlexEnd,
    SelfStart,
    SelfEnd,
    Left,
    Right,
    Legacy,
}

impl Default for JustifyItems {
    fn default() -> Self {
        JustifyItems::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifySelf {
    Auto,
    Normal,
    Stretch,
    Center,
    Start,
    End,
    FlexStart,
    FlexEnd,
    SelfStart,
    SelfEnd,
}

impl Default for JustifySelf {
    fn default() -> Self {
        JustifySelf::Auto
    }
}

// ── Appearance ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    None,
    Auto,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance::Auto
    }
}

// ── Content visibility ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentVisibility {
    Visible,
    Auto,
    Hidden,
}

impl Default for ContentVisibility {
    fn default() -> Self {
        ContentVisibility::Visible
    }
}

// ── Aspect ratio ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AspectRatio {
    Auto,
    Ratio(f32, f32),
}

impl Default for AspectRatio {
    fn default() -> Self {
        AspectRatio::Auto
    }
}

// ── Backface visibility ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackfaceVisibility {
    Visible,
    Hidden,
}

impl Default for BackfaceVisibility {
    fn default() -> Self {
        BackfaceVisibility::Visible
    }
}

// ── Transform style ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformStyle {
    Flat,
    Preserve3d,
}

impl Default for TransformStyle {
    fn default() -> Self {
        TransformStyle::Flat
    }
}

// ── Transform box ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformBox {
    ContentBox,
    BorderBox,
    FillBox,
    StrokeBox,
    ViewBox,
}

impl Default for TransformBox {
    fn default() -> Self {
        TransformBox::ViewBox
    }
}

// ── Perspective ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Perspective {
    None,
    Length(f32),
}

impl Default for Perspective {
    fn default() -> Self {
        Perspective::None
    }
}

// ── Hyphens ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hyphens {
    None,
    Manual,
    Auto,
}

impl Default for Hyphens {
    fn default() -> Self {
        Hyphens::Manual
    }
}

// ── Overflow wrap / word-wrap ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowWrap {
    Normal,
    BreakWord,
    Anywhere,
}

impl Default for OverflowWrap {
    fn default() -> Self {
        OverflowWrap::Normal
    }
}

// ── Text decoration details ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationSkipInk {
    Auto,
    All,
    None,
}

impl Default for TextDecorationSkipInk {
    fn default() -> Self {
        TextDecorationSkipInk::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextUnderlinePosition {
    Auto,
    Under,
    Left,
    Right,
    FromFont,
}

impl Default for TextUnderlinePosition {
    fn default() -> Self {
        TextUnderlinePosition::Auto
    }
}

// ── Text alignment extras ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignLast {
    Auto,
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

impl Default for TextAlignLast {
    fn default() -> Self {
        TextAlignLast::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextJustify {
    Auto,
    InterCharacter,
    InterWord,
    None,
}

impl Default for TextJustify {
    fn default() -> Self {
        TextJustify::Auto
    }
}

// ── Font extras ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl Default for FontStretch {
    fn default() -> Self {
        FontStretch::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontKerning {
    Auto,
    Normal,
    None,
}

impl Default for FontKerning {
    fn default() -> Self {
        FontKerning::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantCaps {
    Normal,
    SmallCaps,
    AllSmallCaps,
    PetiteCaps,
    AllPetiteCaps,
    Unicase,
    TitlingCaps,
}

impl Default for FontVariantCaps {
    fn default() -> Self {
        FontVariantCaps::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantNumeric {
    Normal,
    OldstyleNums,
    LiningNums,
    TabularNums,
    ProportionalNums,
}

impl Default for FontVariantNumeric {
    fn default() -> Self {
        FontVariantNumeric::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontOpticalSizing {
    Auto,
    None,
}

impl Default for FontOpticalSizing {
    fn default() -> Self {
        FontOpticalSizing::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FontSizeAdjust {
    None,
    Number(f32),
}

impl Default for FontSizeAdjust {
    fn default() -> Self {
        FontSizeAdjust::None
    }
}

// ── Image rendering ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageRendering {
    Auto,
    CrispEdges,
    Pixelated,
    HighQuality,
    Smooth,
}

impl Default for ImageRendering {
    fn default() -> Self {
        ImageRendering::Auto
    }
}

// ── Text rendering ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

impl Default for TextRendering {
    fn default() -> Self {
        TextRendering::Auto
    }
}

// ── Touch action ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchAction {
    pub pan_x: bool,
    pub pan_y: bool,
    pub pinch_zoom: bool,
    pub manipulation: bool,
    pub none: bool,
}

impl Default for TouchAction {
    fn default() -> Self {
        Self {
            pan_x: true,
            pan_y: true,
            pinch_zoom: true,
            manipulation: false,
            none: false,
        }
    }
}

impl TouchAction {
    pub fn auto() -> Self {
        Self::default()
    }
    pub fn none_val() -> Self {
        Self {
            pan_x: false,
            pan_y: false,
            pinch_zoom: false,
            manipulation: false,
            none: true,
        }
    }
    pub fn manipulation_val() -> Self {
        Self {
            pan_x: true,
            pan_y: true,
            pinch_zoom: true,
            manipulation: true,
            none: false,
        }
    }
}

// ── Scroll snap ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapType {
    None,
    X(ScrollSnapStrictness),
    Y(ScrollSnapStrictness),
    Block(ScrollSnapStrictness),
    Inline(ScrollSnapStrictness),
    Both(ScrollSnapStrictness),
}

impl Default for ScrollSnapType {
    fn default() -> Self {
        ScrollSnapType::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapStrictness {
    Mandatory,
    Proximity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapAlign {
    None,
    Start,
    End,
    Center,
}

impl Default for ScrollSnapAlign {
    fn default() -> Self {
        ScrollSnapAlign::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapStop {
    Normal,
    Always,
}

impl Default for ScrollSnapStop {
    fn default() -> Self {
        ScrollSnapStop::Normal
    }
}

// ── Color scheme ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorScheme {
    Normal,
    Light,
    Dark,
    LightDark,
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme::Normal
    }
}

// ── Print / fragmentation ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakValue {
    Auto,
    Avoid,
    AvoidPage,
    AvoidColumn,
    AvoidRegion,
    Page,
    Column,
    Region,
    Left,
    Right,
    Recto,
    Verso,
    Always,
}

impl Default for BreakValue {
    fn default() -> Self {
        BreakValue::Auto
    }
}

// ── Box decoration break ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxDecorationBreak {
    Slice,
    Clone,
}

impl Default for BoxDecorationBreak {
    fn default() -> Self {
        BoxDecorationBreak::Slice
    }
}

// ── Line clamp ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineClamp {
    None,
    Count(u32),
}

impl Default for LineClamp {
    fn default() -> Self {
        LineClamp::None
    }
}

// ── Column rule ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnRule {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

impl Default for ColumnRule {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnFill {
    Balance,
    Auto,
}

impl Default for ColumnFill {
    fn default() -> Self {
        ColumnFill::Balance
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnSpan {
    None,
    All,
}

impl Default for ColumnSpan {
    fn default() -> Self {
        ColumnSpan::None
    }
}

// ── Background attachment / clip / origin ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundAttachment {
    Scroll,
    Fixed,
    Local,
}

impl Default for BackgroundAttachment {
    fn default() -> Self {
        BackgroundAttachment::Scroll
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundClip {
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
}

impl Default for BackgroundClip {
    fn default() -> Self {
        BackgroundClip::BorderBox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundOrigin {
    BorderBox,
    PaddingBox,
    ContentBox,
}

impl Default for BackgroundOrigin {
    fn default() -> Self {
        BackgroundOrigin::PaddingBox
    }
}

// ── Paint order ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintOrder {
    Normal,
    Fill,
    Stroke,
    Markers,
}

impl Default for PaintOrder {
    fn default() -> Self {
        PaintOrder::Normal
    }
}

// ── Forced colors & print ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForcedColorAdjust {
    Auto,
    None,
}

impl Default for ForcedColorAdjust {
    fn default() -> Self {
        ForcedColorAdjust::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintColorAdjust {
    Economy,
    Exact,
}

impl Default for PrintColorAdjust {
    fn default() -> Self {
        PrintColorAdjust::Economy
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

// ── Transition longhands ──

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

// ── Animation extras ──

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

// ── Font extras ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantAlternates {
    Normal,
    HistoricalForms,
}
impl Default for FontVariantAlternates {
    fn default() -> Self {
        FontVariantAlternates::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantEastAsian {
    Normal,
    Jis78,
    Jis83,
    Jis90,
    Jis04,
    Simplified,
    Traditional,
    FullWidth,
    ProportionalWidth,
    Ruby,
}
impl Default for FontVariantEastAsian {
    fn default() -> Self {
        FontVariantEastAsian::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantLigatures {
    Normal,
    None,
    CommonLigatures,
    NoCommonLigatures,
    DiscretionaryLigatures,
    NoDiscretionaryLigatures,
    HistoricalLigatures,
    NoHistoricalLigatures,
    Contextual,
    NoContextual,
}
impl Default for FontVariantLigatures {
    fn default() -> Self {
        FontVariantLigatures::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantPosition {
    Normal,
    Sub,
    Super,
}
impl Default for FontVariantPosition {
    fn default() -> Self {
        FontVariantPosition::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantEmoji {
    Normal,
    Text,
    Emoji,
    Unicode,
}
impl Default for FontVariantEmoji {
    fn default() -> Self {
        FontVariantEmoji::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisWeight {
    Auto,
    None,
}
impl Default for FontSynthesisWeight {
    fn default() -> Self {
        FontSynthesisWeight::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisStyle {
    Auto,
    None,
}
impl Default for FontSynthesisStyle {
    fn default() -> Self {
        FontSynthesisStyle::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisSmallCaps {
    Auto,
    None,
}
impl Default for FontSynthesisSmallCaps {
    fn default() -> Self {
        FontSynthesisSmallCaps::Auto
    }
}

// ── Text extras ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOrientation {
    Mixed,
    Upright,
    Sideways,
}
impl Default for TextOrientation {
    fn default() -> Self {
        TextOrientation::Mixed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextCombineUpright {
    None,
    All,
    Digits(u8),
}
impl Default for TextCombineUpright {
    fn default() -> Self {
        TextCombineUpright::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextWrapMode {
    Wrap,
    NoWrap,
}
impl Default for TextWrapMode {
    fn default() -> Self {
        TextWrapMode::Wrap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextWrapStyle {
    Auto,
    Balance,
    Pretty,
    Stable,
}
impl Default for TextWrapStyle {
    fn default() -> Self {
        TextWrapStyle::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextBoxTrim {
    None,
    TrimStart,
    TrimEnd,
    TrimBoth,
}
impl Default for TextBoxTrim {
    fn default() -> Self {
        TextBoxTrim::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhiteSpaceCollapse {
    Collapse,
    Preserve,
    PreserveBreaks,
    PreserveSpaces,
    BreakSpaces,
}
impl Default for WhiteSpaceCollapse {
    fn default() -> Self {
        WhiteSpaceCollapse::Collapse
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineBreak {
    Auto,
    Loose,
    Normal,
    Strict,
    Anywhere,
}
impl Default for LineBreak {
    fn default() -> Self {
        LineBreak::Auto
    }
}

// ── Overflow / scroll extras ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowAnchor {
    Auto,
    None,
}
impl Default for OverflowAnchor {
    fn default() -> Self {
        OverflowAnchor::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollbarWidth {
    Auto,
    Thin,
    None,
}
impl Default for ScrollbarWidth {
    fn default() -> Self {
        ScrollbarWidth::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollbarGutter {
    Auto,
    Stable,
    StableBothEdges,
}
impl Default for ScrollbarGutter {
    fn default() -> Self {
        ScrollbarGutter::Auto
    }
}

// ── Containment extras ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerType {
    Normal,
    InlineSize,
    Size,
}
impl Default for ContainerType {
    fn default() -> Self {
        ContainerType::Normal
    }
}

// ── Shape ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeOutside {
    None,
    MarginBox,
    BorderBox,
    PaddingBox,
    ContentBox,
}
impl Default for ShapeOutside {
    fn default() -> Self {
        ShapeOutside::None
    }
}

// ── Object view box ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageOrientation {
    FromImage,
    None,
}
impl Default for ImageOrientation {
    fn default() -> Self {
        ImageOrientation::FromImage
    }
}

// ── SVG presentation ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
impl Default for FillRule {
    fn default() -> Self {
        FillRule::NonZero
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeLinecap {
    Butt,
    Round,
    Square,
}
impl Default for StrokeLinecap {
    fn default() -> Self {
        StrokeLinecap::Butt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeLinejoin {
    Miter,
    Round,
    Bevel,
}
impl Default for StrokeLinejoin {
    fn default() -> Self {
        StrokeLinejoin::Miter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DominantBaseline {
    Auto,
    TextBottom,
    Alphabetic,
    Ideographic,
    Middle,
    Central,
    Mathematical,
    Hanging,
    TextTop,
}
impl Default for DominantBaseline {
    fn default() -> Self {
        DominantBaseline::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignmentBaseline {
    Auto,
    Baseline,
    TextBottom,
    Alphabetic,
    Ideographic,
    Middle,
    Central,
    Mathematical,
    TextTop,
}
impl Default for AlignmentBaseline {
    fn default() -> Self {
        AlignmentBaseline::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipRule {
    NonZero,
    EvenOdd,
}
impl Default for ClipRule {
    fn default() -> Self {
        ClipRule::NonZero
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}
impl Default for ShapeRendering {
    fn default() -> Self {
        ShapeRendering::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}
impl Default for VectorEffect {
    fn default() -> Self {
        VectorEffect::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}
impl Default for TextAnchor {
    fn default() -> Self {
        TextAnchor::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorInterpolation {
    Auto,
    SRGB,
    LinearRGB,
}
impl Default for ColorInterpolation {
    fn default() -> Self {
        ColorInterpolation::SRGB
    }
}

// ── Mask type ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskType {
    Luminance,
    Alpha,
}
impl Default for MaskType {
    fn default() -> Self {
        MaskType::Luminance
    }
}

// ── Ruby ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RubyPosition {
    Over,
    Under,
    AlternateOver,
    AlternateUnder,
}
impl Default for RubyPosition {
    fn default() -> Self {
        RubyPosition::Over
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RubyAlign {
    SpaceAround,
    Center,
    Start,
    SpaceBetween,
}
impl Default for RubyAlign {
    fn default() -> Self {
        RubyAlign::SpaceAround
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

    // ── Float & clear ──
    pub float: Float,
    pub clear: Clear,

    // ── Writing mode ──
    pub writing_mode: WritingMode,
    pub direction: Direction,
    pub unicode_bidi: UnicodeBidi,

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
    pub vertical_align: VerticalAlign,
    pub tab_size: f32,

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

    // ── Layout extras ──
    pub contain: Contain,
    pub content_visibility: ContentVisibility,
    pub aspect_ratio: AspectRatio,
    pub object_fit: ObjectFit,
    pub object_position_x: Dimension,
    pub object_position_y: Dimension,
    pub resize: Resize,
    pub column_count: Option<u32>,
    pub column_width: Dimension,
    pub column_gap: Dimension,
    pub row_gap: Dimension,

    // ── Alignment extras ──
    pub justify_items: JustifyItems,
    pub justify_self: JustifySelf,
    pub place_content: Option<(AlignContent, JustifyContent)>,

    // ── List styling ──
    pub list_style_type: ListStyleType,
    pub list_style_position: ListStylePosition,

    // ── Table ──
    pub table_layout: TableLayout,
    pub border_collapse: BorderCollapse,
    pub border_spacing: f32,
    pub empty_cells: EmptyCells,
    pub caption_side: CaptionSide,

    // ── User interaction ──
    pub user_select: UserSelect,
    pub appearance: Appearance,
    pub scroll_behavior: ScrollBehavior,
    pub overscroll_behavior_x: OverscrollBehavior,
    pub overscroll_behavior_y: OverscrollBehavior,

    // ── Will-change ──
    pub will_change: Vec<String>,

    // ── Effects ──
    pub transform: Vec<Transform>,
    pub transform_origin: TransformOrigin,
    pub transform_style: TransformStyle,
    pub transform_box: TransformBox,
    pub perspective: Perspective,
    pub perspective_origin: TransformOrigin,
    pub backface_visibility: BackfaceVisibility,
    pub filter: Vec<FilterSpec>,
    pub backdrop_filter: Vec<BackdropFilterSpec>,
    pub mix_blend_mode: BlendMode,
    pub isolation: Isolation,
    pub mask: Option<MaskSpec>,
    pub clip_path: Option<String>,
    pub outline: Option<OutlineSpec>,

    // ── Transitions & animations ──
    pub transition: Vec<TransitionDef>,
    pub animation: Vec<AnimationDef>,

    // ── Typography extras ──
    pub overflow_wrap: OverflowWrap,
    pub hyphens: Hyphens,
    pub text_decoration_line: Option<String>,
    pub text_decoration_style: Option<String>,
    pub text_decoration_color: Option<Color>,
    pub text_decoration_thickness: Option<f32>,
    pub text_decoration_skip_ink: TextDecorationSkipInk,
    pub text_underline_offset: f32,
    pub text_underline_position: TextUnderlinePosition,
    pub text_align_last: TextAlignLast,
    pub text_justify: TextJustify,
    pub text_rendering: TextRendering,
    pub font_stretch: FontStretch,
    pub font_kerning: FontKerning,
    pub font_variant_caps: FontVariantCaps,
    pub font_variant_numeric: FontVariantNumeric,
    pub font_optical_sizing: FontOpticalSizing,
    pub font_size_adjust: FontSizeAdjust,
    pub font_feature_settings: Option<String>,
    pub font_variation_settings: Option<String>,
    pub line_clamp: LineClamp,

    // ── Image rendering ──
    pub image_rendering: ImageRendering,

    // ── Input / interaction extras ──
    pub touch_action: TouchAction,
    pub caret_color: Option<Color>,
    pub accent_color: Option<Color>,
    pub color_scheme: ColorScheme,
    pub forced_color_adjust: ForcedColorAdjust,
    pub print_color_adjust: PrintColorAdjust,

    // ── Scroll snap ──
    pub scroll_snap_type: ScrollSnapType,
    pub scroll_snap_align: ScrollSnapAlign,
    pub scroll_snap_stop: ScrollSnapStop,
    pub scroll_padding: Sides<Dimension>,
    pub scroll_margin: Sides<Dimension>,

    // ── Fragmentation ──
    pub break_before: BreakValue,
    pub break_after: BreakValue,
    pub break_inside: BreakValue,
    pub orphans: u32,
    pub widows: u32,
    pub box_decoration_break: BoxDecorationBreak,

    // ── Column extras ──
    pub column_rule: ColumnRule,
    pub column_fill: ColumnFill,
    pub column_span: ColumnSpan,

    // ── Background extras ──
    pub background_attachment: BackgroundAttachment,
    pub background_clip: BackgroundClip,
    pub background_origin: BackgroundOrigin,
    pub background_blend_mode: BlendMode,
    pub background_position_x: Dimension,
    pub background_position_y: Dimension,
    pub background_size: Option<String>,
    pub background_repeat: Option<String>,
    pub background_image: Option<String>,

    // ── Logical properties ──
    pub inline_size: Dimension,
    pub block_size: Dimension,
    pub min_inline_size: Dimension,
    pub min_block_size: Dimension,
    pub max_inline_size: Dimension,
    pub max_block_size: Dimension,
    pub margin_inline_start: Dimension,
    pub margin_inline_end: Dimension,
    pub margin_block_start: Dimension,
    pub margin_block_end: Dimension,
    pub padding_inline_start: Dimension,
    pub padding_inline_end: Dimension,
    pub padding_block_start: Dimension,
    pub padding_block_end: Dimension,
    pub inset_inline_start: Dimension,
    pub inset_inline_end: Dimension,
    pub inset_block_start: Dimension,
    pub inset_block_end: Dimension,
    pub border_inline_start_width: f32,
    pub border_inline_end_width: f32,
    pub border_block_start_width: f32,
    pub border_block_end_width: f32,
    pub border_inline_start_style: BorderLineStyle,
    pub border_inline_end_style: BorderLineStyle,
    pub border_block_start_style: BorderLineStyle,
    pub border_block_end_style: BorderLineStyle,
    pub border_inline_start_color: Color,
    pub border_inline_end_color: Color,
    pub border_block_start_color: Color,
    pub border_block_end_color: Color,

    // ── Grid extras ──
    pub grid_column_start: GridLine,
    pub grid_column_end: GridLine,
    pub grid_row_start: GridLine,
    pub grid_row_end: GridLine,
    pub grid_template_areas: Vec<String>,

    // ── Content & counters ──
    pub content: Option<String>,
    pub counter_increment: Option<String>,
    pub counter_reset: Option<String>,
    pub counter_set: Option<String>,
    pub quotes: Option<String>,

    // ── SVG / paint order ──
    pub paint_order: PaintOrder,

    // ── Transition longhands ──
    pub transition_property: Option<String>,
    pub transition_duration: Option<String>,
    pub transition_timing_function: Option<String>,
    pub transition_delay: Option<String>,
    pub transition_behavior: TransitionBehavior,

    // ── Animation longhands ──
    pub animation_name: Option<String>,
    pub animation_duration: Option<String>,
    pub animation_timing_function: Option<String>,
    pub animation_delay: Option<String>,
    pub animation_iteration_count: AnimationIterationCount,
    pub animation_direction: AnimationDirection,
    pub animation_fill_mode: AnimationFillMode,
    pub animation_play_state: AnimationPlayState,
    pub animation_composition: AnimationComposition,
    pub animation_timeline: Option<String>,

    // ── Motion path (offset-*) ──
    pub offset_path: Option<String>,
    pub offset_distance: Dimension,
    pub offset_rotate: Option<String>,
    pub offset_anchor: Option<String>,
    pub offset_position: Option<String>,

    // ── Individual transform properties ──
    pub rotate: Option<String>,
    pub scale: Option<String>,
    pub translate: Option<String>,

    // ── Font extras (CSS spec coverage) ──
    pub font_variant_alternates: FontVariantAlternates,
    pub font_variant_east_asian: FontVariantEastAsian,
    pub font_variant_ligatures: FontVariantLigatures,
    pub font_variant_position: FontVariantPosition,
    pub font_variant_emoji: FontVariantEmoji,
    pub font_synthesis_weight: FontSynthesisWeight,
    pub font_synthesis_style: FontSynthesisStyle,
    pub font_synthesis_small_caps: FontSynthesisSmallCaps,
    pub font_language_override: Option<String>,
    pub font_palette: Option<String>,

    // ── Text extras (CSS spec coverage) ──
    pub text_emphasis_style: Option<String>,
    pub text_emphasis_color: Option<Color>,
    pub text_emphasis_position: Option<String>,
    pub text_orientation: TextOrientation,
    pub text_combine_upright: TextCombineUpright,
    pub text_wrap_mode: TextWrapMode,
    pub text_wrap_style: TextWrapStyle,
    pub text_box_trim: TextBoxTrim,
    pub text_box_edge: Option<String>,
    pub text_size_adjust: Option<String>,
    pub text_spacing_trim: Option<String>,
    pub white_space_collapse: WhiteSpaceCollapse,
    pub line_break: LineBreak,
    pub hyphenate_character: Option<String>,
    pub hyphenate_limit_chars: Option<String>,
    pub hanging_punctuation: Option<String>,
    pub initial_letter: Option<String>,
    pub text_autospace: Option<String>,

    // ── Overflow / scroll extras (CSS spec coverage) ──
    pub overflow_anchor: OverflowAnchor,
    pub overflow_clip_margin: Option<f32>,
    pub scrollbar_width: ScrollbarWidth,
    pub scrollbar_gutter: ScrollbarGutter,
    pub scrollbar_color: Option<(Color, Color)>,

    // ── Containment extras ──
    pub container_type: ContainerType,
    pub container_name: Option<String>,
    pub contain_intrinsic_width: Dimension,
    pub contain_intrinsic_height: Dimension,

    // ── Shape ──
    pub shape_outside: Option<String>,
    pub shape_margin: f32,
    pub shape_image_threshold: f32,

    // ── Border image longhands ──
    pub border_image_source: Option<String>,
    pub border_image_slice: Option<String>,
    pub border_image_width: Option<String>,
    pub border_image_outset: Option<String>,
    pub border_image_repeat: Option<String>,

    // ── Logical border radius ──
    pub border_start_start_radius: f32,
    pub border_start_end_radius: f32,
    pub border_end_start_radius: f32,
    pub border_end_end_radius: f32,

    // ── Mask longhands ──
    pub mask_image: Option<String>,
    pub mask_mode: Option<String>,
    pub mask_position: Option<String>,
    pub mask_size: Option<String>,
    pub mask_repeat: Option<String>,
    pub mask_origin: Option<String>,
    pub mask_clip: Option<String>,
    pub mask_composite: Option<String>,
    pub mask_type: MaskType,

    // ── Image extras ──
    pub image_orientation: ImageOrientation,

    // ── SVG presentation properties ──
    pub fill: Option<String>,
    pub fill_opacity: f32,
    pub fill_rule: FillRule,
    pub stroke: Option<String>,
    pub stroke_width: Dimension,
    pub stroke_dasharray: Option<String>,
    pub stroke_dashoffset: Dimension,
    pub stroke_linecap: StrokeLinecap,
    pub stroke_linejoin: StrokeLinejoin,
    pub stroke_miterlimit: f32,
    pub stroke_opacity: f32,
    pub color_interpolation: ColorInterpolation,
    pub color_interpolation_filters: ColorInterpolation,
    pub flood_color: Color,
    pub flood_opacity: f32,
    pub lighting_color: Color,
    pub stop_color: Color,
    pub stop_opacity: f32,
    pub dominant_baseline: DominantBaseline,
    pub alignment_baseline: AlignmentBaseline,
    pub baseline_source: Option<String>,
    pub clip_rule: ClipRule,
    pub shape_rendering: ShapeRendering,
    pub text_anchor: TextAnchor,
    pub vector_effect: VectorEffect,
    pub marker_start: Option<String>,
    pub marker_mid: Option<String>,
    pub marker_end: Option<String>,
    pub d: Option<String>,
    pub cx: Dimension,
    pub cy: Dimension,
    pub r: Dimension,
    pub rx: Dimension,
    pub ry: Dimension,
    pub x: Dimension,
    pub y: Dimension,

    // ── Ruby ──
    pub ruby_position: RubyPosition,
    pub ruby_align: RubyAlign,

    // ── Anchor positioning ──
    pub anchor_name: Option<String>,
    pub position_anchor: Option<String>,
    pub position_area: Option<String>,

    // ── View transitions ──
    pub view_transition_name: Option<String>,
    pub view_transition_class: Option<String>,

    // ── Scroll timeline ──
    pub scroll_timeline_name: Option<String>,
    pub scroll_timeline_axis: Option<String>,
    pub view_timeline_name: Option<String>,
    pub view_timeline_axis: Option<String>,
    pub view_timeline_inset: Option<String>,
    pub timeline_scope: Option<String>,

    // ── Misc CSS spec coverage ──
    pub page: Option<String>,
    pub zoom: f32,
    pub overlay: Option<String>,
    pub math_depth: i32,
    pub math_style: Option<String>,
    pub reading_flow: Option<String>,
    pub field_sizing: Option<String>,

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

            // Float & clear
            float: Float::default(),
            clear: Clear::default(),

            // Writing mode
            writing_mode: WritingMode::default(),
            direction: Direction::default(),
            unicode_bidi: UnicodeBidi::default(),

            // Typography
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            font_family: vec!["Inter".to_string(), "sans-serif".to_string()],
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
            vertical_align: VerticalAlign::default(),
            tab_size: 8.0,

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

            // Layout extras
            contain: Contain::default(),
            content_visibility: ContentVisibility::default(),
            aspect_ratio: AspectRatio::default(),
            object_fit: ObjectFit::default(),
            object_position_x: Dimension::Percent(50.0),
            object_position_y: Dimension::Percent(50.0),
            resize: Resize::default(),
            column_count: None,
            column_width: Dimension::Auto,
            column_gap: Dimension::Auto,
            row_gap: Dimension::Auto,

            // Alignment extras
            justify_items: JustifyItems::default(),
            justify_self: JustifySelf::default(),
            place_content: None,

            // List styling
            list_style_type: ListStyleType::default(),
            list_style_position: ListStylePosition::default(),

            // Table
            table_layout: TableLayout::default(),
            border_collapse: BorderCollapse::default(),
            border_spacing: 0.0,
            empty_cells: EmptyCells::default(),
            caption_side: CaptionSide::default(),

            // User interaction
            user_select: UserSelect::default(),
            appearance: Appearance::default(),
            scroll_behavior: ScrollBehavior::default(),
            overscroll_behavior_x: OverscrollBehavior::default(),
            overscroll_behavior_y: OverscrollBehavior::default(),

            // Will-change
            will_change: Vec::new(),

            // Effects
            transform: Vec::new(),
            transform_origin: TransformOrigin::default(),
            transform_style: TransformStyle::default(),
            transform_box: TransformBox::default(),
            perspective: Perspective::default(),
            perspective_origin: TransformOrigin::default(),
            backface_visibility: BackfaceVisibility::default(),
            filter: Vec::new(),
            backdrop_filter: Vec::new(),
            mix_blend_mode: BlendMode::SrcOver,
            isolation: Isolation::default(),
            mask: None,
            clip_path: None,
            outline: None,

            // Transitions & animations
            transition: Vec::new(),
            animation: Vec::new(),

            // Typography extras
            overflow_wrap: OverflowWrap::default(),
            hyphens: Hyphens::default(),
            text_decoration_line: None,
            text_decoration_style: None,
            text_decoration_color: None,
            text_decoration_thickness: None,
            text_decoration_skip_ink: TextDecorationSkipInk::default(),
            text_underline_offset: 0.0,
            text_underline_position: TextUnderlinePosition::default(),
            text_align_last: TextAlignLast::default(),
            text_justify: TextJustify::default(),
            text_rendering: TextRendering::default(),
            font_stretch: FontStretch::default(),
            font_kerning: FontKerning::default(),
            font_variant_caps: FontVariantCaps::default(),
            font_variant_numeric: FontVariantNumeric::default(),
            font_optical_sizing: FontOpticalSizing::default(),
            font_size_adjust: FontSizeAdjust::default(),
            font_feature_settings: None,
            font_variation_settings: None,
            line_clamp: LineClamp::default(),

            // Image rendering
            image_rendering: ImageRendering::default(),

            // Input / interaction extras
            touch_action: TouchAction::default(),
            caret_color: None,
            accent_color: None,
            color_scheme: ColorScheme::default(),
            forced_color_adjust: ForcedColorAdjust::default(),
            print_color_adjust: PrintColorAdjust::default(),

            // Scroll snap
            scroll_snap_type: ScrollSnapType::default(),
            scroll_snap_align: ScrollSnapAlign::default(),
            scroll_snap_stop: ScrollSnapStop::default(),
            scroll_padding: Sides::all(Dimension::Auto),
            scroll_margin: Sides::all(Dimension::Zero),

            // Fragmentation
            break_before: BreakValue::default(),
            break_after: BreakValue::default(),
            break_inside: BreakValue::default(),
            orphans: 2,
            widows: 2,
            box_decoration_break: BoxDecorationBreak::default(),

            // Column extras
            column_rule: ColumnRule::default(),
            column_fill: ColumnFill::default(),
            column_span: ColumnSpan::default(),

            // Background extras
            background_attachment: BackgroundAttachment::default(),
            background_clip: BackgroundClip::default(),
            background_origin: BackgroundOrigin::default(),
            background_blend_mode: BlendMode::SrcOver,
            background_position_x: Dimension::Zero,
            background_position_y: Dimension::Zero,
            background_size: None,
            background_repeat: None,
            background_image: None,

            // Logical properties
            inline_size: Dimension::Auto,
            block_size: Dimension::Auto,
            min_inline_size: Dimension::Auto,
            min_block_size: Dimension::Auto,
            max_inline_size: Dimension::None,
            max_block_size: Dimension::None,
            margin_inline_start: Dimension::Zero,
            margin_inline_end: Dimension::Zero,
            margin_block_start: Dimension::Zero,
            margin_block_end: Dimension::Zero,
            padding_inline_start: Dimension::Zero,
            padding_inline_end: Dimension::Zero,
            padding_block_start: Dimension::Zero,
            padding_block_end: Dimension::Zero,
            inset_inline_start: Dimension::Auto,
            inset_inline_end: Dimension::Auto,
            inset_block_start: Dimension::Auto,
            inset_block_end: Dimension::Auto,
            border_inline_start_width: 0.0,
            border_inline_end_width: 0.0,
            border_block_start_width: 0.0,
            border_block_end_width: 0.0,
            border_inline_start_style: BorderLineStyle::None,
            border_inline_end_style: BorderLineStyle::None,
            border_block_start_style: BorderLineStyle::None,
            border_block_end_style: BorderLineStyle::None,
            border_inline_start_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            border_inline_end_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            border_block_start_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            border_block_end_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },

            // Grid extras
            grid_column_start: GridLine::default(),
            grid_column_end: GridLine::default(),
            grid_row_start: GridLine::default(),
            grid_row_end: GridLine::default(),
            grid_template_areas: Vec::new(),

            // Content & counters
            content: None,
            counter_increment: None,
            counter_reset: None,
            counter_set: None,
            quotes: None,

            // SVG / paint order
            paint_order: PaintOrder::default(),

            // Transition longhands
            transition_property: None,
            transition_duration: None,
            transition_timing_function: None,
            transition_delay: None,
            transition_behavior: TransitionBehavior::default(),

            // Animation longhands
            animation_name: None,
            animation_duration: None,
            animation_timing_function: None,
            animation_delay: None,
            animation_iteration_count: AnimationIterationCount::default(),
            animation_direction: AnimationDirection::default(),
            animation_fill_mode: AnimationFillMode::default(),
            animation_play_state: AnimationPlayState::default(),
            animation_composition: AnimationComposition::default(),
            animation_timeline: None,

            // Motion path
            offset_path: None,
            offset_distance: Dimension::Zero,
            offset_rotate: None,
            offset_anchor: None,
            offset_position: None,

            // Individual transform
            rotate: None,
            scale: None,
            translate: None,

            // Font extras
            font_variant_alternates: FontVariantAlternates::default(),
            font_variant_east_asian: FontVariantEastAsian::default(),
            font_variant_ligatures: FontVariantLigatures::default(),
            font_variant_position: FontVariantPosition::default(),
            font_variant_emoji: FontVariantEmoji::default(),
            font_synthesis_weight: FontSynthesisWeight::default(),
            font_synthesis_style: FontSynthesisStyle::default(),
            font_synthesis_small_caps: FontSynthesisSmallCaps::default(),
            font_language_override: None,
            font_palette: None,

            // Text extras
            text_emphasis_style: None,
            text_emphasis_color: None,
            text_emphasis_position: None,
            text_orientation: TextOrientation::default(),
            text_combine_upright: TextCombineUpright::default(),
            text_wrap_mode: TextWrapMode::default(),
            text_wrap_style: TextWrapStyle::default(),
            text_box_trim: TextBoxTrim::default(),
            text_box_edge: None,
            text_size_adjust: None,
            text_spacing_trim: None,
            white_space_collapse: WhiteSpaceCollapse::default(),
            line_break: LineBreak::default(),
            hyphenate_character: None,
            hyphenate_limit_chars: None,
            hanging_punctuation: None,
            initial_letter: None,
            text_autospace: None,

            // Overflow / scroll extras
            overflow_anchor: OverflowAnchor::default(),
            overflow_clip_margin: None,
            scrollbar_width: ScrollbarWidth::default(),
            scrollbar_gutter: ScrollbarGutter::default(),
            scrollbar_color: None,

            // Containment extras
            container_type: ContainerType::default(),
            container_name: None,
            contain_intrinsic_width: Dimension::None,
            contain_intrinsic_height: Dimension::None,

            // Shape
            shape_outside: None,
            shape_margin: 0.0,
            shape_image_threshold: 0.0,

            // Border image longhands
            border_image_source: None,
            border_image_slice: None,
            border_image_width: None,
            border_image_outset: None,
            border_image_repeat: None,

            // Logical border radius
            border_start_start_radius: 0.0,
            border_start_end_radius: 0.0,
            border_end_start_radius: 0.0,
            border_end_end_radius: 0.0,

            // Mask longhands
            mask_image: None,
            mask_mode: None,
            mask_position: None,
            mask_size: None,
            mask_repeat: None,
            mask_origin: None,
            mask_clip: None,
            mask_composite: None,
            mask_type: MaskType::default(),

            // Image extras
            image_orientation: ImageOrientation::default(),

            // SVG presentation
            fill: None,
            fill_opacity: 1.0,
            fill_rule: FillRule::default(),
            stroke: None,
            stroke_width: Dimension::Px(1.0),
            stroke_dasharray: None,
            stroke_dashoffset: Dimension::Zero,
            stroke_linecap: StrokeLinecap::default(),
            stroke_linejoin: StrokeLinejoin::default(),
            stroke_miterlimit: 4.0,
            stroke_opacity: 1.0,
            color_interpolation: ColorInterpolation::default(),
            color_interpolation_filters: ColorInterpolation::LinearRGB,
            flood_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            flood_opacity: 1.0,
            lighting_color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            stop_color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            stop_opacity: 1.0,
            dominant_baseline: DominantBaseline::default(),
            alignment_baseline: AlignmentBaseline::default(),
            baseline_source: None,
            clip_rule: ClipRule::default(),
            shape_rendering: ShapeRendering::default(),
            text_anchor: TextAnchor::default(),
            vector_effect: VectorEffect::default(),
            marker_start: None,
            marker_mid: None,
            marker_end: None,
            d: None,
            cx: Dimension::Zero,
            cy: Dimension::Zero,
            r: Dimension::Zero,
            rx: Dimension::Auto,
            ry: Dimension::Auto,
            x: Dimension::Zero,
            y: Dimension::Zero,

            // Ruby
            ruby_position: RubyPosition::default(),
            ruby_align: RubyAlign::default(),

            // Anchor positioning
            anchor_name: None,
            position_anchor: None,
            position_area: None,

            // View transitions
            view_transition_name: None,
            view_transition_class: None,

            // Scroll timeline
            scroll_timeline_name: None,
            scroll_timeline_axis: None,
            view_timeline_name: None,
            view_timeline_axis: None,
            view_timeline_inset: None,
            timeline_scope: None,

            // Misc
            page: None,
            zoom: 1.0,
            overlay: None,
            math_depth: 0,
            math_style: None,
            reading_flow: None,
            field_sizing: None,

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
        self.font_stretch = parent.font_stretch;
        self.font_kerning = parent.font_kerning;
        self.font_variant_caps = parent.font_variant_caps;
        self.font_variant_numeric = parent.font_variant_numeric;
        self.font_optical_sizing = parent.font_optical_sizing;
        self.font_size_adjust = parent.font_size_adjust.clone();
        self.font_feature_settings = parent.font_feature_settings.clone();
        self.font_variation_settings = parent.font_variation_settings.clone();
        self.line_height = parent.line_height.clone();
        self.letter_spacing = parent.letter_spacing;
        self.word_spacing = parent.word_spacing;
        self.text_align = parent.text_align;
        self.text_align_last = parent.text_align_last;
        self.text_justify = parent.text_justify;
        self.text_transform = parent.text_transform;
        self.text_rendering = parent.text_rendering;
        self.white_space = parent.white_space;
        self.word_break = parent.word_break;
        self.overflow_wrap = parent.overflow_wrap;
        self.hyphens = parent.hyphens;
        self.text_indent = parent.text_indent;
        self.tab_size = parent.tab_size;
        self.text_underline_position = parent.text_underline_position;
        self.text_decoration_skip_ink = parent.text_decoration_skip_ink;
        self.line_clamp = parent.line_clamp;
        self.image_rendering = parent.image_rendering;
        // Writing mode (inherited)
        self.writing_mode = parent.writing_mode;
        self.direction = parent.direction;
        // Visibility & cursor (inherited)
        self.visibility = parent.visibility;
        self.cursor = parent.cursor;
        self.caret_color = parent.caret_color;
        // List styling (inherited)
        self.list_style_type = parent.list_style_type;
        self.list_style_position = parent.list_style_position;
        // Table (inherited)
        self.border_collapse = parent.border_collapse;
        self.border_spacing = parent.border_spacing;
        self.empty_cells = parent.empty_cells;
        self.caption_side = parent.caption_side;
        // Color scheme (inherited)
        self.color_scheme = parent.color_scheme;
        self.forced_color_adjust = parent.forced_color_adjust;
        self.print_color_adjust = parent.print_color_adjust;
        // Fragmentation (inherited)
        self.orphans = parent.orphans;
        self.widows = parent.widows;
        // Quotes (inherited)
        self.quotes = parent.quotes.clone();
        // SVG (inherited)
        self.paint_order = parent.paint_order;
        self.fill = parent.fill.clone();
        self.fill_opacity = parent.fill_opacity;
        self.fill_rule = parent.fill_rule;
        self.stroke = parent.stroke.clone();
        self.stroke_width = parent.stroke_width.clone();
        self.stroke_dasharray = parent.stroke_dasharray.clone();
        self.stroke_dashoffset = parent.stroke_dashoffset.clone();
        self.stroke_linecap = parent.stroke_linecap;
        self.stroke_linejoin = parent.stroke_linejoin;
        self.stroke_miterlimit = parent.stroke_miterlimit;
        self.stroke_opacity = parent.stroke_opacity;
        self.color_interpolation = parent.color_interpolation;
        self.dominant_baseline = parent.dominant_baseline;
        self.clip_rule = parent.clip_rule;
        self.shape_rendering = parent.shape_rendering;
        self.text_anchor = parent.text_anchor;
        self.marker_start = parent.marker_start.clone();
        self.marker_mid = parent.marker_mid.clone();
        self.marker_end = parent.marker_end.clone();
        // Font extras (inherited)
        self.font_variant_alternates = parent.font_variant_alternates;
        self.font_variant_east_asian = parent.font_variant_east_asian;
        self.font_variant_ligatures = parent.font_variant_ligatures;
        self.font_variant_position = parent.font_variant_position;
        self.font_variant_emoji = parent.font_variant_emoji;
        self.font_synthesis_weight = parent.font_synthesis_weight;
        self.font_synthesis_style = parent.font_synthesis_style;
        self.font_synthesis_small_caps = parent.font_synthesis_small_caps;
        self.font_language_override = parent.font_language_override.clone();
        self.font_palette = parent.font_palette.clone();
        // Text extras (inherited)
        self.text_emphasis_style = parent.text_emphasis_style.clone();
        self.text_emphasis_color = parent.text_emphasis_color;
        self.text_emphasis_position = parent.text_emphasis_position.clone();
        self.text_orientation = parent.text_orientation;
        self.text_combine_upright = parent.text_combine_upright;
        self.text_wrap_mode = parent.text_wrap_mode;
        self.text_wrap_style = parent.text_wrap_style;
        self.white_space_collapse = parent.white_space_collapse;
        self.line_break = parent.line_break;
        self.hyphenate_character = parent.hyphenate_character.clone();
        self.hyphenate_limit_chars = parent.hyphenate_limit_chars.clone();
        self.hanging_punctuation = parent.hanging_punctuation.clone();
        self.text_autospace = parent.text_autospace.clone();
        self.text_spacing_trim = parent.text_spacing_trim.clone();
        // Ruby (inherited)
        self.ruby_position = parent.ruby_position;
        self.ruby_align = parent.ruby_align;
        // Misc inherited
        self.math_depth = parent.math_depth;
        self.math_style = parent.math_style.clone();
        self.image_orientation = parent.image_orientation;
        // pointer-events (inherited)
        self.pointer_events = parent.pointer_events;
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
            || self.contain.paint
            || self.contain.layout
            || !self.will_change.is_empty()
            || self.content_visibility == ContentVisibility::Auto
            || self.clip_path.is_some()
            || self.mask.is_some()
            || self.transform_style == TransformStyle::Preserve3d
            || matches!(self.perspective, Perspective::Length(_))
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

    /// Is this element a table container?
    pub fn is_table(&self) -> bool {
        matches!(
            self.display,
            Display::Table
                | Display::TableRow
                | Display::TableCell
                | Display::TableRowGroup
                | Display::TableHeaderGroup
                | Display::TableFooterGroup
                | Display::TableCaption
                | Display::TableColumn
                | Display::TableColumnGroup
        )
    }

    /// Is this element a table wrapper (display: table)?
    pub fn is_table_wrapper(&self) -> bool {
        self.display == Display::Table
    }

    /// Is this element a table internal element?
    pub fn is_table_internal(&self) -> bool {
        matches!(
            self.display,
            Display::TableRow
                | Display::TableCell
                | Display::TableRowGroup
                | Display::TableHeaderGroup
                | Display::TableFooterGroup
                | Display::TableColumn
                | Display::TableColumnGroup
                | Display::TableCaption
        )
    }

    /// Does this element establish a new block formatting context?
    pub fn establishes_bfc(&self) -> bool {
        matches!(self.display, Display::FlowRoot)
            || self.is_flex_container()
            || self.is_grid_container()
            || matches!(self.position, Position::Absolute | Position::Fixed)
            || matches!(self.overflow_x, Overflow::Hidden | Overflow::Scroll | Overflow::Auto)
            || matches!(self.overflow_y, Overflow::Hidden | Overflow::Scroll | Overflow::Auto)
            || self.display == Display::InlineBlock
            || self.is_table_wrapper()
            || self.column_count.is_some()
            || self.contain.layout
    }

    /// Is this element a CSS container query container?
    pub fn is_container_query_host(&self) -> bool {
        self.container_type != ContainerType::Normal
    }

    /// Is this a list-item?
    pub fn is_list_item(&self) -> bool {
        self.display == Display::ListItem
    }

    /// Is this element a multi-column container?
    pub fn is_multicol(&self) -> bool {
        self.column_count.is_some() || !matches!(self.column_width, Dimension::Auto)
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

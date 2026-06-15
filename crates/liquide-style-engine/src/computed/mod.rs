//! ComputedStyle — the fully resolved style for a single element.

mod animation;
mod border;
mod display;
mod effects;
mod flex;
mod grid;
mod misc;
mod scroll;
mod svg;
mod typography;
mod visual;

pub use animation::*;
pub use border::*;
pub use display::*;
pub use effects::*;
pub use flex::*;
pub use grid::*;
pub use misc::*;
pub use scroll::*;
pub use svg::*;
pub use typography::*;
pub use visual::*;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use liquide_compositor::pixel::{BlendMode, Color};
pub use liquide_compositor::scene::Overflow;
use liquide_compositor::scene::{
    BackdropFilterSpec, BackgroundSpec, BorderImageSpec, BoxShadowSpec, FilterSpec, MaskSpec,
    OutlineSpec, TextDecoration, TextShadow,
};

use crate::dimension::{Corners, Dimension, EllipticalRadius, Sides, Size};

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
    pub border_radius: Corners<EllipticalRadius>,
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
    pub font_family: Arc<Vec<String>>,
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
    pub background: Vec<BackgroundSpec>,
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
    /// `list-style-image` — the marker image source (e.g. `url(...)`), or
    /// `None` for the default (no image, use `list-style-type`). Inherited.
    pub list_style_image: Option<String>,

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
            border_radius: Corners::all(EllipticalRadius::default()),
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
            font_family: Arc::new(vec!["Inter".to_string(), "sans-serif".to_string()]),
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
            background: Vec::new(),
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
            list_style_image: None,

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
            // NOTE: the logical margin/padding longhands default to `Auto`
            // (NOT `Zero`) deliberately. `resolve_logical_properties` only
            // overrides the physical longhand when the logical one is not
            // `Auto`; an `Auto` default therefore means "unset" and is skipped,
            // so an unset logical longhand never clobbers a freshly-cascaded
            // physical `padding-left`/`margin-*` back to zero on the
            // restyle_node path. This mirrors `inset_inline_*`/`inline_size`,
            // which already default to `Auto` (and is why width survived while
            // padding/margin were being zeroed). When a logical longhand IS
            // explicitly set in the cascade it carries its real value and maps
            // to the physical side as normal.
            margin_inline_start: Dimension::Auto,
            margin_inline_end: Dimension::Auto,
            margin_block_start: Dimension::Auto,
            margin_block_end: Dimension::Auto,
            padding_inline_start: Dimension::Auto,
            padding_inline_end: Dimension::Auto,
            padding_block_start: Dimension::Auto,
            padding_block_end: Dimension::Auto,
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
        self.font_family = Arc::clone(&parent.font_family);
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
        self.accent_color = parent.accent_color;
        self.text_shadow = parent.text_shadow.clone();
        // List styling (inherited)
        self.list_style_type = parent.list_style_type;
        self.list_style_position = parent.list_style_position;
        self.list_style_image = parent.list_style_image.clone();
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

    /// Does this element establish a containing block for fixed-position descendants?
    ///
    /// Per CSS Transforms §7.1, an element with a transform, perspective, filter,
    /// backdrop-filter, or contain:paint creates a containing block for all descendants
    /// (including fixed-position ones), overriding the viewport as containing block.
    pub fn establishes_fixed_containing_block(&self) -> bool {
        !self.transform.is_empty()
            || matches!(self.perspective, Perspective::Length(_))
            || !self.filter.is_empty()
            || !self.backdrop_filter.is_empty()
            || self.contain.paint
            || self
                .will_change
                .iter()
                .any(|prop| prop == "transform" || prop == "perspective" || prop == "filter")
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

    /// Is this flex container using row direction?
    pub fn is_flex_row(&self) -> bool {
        matches!(
            self.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        )
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
            || matches!(
                self.overflow_x,
                Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
            )
            || matches!(
                self.overflow_y,
                Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
            )
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

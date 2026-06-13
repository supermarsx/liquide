//! Display list — a flat list of paint commands with spatial indexing.
//!
//! A flat contiguous list of typed paint operations for recording and replay:
//! - Flat contiguous list of typed paint operations
//! - R-tree spatial index for efficient partial invalidation
//! - Push/Pop state commands for clip, transform, opacity, filters

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use liquide_compositor::geometry::Affine2D;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::property_tree::FilterOp;
use liquide_layout::Rect;
use liquide_style_engine::computed::{
    BorderLineStyle, Cursor, FontStyle, ImageOrientation, ImageRendering, Isolation, LineHeight,
    OverflowAnchor, OverscrollBehavior, ScrollBehavior, ScrollSnapAlign, ScrollSnapStop,
    ScrollSnapType, TextAlign, TextOverflow, TextTransform, TouchAction, WhiteSpace, WordBreak,
};
use liquide_style_engine::dimension::{Corners, EllipticalRadius};

/// A single paint command — draw ops produce pixels, state ops push/pop compositor state.
#[derive(Debug, Clone)]
pub enum DisplayItem {
    // ═══════════════════════════════════════════════════
    //  DRAW OPERATIONS (produce pixels)
    // ═══════════════════════════════════════════════════

    // ── Backgrounds ──
    SolidColor {
        rect: Rect,
        color: Color,
        radius: Corners<EllipticalRadius>,
    },

    /// Linear gradient fill.
    LinearGradient {
        rect: Rect,
        angle_deg: f32,
        stops: Vec<GradientStop>,
        radius: Corners<EllipticalRadius>,
    },

    /// Radial gradient fill.
    RadialGradient {
        rect: Rect,
        center_x: f32,
        center_y: f32,
        radius_x: f32,
        radius_y: f32,
        stops: Vec<GradientStop>,
    },

    /// Conic gradient fill.
    ConicGradient {
        rect: Rect,
        center_x: f32,
        center_y: f32,
        angle_deg: f32,
        stops: Vec<GradientStop>,
    },

    // ── Borders ──
    Border {
        rect: Rect,
        top: BorderEdge,
        right: BorderEdge,
        bottom: BorderEdge,
        left: BorderEdge,
        radius: Corners<EllipticalRadius>,
    },

    /// Border image (9-patch or gradient).
    BorderImage {
        rect: Rect,
        source: String,
        slice: (f32, f32, f32, f32),
        widths: (f32, f32, f32, f32),
        outset: (f32, f32, f32, f32),
        repeat_x: BorderImageRepeat,
        repeat_y: BorderImageRepeat,
        /// Whether to fill the center of the border image (9-slice fill keyword).
        fill: bool,
    },

    // ── Shadows ──
    BoxShadow {
        rect: Rect,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: Color,
        inset: bool,
        radius: Corners<EllipticalRadius>,
    },

    // ── Outline ──
    Outline {
        rect: Rect,
        width: f32,
        style: BorderLineStyle,
        color: Color,
        offset: f32,
    },

    // ── Text ──
    Text {
        rect: Rect,
        text: String,
        color: Color,
        font_size: f32,
        font_family: Arc<Vec<String>>,
        font_weight: u16,
        font_style: FontStyle,
        letter_spacing: f32,
        word_spacing: f32,
        line_height: LineHeight,
        text_align: TextAlign,
        text_transform: TextTransform,
        text_overflow: TextOverflow,
        white_space: WhiteSpace,
        word_break: WordBreak,
        text_indent: f32,
        text_decoration: Option<liquide_compositor::scene::TextDecoration>,
        text_shadows: Vec<liquide_compositor::scene::TextShadow>,
        text_emphasis: Option<TextEmphasis>,
        caret_color: Option<Color>,
    },

    /// Single line of pre-measured text (for optimized text rendering).
    TextRun {
        rect: Rect,
        text: String,
        color: Color,
        font_size: f32,
        font_family: String,
        font_weight: u16,
        baseline: f32,
    },

    // ── Images ──
    Image {
        rect: Rect,
        src: String,
        radius: Corners<EllipticalRadius>,
    },

    /// Draw scaled image with explicit fit mode.
    ImageRect {
        rect: Rect,
        src: String,
        src_rect: Option<Rect>,
        radius: Corners<EllipticalRadius>,
        fit: ImageFit,
        image_rendering: ImageRendering,
        /// EXIF auto-rotation: `FromImage` respects EXIF orientation, `None` ignores it.
        image_orientation: ImageOrientation,
    },

    // ── Icons (built-in vector icons) ──
    Icon {
        rect: Rect,
        icon_id: u32,
        color: Color,
    },

    /// Draw a filled rect (no border radius — fastest path).
    FillRect {
        rect: Rect,
        color: Color,
    },

    /// Draw a rounded rect outline (stroke).
    StrokeRoundedRect {
        rect: Rect,
        radius: Corners<EllipticalRadius>,
        color: Color,
        width: f32,
    },

    /// Draw a line.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        width: f32,
    },

    // ═══════════════════════════════════════════════════
    //  STATE OPERATIONS (push/pop compositor state)
    // ═══════════════════════════════════════════════════

    // ── Clip ──
    PushClip {
        rect: Rect,
        radius: Corners<EllipticalRadius>,
    },
    /// Clip to an arbitrary path (circle, polygon, etc.).
    PushClipPath {
        path: ClipPath,
    },
    PopClip,

    // ── Opacity ──
    PushOpacity {
        opacity: f32,
    },
    PopOpacity,

    // ── Transform ──
    /// Push a CSS transform as a composed affine matrix.
    ///
    /// The matrix includes transform-origin handling and preserves the exact
    /// composition order from the CSS `transform` property. This ensures
    /// paint and hit-test transforms match exactly.
    PushTransform {
        /// The composed 2D affine transformation matrix.
        transform: Affine2D,
    },
    PopTransform,

    // ── Blend mode ──
    PushBlendMode {
        mode: BlendMode,
    },
    PopBlendMode,

    // ── Filters ──
    /// Push a CSS filter effect (applies to everything until PopFilter).
    PushFilter {
        filters: Vec<FilterOp>,
    },
    PopFilter,

    /// Push a CSS backdrop-filter effect.
    PushBackdropFilter {
        filters: Vec<FilterOp>,
        bounds: Rect,
    },
    PopBackdropFilter,

    // ── Mask ──
    /// Push a CSS mask.
    PushMask {
        mask_image: String,
        rect: Rect,
    },
    PopMask,

    // ── Stacking context ──
    PushStackingContext {
        z_index: i32,
        isolation: Isolation,
    },
    PopStackingContext,

    // ── Save/Restore (isolated layer) ──
    SaveLayer {
        rect: Rect,
        opacity: f32,
    },
    RestoreLayer,

    // ── External surface (sandboxed app) ──
    Surface {
        rect: Rect,
        surface_id: u64,
    },

    /// Set the cursor for a hit-test region.
    SetCursor {
        rect: Rect,
        cursor: Cursor,
    },

    /// Scroll container behaviour hints for the shell input subsystem.
    ScrollContainerHints {
        rect: Rect,
        scroll_behavior: ScrollBehavior,
        overscroll_x: OverscrollBehavior,
        overscroll_y: OverscrollBehavior,
        overflow_anchor: OverflowAnchor,
        touch_action: TouchAction,
        /// Scroll padding (top, right, bottom, left) — defines snap alignment target area.
        scroll_padding: (f32, f32, f32, f32),
        /// Scroll margin (top, right, bottom, left) — defines snap area margin.
        scroll_margin: (f32, f32, f32, f32),
        /// Scroll snap type (axis + strictness).
        scroll_snap_type: ScrollSnapType,
        /// Scroll snap alignment for this container's children.
        scroll_snap_align: ScrollSnapAlign,
        /// Whether snap points are mandatory stop points.
        scroll_snap_stop: ScrollSnapStop,
    },

    /// Animation & transition property hints for the animation scheduler.
    AnimationHints {
        rect: Rect,
        animation_name: Option<String>,
        animation_duration: Option<String>,
        animation_timing_function: Option<String>,
        animation_delay: Option<String>,
        animation_iteration_count: String,
        animation_direction: String,
        animation_fill_mode: String,
        animation_play_state: String,
        transition_property: Option<String>,
        transition_duration: Option<String>,
        transition_timing_function: Option<String>,
        transition_delay: Option<String>,
    },

    /// Scroll / view timeline hints for scroll-driven animations.
    TimelineHints {
        rect: Rect,
        scroll_timeline_name: Option<String>,
        scroll_timeline_axis: Option<String>,
        view_timeline_name: Option<String>,
        view_timeline_axis: Option<String>,
        view_timeline_inset: Option<String>,
        timeline_scope: Option<String>,
    },

    /// Annotation (debug label for a region).
    Annotate {
        rect: Rect,
        label: String,
    },

    /// No-op (placeholder for alignment or deleted items).
    Noop,
}

/// Gradient color stop.
#[derive(Debug, Clone)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// Image fit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    Fill,
    Contain,
    Cover,
    ScaleDown,
    None,
}

/// Border-image repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderImageRepeat {
    Stretch,
    Repeat,
    Round,
    Space,
}

/// Clip path shapes.
#[derive(Debug, Clone)]
pub enum ClipPath {
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    RoundedRect {
        rect: Rect,
        radii: Corners<EllipticalRadius>,
    },
    Polygon(Vec<(f32, f32)>),
    Inset {
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
        radius: Corners<EllipticalRadius>,
    },
}

/// A border edge for painting.
#[derive(Debug, Clone)]
pub struct BorderEdge {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

// ─── Text Emphasis ──────────────────────────────────────────

/// How the emphasis mark is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisFill {
    Filled,
    Open,
}

/// Shape of the emphasis mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmphasisShape {
    Dot,
    Circle,
    DoubleCircle,
    Triangle,
    Sesame,
    Custom(String),
}

/// Position of emphasis marks relative to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisPosition {
    Over,
    Under,
    /// For vertical writing modes (over + right).
    OverRight,
    /// For vertical writing modes (under + left).
    UnderLeft,
}

/// Parsed text-emphasis properties.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEmphasis {
    pub fill: EmphasisFill,
    pub shape: EmphasisShape,
    pub color: Color,
    pub position: EmphasisPosition,
}

impl TextEmphasis {
    /// Parse text-emphasis from raw CSS property strings.
    ///
    /// `style` follows the CSS `text-emphasis-style` grammar:
    ///   `none | [ [ filled | open ] || [ dot | circle | double-circle | triangle | sesame ] ] | <string>`
    ///
    /// When only a fill keyword is given the default shape is `Filled → Dot`.
    /// When only a shape keyword is given the default fill is `Filled`.
    pub fn parse(
        style: &str,
        color: Option<Color>,
        position: Option<&str>,
    ) -> Option<TextEmphasis> {
        let style = style.trim();
        if style.is_empty() || style.eq_ignore_ascii_case("none") {
            return None;
        }

        let color = color.unwrap_or(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });
        let position = Self::parse_position(position);

        // Single custom character / string (quoted or single char)
        let unquoted = style
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| style.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));

        if let Some(custom) = unquoted {
            return Some(TextEmphasis {
                fill: EmphasisFill::Filled,
                shape: EmphasisShape::Custom(custom.to_string()),
                color,
                position,
            });
        }

        let mut fill: Option<EmphasisFill> = None;
        let mut shape: Option<EmphasisShape> = None;

        for token in style.split_ascii_whitespace() {
            match token.to_ascii_lowercase().as_str() {
                "filled" => fill = Some(EmphasisFill::Filled),
                "open" => fill = Some(EmphasisFill::Open),
                "dot" => shape = Some(EmphasisShape::Dot),
                "circle" => shape = Some(EmphasisShape::Circle),
                "double-circle" => shape = Some(EmphasisShape::DoubleCircle),
                "triangle" => shape = Some(EmphasisShape::Triangle),
                "sesame" => shape = Some(EmphasisShape::Sesame),
                other => {
                    // Treat as custom string (single unquoted char, per spec)
                    shape = Some(EmphasisShape::Custom(other.to_string()));
                }
            }
        }

        let fill = fill.unwrap_or(EmphasisFill::Filled);
        let shape = shape.unwrap_or(EmphasisShape::Dot);

        Some(TextEmphasis {
            fill,
            shape,
            color,
            position,
        })
    }

    fn parse_position(pos: Option<&str>) -> EmphasisPosition {
        match pos.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("under") => EmphasisPosition::Under,
            Some("over right") | Some("over-right") => EmphasisPosition::OverRight,
            Some("under left") | Some("under-left") => EmphasisPosition::UnderLeft,
            _ => EmphasisPosition::Over,
        }
    }
}

impl Default for BorderEdge {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Display-list identity / diff / merge metadata layer
//
//  STAGING STATUS (t49-e4-09): this metadata-only diff/merge layer is NOT YET
//  driven by the live runtime. No production caller currently builds
//  `DisplayItemMetadata`, runs `diff_display_list_metadata`, or consumes
//  `can_merge_display_items` / `DisplayItemMergeClass` to actually skip or
//  batch paint work. The renderer still repaints from damage tiles, not from
//  these summaries. It exists as a self-contained, unit-tested vocabulary so a
//  future incremental-paint consumer can adopt it without a redesign.
//
//  SOUNDNESS CONTRACT (why this is safe to stage rather than half-wire): every
//  helper here is deliberately CONSERVATIVE — when in doubt it reports "must
//  repaint", never "safe to retain". Concretely:
//    * Missing or duplicate identities  -> full repaint.
//    * Unknown content revisions        -> treated as changed (repaint).
//    * State / non-pixel-producing items -> full repaint when touched.
//    * Merge predicates                 -> only fire when provably
//      pixel-neutral (equal opaque colors, or disjoint rects).
//  This guarantees a future consumer can NEVER produce an empty/garbage frame
//  by trusting a stale "retain"; the worst case is an over-broad repaint. That
//  conservatism is intentional and load-bearing — do not relax a helper toward
//  "retain" without a real consumer and pixel-parity proof.
// ════════════════════════════════════════════════════════════════════════

/// Stable identity for one emitted display item.
///
/// The paint recorder is expected to derive `source_id` from a stable upstream
/// object, such as a DOM/layout node, and `local_id` from that object's paint
/// slot. List indices are intentionally kept separate because insertions before
/// an item should not change its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayItemIdentity {
    /// Stable upstream source identifier.
    pub source_id: u64,
    /// Stable per-source display item slot.
    pub local_id: u32,
}

impl DisplayItemIdentity {
    /// Create a stable display item identity.
    #[must_use]
    pub const fn new(source_id: u64, local_id: u32) -> Self {
        Self {
            source_id,
            local_id,
        }
    }
}

/// Coarse display item kind used by metadata-only diffing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayItemKind {
    SolidColor,
    LinearGradient,
    RadialGradient,
    ConicGradient,
    Border,
    BorderImage,
    BoxShadow,
    Outline,
    Text,
    TextRun,
    Image,
    ImageRect,
    Icon,
    FillRect,
    StrokeRoundedRect,
    Line,
    PushClip,
    PushClipPath,
    PopClip,
    PushOpacity,
    PopOpacity,
    PushTransform,
    PopTransform,
    PushBlendMode,
    PopBlendMode,
    PushFilter,
    PopFilter,
    PushBackdropFilter,
    PopBackdropFilter,
    PushMask,
    PopMask,
    PushStackingContext,
    PopStackingContext,
    SaveLayer,
    RestoreLayer,
    Surface,
    SetCursor,
    ScrollContainerHints,
    AnimationHints,
    TimelineHints,
    Annotate,
    Noop,
}

impl DisplayItemKind {
    /// Classify a display item by enum variant.
    #[must_use]
    pub fn of(item: &DisplayItem) -> Self {
        match item {
            DisplayItem::SolidColor { .. } => Self::SolidColor,
            DisplayItem::LinearGradient { .. } => Self::LinearGradient,
            DisplayItem::RadialGradient { .. } => Self::RadialGradient,
            DisplayItem::ConicGradient { .. } => Self::ConicGradient,
            DisplayItem::Border { .. } => Self::Border,
            DisplayItem::BorderImage { .. } => Self::BorderImage,
            DisplayItem::BoxShadow { .. } => Self::BoxShadow,
            DisplayItem::Outline { .. } => Self::Outline,
            DisplayItem::Text { .. } => Self::Text,
            DisplayItem::TextRun { .. } => Self::TextRun,
            DisplayItem::Image { .. } => Self::Image,
            DisplayItem::ImageRect { .. } => Self::ImageRect,
            DisplayItem::Icon { .. } => Self::Icon,
            DisplayItem::FillRect { .. } => Self::FillRect,
            DisplayItem::StrokeRoundedRect { .. } => Self::StrokeRoundedRect,
            DisplayItem::Line { .. } => Self::Line,
            DisplayItem::PushClip { .. } => Self::PushClip,
            DisplayItem::PushClipPath { .. } => Self::PushClipPath,
            DisplayItem::PopClip => Self::PopClip,
            DisplayItem::PushOpacity { .. } => Self::PushOpacity,
            DisplayItem::PopOpacity => Self::PopOpacity,
            DisplayItem::PushTransform { .. } => Self::PushTransform,
            DisplayItem::PopTransform => Self::PopTransform,
            DisplayItem::PushBlendMode { .. } => Self::PushBlendMode,
            DisplayItem::PopBlendMode => Self::PopBlendMode,
            DisplayItem::PushFilter { .. } => Self::PushFilter,
            DisplayItem::PopFilter => Self::PopFilter,
            DisplayItem::PushBackdropFilter { .. } => Self::PushBackdropFilter,
            DisplayItem::PopBackdropFilter => Self::PopBackdropFilter,
            DisplayItem::PushMask { .. } => Self::PushMask,
            DisplayItem::PopMask => Self::PopMask,
            DisplayItem::PushStackingContext { .. } => Self::PushStackingContext,
            DisplayItem::PopStackingContext => Self::PopStackingContext,
            DisplayItem::SaveLayer { .. } => Self::SaveLayer,
            DisplayItem::RestoreLayer => Self::RestoreLayer,
            DisplayItem::Surface { .. } => Self::Surface,
            DisplayItem::SetCursor { .. } => Self::SetCursor,
            DisplayItem::ScrollContainerHints { .. } => Self::ScrollContainerHints,
            DisplayItem::AnimationHints { .. } => Self::AnimationHints,
            DisplayItem::TimelineHints { .. } => Self::TimelineHints,
            DisplayItem::Annotate { .. } => Self::Annotate,
            DisplayItem::Noop => Self::Noop,
        }
    }

    /// Whether this item kind can directly affect pixels under current paint semantics.
    #[must_use]
    pub fn is_pixel_producing(self) -> bool {
        matches!(
            self,
            Self::SolidColor
                | Self::LinearGradient
                | Self::RadialGradient
                | Self::ConicGradient
                | Self::Border
                | Self::BorderImage
                | Self::BoxShadow
                | Self::Outline
                | Self::Text
                | Self::TextRun
                | Self::Image
                | Self::ImageRect
                | Self::Icon
                | Self::FillRect
                | Self::StrokeRoundedRect
                | Self::Line
                | Self::Surface
        )
    }

    /// Whether this item kind mutates paint/compositor state for following items.
    #[must_use]
    pub fn is_stateful(self) -> bool {
        matches!(
            self,
            Self::PushClip
                | Self::PushClipPath
                | Self::PopClip
                | Self::PushOpacity
                | Self::PopOpacity
                | Self::PushTransform
                | Self::PopTransform
                | Self::PushBlendMode
                | Self::PopBlendMode
                | Self::PushFilter
                | Self::PopFilter
                | Self::PushBackdropFilter
                | Self::PopBackdropFilter
                | Self::PushMask
                | Self::PopMask
                | Self::PushStackingContext
                | Self::PopStackingContext
                | Self::SaveLayer
                | Self::RestoreLayer
        )
    }
}

/// Conservative command merge class for future display command batching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayItemMergeClass {
    /// Axis-aligned solid fill commands with identical color can be merged.
    SolidFill,
    /// No safe merge rule is known for this item.
    NonMergeable,
}

impl DisplayItemMergeClass {
    /// Whether this class represents an item eligible for merge checks.
    #[must_use]
    pub fn is_mergeable(self) -> bool {
        matches!(self, Self::SolidFill)
    }
}

impl DisplayItem {
    /// Return the coarse display item kind.
    #[must_use]
    pub fn kind(&self) -> DisplayItemKind {
        DisplayItemKind::of(self)
    }

    /// Return conservative spatial bounds, if known.
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        item_bounds(self)
    }

    /// Return the conservative merge class for this item.
    #[must_use]
    pub fn merge_class(&self) -> DisplayItemMergeClass {
        display_item_merge_class(self)
    }

    /// Return true if two adjacent items can be merged without changing pixels.
    #[must_use]
    pub fn can_merge_with(&self, next: &DisplayItem) -> bool {
        can_merge_display_items(self, next)
    }
}

/// Metadata snapshot for one display item.
///
/// `content_revision` is optional by design. A missing revision is treated as
/// changed by the diff helper, so early callers can opt in to identity metadata
/// without risking stale pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayItemMetadata {
    /// Stable item identity. `None` means the item is intentionally unstable.
    pub identity: Option<DisplayItemIdentity>,
    /// Current index in the display list.
    pub index: usize,
    /// Coarse item kind.
    pub kind: DisplayItemKind,
    /// Conservative spatial bounds, if known.
    pub bounds: Option<Rect>,
    /// Conservative command merge class.
    pub merge_class: DisplayItemMergeClass,
    /// Caller-provided content revision or fingerprint.
    pub content_revision: Option<u64>,
}

impl DisplayItemMetadata {
    /// Create metadata for a stable item without a known content revision.
    #[must_use]
    pub fn new(identity: DisplayItemIdentity, index: usize, item: &DisplayItem) -> Self {
        Self::from_item(Some(identity), index, item)
    }

    /// Create metadata for an item whose stable identity is not yet available.
    #[must_use]
    pub fn unstable(index: usize, item: &DisplayItem) -> Self {
        Self::from_item(None, index, item)
    }

    /// Create metadata from an optional identity.
    #[must_use]
    pub fn from_item(
        identity: Option<DisplayItemIdentity>,
        index: usize,
        item: &DisplayItem,
    ) -> Self {
        Self {
            identity,
            index,
            kind: item.kind(),
            bounds: item.bounds(),
            merge_class: item.merge_class(),
            content_revision: None,
        }
    }

    /// Attach a caller-provided content revision or fingerprint.
    #[must_use]
    pub fn with_content_revision(mut self, revision: u64) -> Self {
        self.content_revision = Some(revision);
        self
    }

    fn can_retain_without_repaint(&self, current: &Self) -> bool {
        self.kind == current.kind
            && self.bounds == current.bounds
            && self.merge_class == current.merge_class
            && matches!(
                (self.content_revision, current.content_revision),
                (Some(previous), Some(next)) if previous == next
            )
    }

    fn requires_full_repaint_when_changed(&self) -> bool {
        !self.kind.is_pixel_producing() || self.bounds.is_none()
    }
}

/// Conservative summary of a display-list metadata diff.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayListDiffSummary {
    /// Number of items in the previous metadata snapshot.
    pub previous_items: usize,
    /// Number of items in the current metadata snapshot.
    pub current_items: usize,
    /// Items retained without repaint.
    pub retained_items: usize,
    /// Stable items whose kind, bounds, merge class, or content revision changed.
    pub changed_items: usize,
    /// Current items with no matching previous identity.
    pub added_items: usize,
    /// Previous items with no matching current identity.
    pub removed_items: usize,
    /// Stable items whose list index changed.
    pub moved_items: usize,
    /// Missing or duplicate identities encountered while diffing.
    pub unstable_identity_count: usize,
    /// Regions that must be repainted when a bounded partial repaint is possible.
    pub repaint_bounds: Vec<Rect>,
    /// Whether callers must ignore `repaint_bounds` and repaint the whole list.
    pub full_repaint_required: bool,
}

/// Coarse repaint strategy selected by a display-list diff summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayListRepaintStrategy {
    /// No repaint-affecting metadata changed.
    Retain,
    /// Repaint only the bounded regions in the diff summary.
    Partial,
    /// Repaint the full display list because state or identity metadata is unsafe.
    Full,
}

impl DisplayListRepaintStrategy {
    /// Stable metric label for this strategy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retain => "retain",
            Self::Partial => "partial",
            Self::Full => "full",
        }
    }
}

impl DisplayListDiffSummary {
    fn new(previous_items: usize, current_items: usize) -> Self {
        Self {
            previous_items,
            current_items,
            ..Self::default()
        }
    }

    /// Return true when the diff found no repaint-affecting changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changed_items == 0
            && self.added_items == 0
            && self.removed_items == 0
            && self.moved_items == 0
            && self.unstable_identity_count == 0
            && !self.full_repaint_required
    }

    /// Return true when some paint work is required.
    #[must_use]
    pub fn requires_repaint(&self) -> bool {
        !self.is_empty()
    }

    /// Return true when the diff cannot be safely narrowed to bounded repaint regions.
    #[must_use]
    pub fn requires_full_repaint(&self) -> bool {
        self.full_repaint_required
    }

    /// Return the coarse repaint strategy implied by this diff summary.
    #[must_use]
    pub fn repaint_strategy(&self) -> DisplayListRepaintStrategy {
        if self.requires_full_repaint() {
            DisplayListRepaintStrategy::Full
        } else if self.requires_repaint() {
            DisplayListRepaintStrategy::Partial
        } else {
            DisplayListRepaintStrategy::Retain
        }
    }

    /// Count items that were not retained and may require downstream work.
    #[must_use]
    pub fn invalidated_items(&self) -> usize {
        self.changed_items + self.added_items + self.removed_items + self.moved_items
    }

    /// Number of bounded repaint regions retained in this summary.
    #[must_use]
    pub fn repaint_region_count(&self) -> usize {
        self.repaint_bounds.len()
    }

    /// Fraction of current items retained by stable metadata.
    #[must_use]
    pub fn retained_item_ratio(&self) -> f32 {
        if self.current_items == 0 {
            1.0
        } else {
            self.retained_items as f32 / self.current_items as f32
        }
    }

    fn record_item_repaint(&mut self, item: &DisplayItemMetadata) {
        if item.requires_full_repaint_when_changed() {
            self.full_repaint_required = true;
        }

        if let Some(bounds) = item.bounds {
            if !self.repaint_bounds.contains(&bounds) {
                self.repaint_bounds.push(bounds);
            }
        }
    }

    fn record_pair_repaint(
        &mut self,
        previous: &DisplayItemMetadata,
        current: &DisplayItemMetadata,
    ) {
        self.record_item_repaint(previous);
        self.record_item_repaint(current);
    }
}

/// Diff two display-list metadata snapshots by stable identity.
///
/// Missing identities, duplicate identities, state-item changes, and unknown
/// content revisions all fall back conservatively instead of treating an item as
/// retained.
#[must_use]
pub fn diff_display_list_metadata(
    previous: &[DisplayItemMetadata],
    current: &[DisplayItemMetadata],
) -> DisplayListDiffSummary {
    let mut summary = DisplayListDiffSummary::new(previous.len(), current.len());
    let unstable_identity_count =
        unstable_identity_count(previous) + unstable_identity_count(current);
    if unstable_identity_count > 0 {
        summary.unstable_identity_count = unstable_identity_count;
        summary.changed_items = current.len();
        summary.removed_items = previous.len();
        summary.full_repaint_required = true;
        for item in previous.iter().chain(current.iter()) {
            summary.record_item_repaint(item);
        }
        return summary;
    }

    let previous_by_identity: HashMap<DisplayItemIdentity, &DisplayItemMetadata> = previous
        .iter()
        .filter_map(|item| item.identity.map(|identity| (identity, item)))
        .collect();
    let mut seen_current = HashSet::with_capacity(current.len());

    for current_item in current {
        let Some(identity) = current_item.identity else {
            continue;
        };
        seen_current.insert(identity);

        let Some(previous_item) = previous_by_identity.get(&identity).copied() else {
            summary.added_items += 1;
            summary.record_item_repaint(current_item);
            continue;
        };

        let metadata_changed = !previous_item.can_retain_without_repaint(current_item);
        let moved = previous_item.index != current_item.index;

        if metadata_changed {
            summary.changed_items += 1;
            summary.record_pair_repaint(previous_item, current_item);
        }
        if moved {
            summary.moved_items += 1;
            summary.record_pair_repaint(previous_item, current_item);
        }
        if !metadata_changed && !moved {
            summary.retained_items += 1;
        }
    }

    for previous_item in previous {
        if let Some(identity) = previous_item.identity {
            if !seen_current.contains(&identity) {
                summary.removed_items += 1;
                summary.record_item_repaint(previous_item);
            }
        }
    }

    summary
}

/// Return the conservative merge class for a display item.
#[must_use]
pub fn display_item_merge_class(item: &DisplayItem) -> DisplayItemMergeClass {
    match item {
        DisplayItem::FillRect { rect, .. } if rect_is_mergeable(*rect) => {
            DisplayItemMergeClass::SolidFill
        }
        DisplayItem::SolidColor { rect, radius, .. }
            if rect_is_mergeable(*rect) && corners_are_zero(radius) =>
        {
            DisplayItemMergeClass::SolidFill
        }
        _ => DisplayItemMergeClass::NonMergeable,
    }
}

/// Return true if two adjacent display items can be merged without changing pixels.
///
/// Two solid-fill draws may only be coalesced when the result is provably
/// pixel-identical. The colours must be equal AND the merge must not change
/// blending in any pixel both rects cover. With a translucent colour, the
/// overlap region of two adjacent draws is composited twice, so merging them
/// into a single draw would lighten that region. We therefore require the
/// shared colour to be either:
///   * fully opaque (`a == 255`) — double-covering an opaque pixel is a no-op,
///     so any overlap is harmless; or
///   * non-overlapping — when the rects are disjoint there is no
///     double-blended region, so even a translucent colour merges cleanly.
/// (t49-e4-10: prior versions ignored alpha and overlap entirely.)
#[must_use]
pub fn can_merge_display_items(previous: &DisplayItem, current: &DisplayItem) -> bool {
    match (previous, current) {
        (
            DisplayItem::FillRect {
                rect: previous_rect,
                color: previous_color,
            },
            DisplayItem::FillRect {
                rect: current_rect,
                color: current_color,
            },
        ) => {
            previous_color == current_color
                && rect_is_mergeable(*previous_rect)
                && rect_is_mergeable(*current_rect)
                && solid_overlap_is_pixel_neutral(*previous_color, *previous_rect, *current_rect)
        }
        (
            DisplayItem::SolidColor {
                rect: previous_rect,
                color: previous_color,
                radius: previous_radius,
            },
            DisplayItem::SolidColor {
                rect: current_rect,
                color: current_color,
                radius: current_radius,
            },
        ) => {
            previous_color == current_color
                && rect_is_mergeable(*previous_rect)
                && rect_is_mergeable(*current_rect)
                && corners_are_zero(previous_radius)
                && corners_are_zero(current_radius)
                && solid_overlap_is_pixel_neutral(*previous_color, *previous_rect, *current_rect)
        }
        (
            DisplayItem::FillRect {
                rect: previous_rect,
                color: previous_color,
            },
            DisplayItem::SolidColor {
                rect: current_rect,
                color: current_color,
                radius: current_radius,
            },
        )
        | (
            DisplayItem::SolidColor {
                rect: current_rect,
                color: current_color,
                radius: current_radius,
            },
            DisplayItem::FillRect {
                rect: previous_rect,
                color: previous_color,
            },
        ) => {
            previous_color == current_color
                && rect_is_mergeable(*previous_rect)
                && rect_is_mergeable(*current_rect)
                && corners_are_zero(current_radius)
                && solid_overlap_is_pixel_neutral(*previous_color, *previous_rect, *current_rect)
        }
        _ => false,
    }
}

/// Whether merging two equal-colour solid fills is guaranteed pixel-neutral.
///
/// Safe when the colour is fully opaque (overlap is idempotent) or when the
/// two rects do not overlap (no pixel is composited twice). See
/// `can_merge_display_items` for the full rationale (t49-e4-10).
fn solid_overlap_is_pixel_neutral(color: Color, a: Rect, b: Rect) -> bool {
    color.a == 255 || !rects_intersect(&a, &b)
}

fn unstable_identity_count(items: &[DisplayItemMetadata]) -> usize {
    let mut identities = HashSet::with_capacity(items.len());
    let mut unstable = 0;

    for item in items {
        match item.identity {
            Some(identity) if identities.insert(identity) => {}
            Some(_) | None => unstable += 1,
        }
    }

    unstable
}

fn rect_is_mergeable(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0
}

fn corners_are_zero(radius: &Corners<EllipticalRadius>) -> bool {
    radius.top_left.is_zero()
        && radius.top_right.is_zero()
        && radius.bottom_right.is_zero()
        && radius.bottom_left.is_zero()
}

/// An ordered list of paint commands with optional spatial indexing.
#[derive(Debug, Clone)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    /// Spatial index: each entry is (item_index, bounding_rect).
    /// Built on demand via `build_spatial_index()`.
    spatial_index: Vec<SpatialEntry>,
    /// Whether the spatial index is up-to-date.
    spatial_dirty: bool,
}

/// Entry in the spatial index.
#[derive(Debug, Clone)]
struct SpatialEntry {
    index: usize,
    bounds: Rect,
}

impl DisplayList {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            spatial_index: Vec::new(),
            spatial_dirty: true,
        }
    }

    /// Create a display list from an existing owned item buffer.
    pub fn from_items(items: Vec<DisplayItem>) -> Self {
        Self {
            items,
            spatial_index: Vec::new(),
            spatial_dirty: true,
        }
    }

    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            spatial_index: Vec::new(),
            spatial_dirty: true,
        }
    }

    pub fn push(&mut self, item: DisplayItem) {
        self.spatial_dirty = true;
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.spatial_index.clear();
        self.spatial_dirty = true;
    }

    /// Build (or rebuild) the spatial index for efficient region queries.
    pub fn build_spatial_index(&mut self) {
        self.spatial_index.clear();
        for (i, item) in self.items.iter().enumerate() {
            if let Some(bounds) = item_bounds(item) {
                self.spatial_index.push(SpatialEntry { index: i, bounds });
            }
        }
        self.spatial_dirty = false;
    }

    /// Query all display items that intersect the given region.
    /// Returns indices into `self.items`.
    pub fn query_region(&mut self, region: &Rect) -> Vec<usize> {
        if self.spatial_dirty {
            self.build_spatial_index();
        }
        self.spatial_index
            .iter()
            .filter(|entry| rects_intersect(&entry.bounds, region))
            .map(|entry| entry.index)
            .collect()
    }

    /// Total number of pixel-producing draw operations.
    ///
    /// "Draw op" means an item whose kind is pixel-producing per
    /// [`DisplayItemKind::is_pixel_producing`] — the authoritative classifier.
    /// State Push/Pop ops, non-painting hints (cursor, scroll, animation,
    /// timeline, annotate), and `Noop` are excluded. (t49-e4-14: this used to
    /// be keyed on "has spatial bounds", which over-counted hint/clip items
    /// that carry a rect but produce no pixels.)
    pub fn draw_op_count(&self) -> usize {
        self.items.iter().filter(|item| is_draw_op(item)).count()
    }

    /// Total number of non-draw operations (state Push/Pop ops, hints, `Noop`).
    pub fn state_op_count(&self) -> usize {
        self.items.len() - self.draw_op_count()
    }

    /// Append all items from another display list (by reference, cloning items).
    pub fn extend(&mut self, other: &DisplayList) {
        self.spatial_dirty = true;
        self.items.extend(other.items.iter().cloned());
    }

    /// Append all items from another display list by consuming it (no clones).
    pub fn extend_owned(&mut self, other: DisplayList) {
        self.spatial_dirty = true;
        self.items.extend(other.items);
    }

    /// Build item metadata using caller-supplied stable identities.
    ///
    /// If fewer identities than items are supplied, the remaining items are
    /// marked unstable and any diff involving them will require full repaint.
    pub fn item_metadata_with_identities<IdentityIter>(
        &self,
        identities: IdentityIter,
    ) -> Vec<DisplayItemMetadata>
    where
        IdentityIter: IntoIterator<Item = DisplayItemIdentity>,
    {
        let mut identities = identities.into_iter();
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| DisplayItemMetadata::from_item(identities.next(), index, item))
            .collect()
    }
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the bounding rect of a display item (None for state ops).
fn item_bounds(item: &DisplayItem) -> Option<Rect> {
    match item {
        DisplayItem::SolidColor { rect, .. }
        | DisplayItem::LinearGradient { rect, .. }
        | DisplayItem::RadialGradient { rect, .. }
        | DisplayItem::ConicGradient { rect, .. }
        | DisplayItem::Border { rect, .. }
        | DisplayItem::BorderImage { rect, .. }
        | DisplayItem::Text { rect, .. }
        | DisplayItem::TextRun { rect, .. }
        | DisplayItem::Image { rect, .. }
        | DisplayItem::ImageRect { rect, .. }
        | DisplayItem::Icon { rect, .. }
        | DisplayItem::FillRect { rect, .. }
        | DisplayItem::StrokeRoundedRect { rect, .. }
        | DisplayItem::Surface { rect, .. }
        | DisplayItem::SetCursor { rect, .. }
        | DisplayItem::ScrollContainerHints { rect, .. }
        | DisplayItem::AnimationHints { rect, .. }
        | DisplayItem::TimelineHints { rect, .. }
        | DisplayItem::PushClip { rect, .. }
        | DisplayItem::PushBackdropFilter { bounds: rect, .. }
        | DisplayItem::PushMask { rect, .. }
        | DisplayItem::SaveLayer { rect, .. }
        | DisplayItem::Annotate { rect, .. } => Some(*rect),

        DisplayItem::BoxShadow {
            rect,
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            inset,
            ..
        } => {
            if *inset {
                // Inset shadows are clipped to the element's border box
                Some(*rect)
            } else {
                // Outer shadow extends by offset + blur + spread
                let expand = *blur_radius + spread_radius.max(0.0);
                let shadow_x = rect.x + offset_x - expand;
                let shadow_y = rect.y + offset_y - expand;
                let shadow_r = rect.x + rect.width + offset_x + expand;
                let shadow_b = rect.y + rect.height + offset_y + expand;
                // Union of element rect and shadow rect
                let min_x = rect.x.min(shadow_x);
                let min_y = rect.y.min(shadow_y);
                let max_x = (rect.x + rect.width).max(shadow_r);
                let max_y = (rect.y + rect.height).max(shadow_b);
                Some(Rect {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x,
                    height: max_y - min_y,
                })
            }
        }

        DisplayItem::Outline {
            rect,
            width,
            offset,
            ..
        } => {
            // Outline is drawn outside the border box, offset further by `offset`
            let expand = *width + offset.max(0.0);
            Some(Rect {
                x: rect.x - expand,
                y: rect.y - expand,
                width: rect.width + expand * 2.0,
                height: rect.height + expand * 2.0,
            })
        }

        DisplayItem::Line {
            x1,
            y1,
            x2,
            y2,
            width,
            ..
        } => {
            let half_w = width / 2.0;
            let min_x = x1.min(*x2) - half_w;
            let min_y = y1.min(*y2) - half_w;
            let max_x = x1.max(*x2) + half_w;
            let max_y = y1.max(*y2) + half_w;
            Some(Rect {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            })
        }

        // State ops have no spatial bounds
        DisplayItem::PopClip
        | DisplayItem::PushClipPath { .. }
        | DisplayItem::PushOpacity { .. }
        | DisplayItem::PopOpacity
        | DisplayItem::PushTransform { .. }
        | DisplayItem::PopTransform
        | DisplayItem::PushBlendMode { .. }
        | DisplayItem::PopBlendMode
        | DisplayItem::PushFilter { .. }
        | DisplayItem::PopFilter
        | DisplayItem::PopBackdropFilter
        | DisplayItem::PopMask
        | DisplayItem::PushStackingContext { .. }
        | DisplayItem::PopStackingContext
        | DisplayItem::RestoreLayer
        | DisplayItem::Noop => None,
    }
}

/// Check if a display item is a pixel-producing draw operation (vs. state op
/// or non-painting hint).
///
/// Keyed on [`DisplayItemKind::is_pixel_producing`] so it stays consistent with
/// the metadata diff layer. Note this is deliberately NOT `item_bounds().is_some()`:
/// several hint/clip items (`SetCursor`, `ScrollContainerHints`, `AnimationHints`,
/// `TimelineHints`, `PushClip`, `PushBackdropFilter`, `PushMask`, `SaveLayer`,
/// `Annotate`) carry a spatial rect but emit no pixels (t49-e4-14).
fn is_draw_op(item: &DisplayItem) -> bool {
    DisplayItemKind::of(item).is_pixel_producing()
}

/// AABB intersection test.
fn rects_intersect(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn test_color(red: u8, green: u8, blue: u8) -> Color {
        Color {
            r: red,
            g: green,
            b: blue,
            a: 255,
        }
    }

    fn fill_item(x: f32, color: Color) -> DisplayItem {
        DisplayItem::FillRect {
            rect: test_rect(x, 0.0, 10.0, 10.0),
            color,
        }
    }

    fn solid_item(x: f32, color: Color, radius: f32) -> DisplayItem {
        DisplayItem::SolidColor {
            rect: test_rect(x, 0.0, 10.0, 10.0),
            color,
            radius: Corners::all(EllipticalRadius::from(radius)),
        }
    }

    fn stable_metadata(
        source_id: u64,
        local_id: u32,
        index: usize,
        item: &DisplayItem,
        revision: u64,
    ) -> DisplayItemMetadata {
        DisplayItemMetadata::new(DisplayItemIdentity::new(source_id, local_id), index, item)
            .with_content_revision(revision)
    }

    #[test]
    fn display_list_basics() {
        let mut dl = DisplayList::new();
        assert!(dl.is_empty());

        dl.push(DisplayItem::FillRect {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            color: Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
        });
        assert_eq!(dl.len(), 1);
        assert_eq!(dl.draw_op_count(), 1);
        assert_eq!(dl.state_op_count(), 0);
    }

    #[test]
    fn spatial_query() {
        let mut dl = DisplayList::new();
        dl.push(DisplayItem::FillRect {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            color: Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
        });
        dl.push(DisplayItem::FillRect {
            rect: Rect {
                x: 100.0,
                y: 100.0,
                width: 50.0,
                height: 50.0,
            },
            color: Color {
                r: 0,
                g: 255,
                b: 0,
                a: 255,
            },
        });

        // Query top-left region
        let hits = dl.query_region(&Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 60.0,
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], 0);

        // Query bottom-right region
        let hits = dl.query_region(&Rect {
            x: 90.0,
            y: 90.0,
            width: 70.0,
            height: 70.0,
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], 1);

        // Query everything
        let hits = dl.query_region(&Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        });
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn state_ops_not_in_spatial_index() {
        let mut dl = DisplayList::new();
        dl.push(DisplayItem::PushOpacity { opacity: 0.5 });
        dl.push(DisplayItem::FillRect {
            rect: Rect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        });
        dl.push(DisplayItem::PopOpacity);

        assert_eq!(dl.draw_op_count(), 1);
        assert_eq!(dl.state_op_count(), 2);

        let hits = dl.query_region(&Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        });
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn display_item_metadata_with_identity_retains_known_revision() {
        let item = fill_item(0.0, test_color(255, 0, 0));
        let previous = vec![stable_metadata(7, 0, 0, &item, 11)];
        let current = vec![stable_metadata(7, 0, 0, &item, 11)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert!(summary.is_empty());
        assert_eq!(summary.retained_items, 1);
        assert_eq!(summary.repaint_bounds.len(), 0);
        assert_eq!(
            summary.repaint_strategy(),
            DisplayListRepaintStrategy::Retain
        );
        assert_eq!(summary.invalidated_items(), 0);
        assert_eq!(summary.repaint_region_count(), 0);
        assert_eq!(summary.retained_item_ratio(), 1.0);
    }

    #[test]
    fn display_item_unknown_revision_repaints_conservatively() {
        let item = fill_item(0.0, test_color(255, 0, 0));
        let identity = DisplayItemIdentity::new(7, 0);
        let previous = vec![DisplayItemMetadata::new(identity, 0, &item)];
        let current = vec![DisplayItemMetadata::new(identity, 0, &item)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert!(summary.requires_repaint());
        assert!(!summary.requires_full_repaint());
        assert_eq!(summary.changed_items, 1);
        assert_eq!(
            summary.repaint_bounds,
            vec![test_rect(0.0, 0.0, 10.0, 10.0)]
        );
        assert_eq!(
            summary.repaint_strategy(),
            DisplayListRepaintStrategy::Partial
        );
        assert_eq!(summary.invalidated_items(), 1);
        assert_eq!(summary.repaint_region_count(), 1);
        assert_eq!(summary.retained_item_ratio(), 0.0);
    }

    #[test]
    fn display_item_changed_revision_repaints_previous_and_current_bounds() {
        let previous_item = fill_item(0.0, test_color(255, 0, 0));
        let current_item = fill_item(20.0, test_color(255, 0, 0));
        let previous = vec![stable_metadata(7, 0, 0, &previous_item, 11)];
        let current = vec![stable_metadata(7, 0, 0, &current_item, 12)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert_eq!(summary.changed_items, 1);
        assert!(!summary.requires_full_repaint());
        assert_eq!(summary.repaint_bounds.len(), 2);
        assert!(
            summary
                .repaint_bounds
                .contains(&test_rect(0.0, 0.0, 10.0, 10.0))
        );
        assert!(
            summary
                .repaint_bounds
                .contains(&test_rect(20.0, 0.0, 10.0, 10.0))
        );
    }

    #[test]
    fn display_item_missing_identity_requires_full_repaint() {
        let item = fill_item(0.0, test_color(255, 0, 0));
        let previous = vec![stable_metadata(7, 0, 0, &item, 11)];
        let current = vec![DisplayItemMetadata::unstable(0, &item).with_content_revision(11)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert!(summary.requires_full_repaint());
        assert_eq!(summary.unstable_identity_count, 1);
        assert_eq!(summary.repaint_strategy(), DisplayListRepaintStrategy::Full);
        assert_eq!(summary.repaint_strategy().as_str(), "full");
    }

    #[test]
    fn display_item_duplicate_identity_requires_full_repaint() {
        let item = fill_item(0.0, test_color(255, 0, 0));
        let duplicate = DisplayItemIdentity::new(7, 0);
        let previous = vec![DisplayItemMetadata::new(duplicate, 0, &item).with_content_revision(1)];
        let current = vec![
            DisplayItemMetadata::new(duplicate, 0, &item).with_content_revision(1),
            DisplayItemMetadata::new(duplicate, 1, &item).with_content_revision(1),
        ];

        let summary = diff_display_list_metadata(&previous, &current);

        assert!(summary.requires_full_repaint());
        assert_eq!(summary.unstable_identity_count, 1);
    }

    #[test]
    fn display_item_move_repaints_bounds_without_marking_content_changed() {
        let item = fill_item(0.0, test_color(255, 0, 0));
        let previous = vec![stable_metadata(7, 0, 0, &item, 11)];
        let current = vec![stable_metadata(7, 0, 2, &item, 11)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert_eq!(summary.changed_items, 0);
        assert_eq!(summary.moved_items, 1);
        assert!(!summary.requires_full_repaint());
        assert_eq!(
            summary.repaint_bounds,
            vec![test_rect(0.0, 0.0, 10.0, 10.0)]
        );
    }

    #[test]
    fn display_item_state_change_requires_full_repaint() {
        let previous_item = DisplayItem::PushOpacity { opacity: 0.5 };
        let current_item = DisplayItem::PushOpacity { opacity: 0.75 };
        let previous = vec![stable_metadata(7, 0, 0, &previous_item, 1)];
        let current = vec![stable_metadata(7, 0, 0, &current_item, 2)];

        let summary = diff_display_list_metadata(&previous, &current);

        assert_eq!(summary.changed_items, 1);
        assert!(summary.requires_full_repaint());
        assert!(summary.repaint_bounds.is_empty());
    }

    #[test]
    fn display_list_metadata_helper_marks_missing_identities_unstable() {
        let display_list = DisplayList::from_items(vec![
            fill_item(0.0, test_color(255, 0, 0)),
            fill_item(20.0, test_color(0, 255, 0)),
        ]);

        let metadata = display_list.item_metadata_with_identities([DisplayItemIdentity::new(1, 0)]);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].identity, Some(DisplayItemIdentity::new(1, 0)));
        assert_eq!(metadata[1].identity, None);
    }

    #[test]
    fn display_item_merge_class_is_conservative() {
        let red = test_color(255, 0, 0);
        let blue = test_color(0, 0, 255);
        let first_fill = fill_item(0.0, red);
        let second_fill = fill_item(20.0, red);
        let blue_fill = fill_item(40.0, blue);
        let square_solid = solid_item(60.0, red, 0.0);
        let rounded_solid = solid_item(80.0, red, 4.0);

        assert_eq!(first_fill.merge_class(), DisplayItemMergeClass::SolidFill);
        assert!(first_fill.can_merge_with(&second_fill));
        assert!(can_merge_display_items(&first_fill, &square_solid));
        assert!(!can_merge_display_items(&first_fill, &blue_fill));
        assert_eq!(
            rounded_solid.merge_class(),
            DisplayItemMergeClass::NonMergeable
        );
        assert!(!can_merge_display_items(&first_fill, &rounded_solid));
        assert!(!can_merge_display_items(
            &DisplayItem::Noop,
            &DisplayItem::Noop
        ));
    }

    fn fill_item_at(x: f32, y: f32, w: f32, h: f32, color: Color) -> DisplayItem {
        DisplayItem::FillRect {
            rect: test_rect(x, y, w, h),
            color,
        }
    }

    fn translucent(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
        Color {
            r: red,
            g: green,
            b: blue,
            a: alpha,
        }
    }

    #[test]
    fn merge_rejects_overlapping_translucent_fills() {
        // Same translucent colour, overlapping rects: merging would double-blend
        // the overlap region and lighten it. Must NOT merge (t49-e4-10).
        let color = translucent(255, 0, 0, 128);
        let a = fill_item_at(0.0, 0.0, 20.0, 20.0, color);
        let b = fill_item_at(10.0, 0.0, 20.0, 20.0, color);
        assert!(!can_merge_display_items(&a, &b));
    }

    #[test]
    fn merge_allows_disjoint_translucent_fills() {
        // Same translucent colour but disjoint rects: no pixel is composited
        // twice, so the merge is pixel-neutral and allowed.
        let color = translucent(255, 0, 0, 128);
        let a = fill_item_at(0.0, 0.0, 10.0, 10.0, color);
        let b = fill_item_at(20.0, 0.0, 10.0, 10.0, color);
        assert!(can_merge_display_items(&a, &b));
    }

    #[test]
    fn merge_allows_overlapping_opaque_fills() {
        // Fully opaque overlapping rects: double-covering an opaque pixel is a
        // no-op, so the merge stays pixel-neutral.
        let color = test_color(255, 0, 0);
        let a = fill_item_at(0.0, 0.0, 20.0, 20.0, color);
        let b = fill_item_at(10.0, 0.0, 20.0, 20.0, color);
        assert!(can_merge_display_items(&a, &b));
    }

    #[test]
    fn merge_rejects_overlapping_translucent_solid_color() {
        // Same guard applies to the SolidColor / SolidColor and mixed arms.
        let color = translucent(0, 0, 255, 200);
        let a = DisplayItem::SolidColor {
            rect: test_rect(0.0, 0.0, 20.0, 20.0),
            color,
            radius: Corners::all(EllipticalRadius::from(0.0)),
        };
        let b = DisplayItem::SolidColor {
            rect: test_rect(10.0, 0.0, 20.0, 20.0),
            color,
            radius: Corners::all(EllipticalRadius::from(0.0)),
        };
        assert!(!can_merge_display_items(&a, &b));

        let fill = fill_item_at(10.0, 0.0, 20.0, 20.0, color);
        assert!(!can_merge_display_items(&a, &fill));
        assert!(!can_merge_display_items(&fill, &a));
    }

    #[test]
    fn draw_op_count_excludes_non_pixel_hints() {
        // Hint / clip items carry a rect but produce no pixels; draw_op_count is
        // keyed on is_pixel_producing, so only the FillRect counts (t49-e4-14).
        let mut dl = DisplayList::new();
        dl.push(fill_item_at(0.0, 0.0, 10.0, 10.0, test_color(255, 0, 0)));
        dl.push(DisplayItem::SetCursor {
            rect: test_rect(0.0, 0.0, 10.0, 10.0),
            cursor: Cursor::default(),
        });
        dl.push(DisplayItem::Annotate {
            rect: test_rect(0.0, 0.0, 10.0, 10.0),
            label: "debug".to_string(),
        });
        dl.push(DisplayItem::PushClip {
            rect: test_rect(0.0, 0.0, 10.0, 10.0),
            radius: Corners::all(EllipticalRadius::from(0.0)),
        });
        dl.push(DisplayItem::SaveLayer {
            rect: test_rect(0.0, 0.0, 10.0, 10.0),
            opacity: 1.0,
        });
        dl.push(DisplayItem::PopClip);

        // Only the single FillRect is a draw op.
        assert_eq!(dl.draw_op_count(), 1);
        // The other five items are state/hint ops.
        assert_eq!(dl.state_op_count(), 5);
        assert_eq!(dl.draw_op_count() + dl.state_op_count(), dl.len());
    }

    #[test]
    fn draw_op_count_matches_kind_classifier() {
        // Cross-check: draw_op_count must equal the number of pixel-producing
        // kinds, never the number of items that merely have spatial bounds.
        let dl = DisplayList::from_items(vec![
            fill_item_at(0.0, 0.0, 10.0, 10.0, test_color(255, 0, 0)),
            DisplayItem::Text {
                rect: test_rect(0.0, 0.0, 10.0, 10.0),
                text: "hi".to_string(),
                color: test_color(0, 0, 0),
                font_size: 12.0,
                font_family: Arc::new(vec!["sans".to_string()]),
                font_weight: 400,
                font_style: FontStyle::Normal,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: LineHeight::Normal,
                text_align: TextAlign::Start,
                text_transform: TextTransform::None,
                text_overflow: TextOverflow::Clip,
                white_space: WhiteSpace::Normal,
                word_break: WordBreak::Normal,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: Vec::new(),
                text_emphasis: None,
                caret_color: None,
            },
            DisplayItem::AnimationHints {
                rect: test_rect(0.0, 0.0, 10.0, 10.0),
                animation_name: None,
                animation_duration: None,
                animation_timing_function: None,
                animation_delay: None,
                animation_iteration_count: "1".to_string(),
                animation_direction: "normal".to_string(),
                animation_fill_mode: "none".to_string(),
                animation_play_state: "running".to_string(),
                transition_property: None,
                transition_duration: None,
                transition_timing_function: None,
                transition_delay: None,
            },
            DisplayItem::Noop,
        ]);

        let pixel_kinds = dl
            .items
            .iter()
            .filter(|item| DisplayItemKind::of(item).is_pixel_producing())
            .count();
        assert_eq!(dl.draw_op_count(), pixel_kinds);
        assert_eq!(dl.draw_op_count(), 2); // FillRect + Text
    }

    #[test]
    fn diff_round_trip_leaves_no_stale_entries() {
        // A->B->A identity diff must report the original list fully retained with
        // zero residual added/removed/changed entries: proves the diff carries no
        // stale state across snapshots (no garbage repaint regions).
        let item_a = fill_item(0.0, test_color(255, 0, 0));
        let item_b = fill_item(20.0, test_color(0, 255, 0));
        let snap_v1 = vec![
            stable_metadata(1, 0, 0, &item_a, 100),
            stable_metadata(2, 0, 1, &item_b, 200),
        ];
        // Change item_b's revision, then change it back.
        let snap_v2 = vec![
            stable_metadata(1, 0, 0, &item_a, 100),
            stable_metadata(2, 0, 1, &item_b, 201),
        ];

        let forward = diff_display_list_metadata(&snap_v1, &snap_v2);
        assert_eq!(forward.changed_items, 1);
        assert_eq!(forward.added_items, 0);
        assert_eq!(forward.removed_items, 0);

        // Diffing back to the original must again retain item_a and report only
        // item_b changed — never leak a phantom removal/addition from v2.
        let backward = diff_display_list_metadata(&snap_v2, &snap_v1);
        assert_eq!(backward.changed_items, 1);
        assert_eq!(backward.added_items, 0);
        assert_eq!(backward.removed_items, 0);
        assert_eq!(backward.retained_items, 1);

        // An identity diff (v1 vs v1) must be completely empty: no stale carry.
        let identity = diff_display_list_metadata(&snap_v1, &snap_v1);
        assert!(identity.is_empty());
        assert_eq!(identity.retained_items, 2);
        assert!(identity.repaint_bounds.is_empty());
    }

    #[test]
    fn new_display_item_variants() {
        // Verify all new variants compile
        let items: Vec<DisplayItem> = vec![
            DisplayItem::LinearGradient {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                angle_deg: 180.0,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: Color {
                            r: 255,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                    },
                    GradientStop {
                        offset: 1.0,
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 255,
                            a: 255,
                        },
                    },
                ],
                radius: Corners::all(EllipticalRadius::from(0.0)),
            },
            DisplayItem::Outline {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 50.0,
                },
                width: 2.0,
                style: BorderLineStyle::Solid,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                offset: 0.0,
            },
            DisplayItem::Line {
                x1: 0.0,
                y1: 0.0,
                x2: 100.0,
                y2: 100.0,
                color: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                width: 1.0,
            },
            DisplayItem::PushFilter {
                filters: vec![FilterOp::Blur(5.0)],
            },
            DisplayItem::PopFilter,
            DisplayItem::PushBackdropFilter {
                filters: vec![FilterOp::Blur(20.0)],
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
            },
            DisplayItem::PopBackdropFilter,
            DisplayItem::Noop,
        ];
        assert_eq!(items.len(), 8);
    }

    // ─── Text Emphasis parsing tests ───────────────────────

    #[test]
    fn parse_filled_dot() {
        let em = TextEmphasis::parse("filled dot", None, None).unwrap();
        assert_eq!(em.fill, EmphasisFill::Filled);
        assert_eq!(em.shape, EmphasisShape::Dot);
        assert_eq!(em.position, EmphasisPosition::Over);
    }

    #[test]
    fn parse_open_circle() {
        let em = TextEmphasis::parse("open circle", None, None).unwrap();
        assert_eq!(em.fill, EmphasisFill::Open);
        assert_eq!(em.shape, EmphasisShape::Circle);
    }

    #[test]
    fn parse_triangle_default_fill() {
        let em = TextEmphasis::parse("triangle", None, None).unwrap();
        assert_eq!(em.fill, EmphasisFill::Filled);
        assert_eq!(em.shape, EmphasisShape::Triangle);
    }

    #[test]
    fn parse_sesame() {
        let em = TextEmphasis::parse("sesame", None, None).unwrap();
        assert_eq!(em.shape, EmphasisShape::Sesame);
    }

    #[test]
    fn parse_double_circle() {
        let em = TextEmphasis::parse("open double-circle", None, None).unwrap();
        assert_eq!(em.fill, EmphasisFill::Open);
        assert_eq!(em.shape, EmphasisShape::DoubleCircle);
    }

    #[test]
    fn parse_custom_string() {
        let em = TextEmphasis::parse("\"★\"", None, None).unwrap();
        assert_eq!(em.shape, EmphasisShape::Custom("★".to_string()));
        assert_eq!(em.fill, EmphasisFill::Filled);
    }

    #[test]
    fn parse_none_returns_none() {
        assert!(TextEmphasis::parse("none", None, None).is_none());
        assert!(TextEmphasis::parse("", None, None).is_none());
    }

    #[test]
    fn parse_with_color_and_position() {
        let red = Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let em = TextEmphasis::parse("filled dot", Some(red), Some("under")).unwrap();
        assert_eq!(em.color, red);
        assert_eq!(em.position, EmphasisPosition::Under);
    }

    #[test]
    fn parse_position_variants() {
        let em = TextEmphasis::parse("dot", None, Some("over right")).unwrap();
        assert_eq!(em.position, EmphasisPosition::OverRight);

        let em = TextEmphasis::parse("dot", None, Some("under left")).unwrap();
        assert_eq!(em.position, EmphasisPosition::UnderLeft);
    }

    #[test]
    fn parse_fill_only_defaults_to_dot() {
        let em = TextEmphasis::parse("filled", None, None).unwrap();
        assert_eq!(em.fill, EmphasisFill::Filled);
        assert_eq!(em.shape, EmphasisShape::Dot);
    }
}

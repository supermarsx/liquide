//! Display list — a flat list of paint commands with spatial indexing.
//!
//! A flat contiguous list of typed paint operations for recording and replay:
//! - Flat contiguous list of typed paint operations
//! - R-tree spatial index for efficient partial invalidation
//! - Push/Pop state commands for clip, transform, opacity, filters

use std::sync::Arc;

use liquide_compositor::geometry::Affine2D;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::property_tree::FilterOp;
use liquide_layout::Rect;
use liquide_style_engine::computed::{
    BorderLineStyle, Cursor, FontStyle, ImageOrientation, ImageRendering, Isolation, LineHeight,
    OverflowAnchor, OverscrollBehavior, ScrollBehavior, ScrollSnapAlign, ScrollSnapStop,
    ScrollSnapType, TextAlign, TextOverflow, TextTransform,
    TouchAction, WhiteSpace, WordBreak,
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
    Circle { cx: f32, cy: f32, r: f32 },
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32 },
    RoundedRect { rect: Rect, radii: Corners<EllipticalRadius> },
    Polygon(Vec<(f32, f32)>),
    Inset { top: f32, right: f32, bottom: f32, left: f32, radius: Corners<EllipticalRadius> },
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

        let color = color.unwrap_or(Color { r: 0, g: 0, b: 0, a: 255 });
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

        Some(TextEmphasis { fill, shape, color, position })
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
            color: Color { r: 0, g: 0, b: 0, a: 0 },
        }
    }
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

    /// Total number of draw operations (excludes state Push/Pop ops).
    pub fn draw_op_count(&self) -> usize {
        self.items.iter().filter(|item| is_draw_op(item)).count()
    }

    /// Total number of state Push/Pop operations.
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

        DisplayItem::BoxShadow { rect, offset_x, offset_y, blur_radius, spread_radius, inset, .. } => {
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
                Some(Rect { x: min_x, y: min_y, width: max_x - min_x, height: max_y - min_y })
            }
        }

        DisplayItem::Outline { rect, width, offset, .. } => {
            // Outline is drawn outside the border box, offset further by `offset`
            let expand = *width + offset.max(0.0);
            Some(Rect {
                x: rect.x - expand,
                y: rect.y - expand,
                width: rect.width + expand * 2.0,
                height: rect.height + expand * 2.0,
            })
        }

        DisplayItem::Line { x1, y1, x2, y2, width, .. } => {
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

/// Check if a display item is a draw operation (vs. state op).
fn is_draw_op(item: &DisplayItem) -> bool {
    item_bounds(item).is_some()
}

/// AABB intersection test.
fn rects_intersect(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width
        && a.x + a.width > b.x
        && a.y < b.y + b.height
        && a.y + a.height > b.y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_list_basics() {
        let mut dl = DisplayList::new();
        assert!(dl.is_empty());

        dl.push(DisplayItem::FillRect {
            rect: Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
            color: Color { r: 255, g: 0, b: 0, a: 255 },
        });
        assert_eq!(dl.len(), 1);
        assert_eq!(dl.draw_op_count(), 1);
        assert_eq!(dl.state_op_count(), 0);
    }

    #[test]
    fn spatial_query() {
        let mut dl = DisplayList::new();
        dl.push(DisplayItem::FillRect {
            rect: Rect { x: 0.0, y: 0.0, width: 50.0, height: 50.0 },
            color: Color { r: 255, g: 0, b: 0, a: 255 },
        });
        dl.push(DisplayItem::FillRect {
            rect: Rect { x: 100.0, y: 100.0, width: 50.0, height: 50.0 },
            color: Color { r: 0, g: 255, b: 0, a: 255 },
        });

        // Query top-left region
        let hits = dl.query_region(&Rect { x: 0.0, y: 0.0, width: 60.0, height: 60.0 });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], 0);

        // Query bottom-right region
        let hits = dl.query_region(&Rect { x: 90.0, y: 90.0, width: 70.0, height: 70.0 });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], 1);

        // Query everything
        let hits = dl.query_region(&Rect { x: 0.0, y: 0.0, width: 200.0, height: 200.0 });
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn state_ops_not_in_spatial_index() {
        let mut dl = DisplayList::new();
        dl.push(DisplayItem::PushOpacity { opacity: 0.5 });
        dl.push(DisplayItem::FillRect {
            rect: Rect { x: 10.0, y: 10.0, width: 20.0, height: 20.0 },
            color: Color { r: 0, g: 0, b: 0, a: 255 },
        });
        dl.push(DisplayItem::PopOpacity);

        assert_eq!(dl.draw_op_count(), 1);
        assert_eq!(dl.state_op_count(), 2);

        let hits = dl.query_region(&Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 });
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn new_display_item_variants() {
        // Verify all new variants compile
        let items: Vec<DisplayItem> = vec![
            DisplayItem::LinearGradient {
                rect: Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
                angle_deg: 180.0,
                stops: vec![
                    GradientStop { offset: 0.0, color: Color { r: 255, g: 0, b: 0, a: 255 } },
                    GradientStop { offset: 1.0, color: Color { r: 0, g: 0, b: 255, a: 255 } },
                ],
                radius: Corners::all(EllipticalRadius::from(0.0)),
            },
            DisplayItem::Outline {
                rect: Rect { x: 0.0, y: 0.0, width: 50.0, height: 50.0 },
                width: 2.0,
                style: BorderLineStyle::Solid,
                color: Color { r: 0, g: 0, b: 0, a: 255 },
                offset: 0.0,
            },
            DisplayItem::Line {
                x1: 0.0, y1: 0.0, x2: 100.0, y2: 100.0,
                color: Color { r: 0, g: 0, b: 0, a: 255 },
                width: 1.0,
            },
            DisplayItem::PushFilter {
                filters: vec![FilterOp::Blur(5.0)],
            },
            DisplayItem::PopFilter,
            DisplayItem::PushBackdropFilter {
                filters: vec![FilterOp::Blur(20.0)],
                bounds: Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
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
        let red = Color { r: 255, g: 0, b: 0, a: 255 };
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

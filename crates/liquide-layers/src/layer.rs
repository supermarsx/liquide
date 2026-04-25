//! Layer, PromotionReason, BlendMode, and core layer types.

/// Unique identifier for a compositor layer.
pub type LayerId = u64;

/// A rectangle in compositor-space pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge (x + width).
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y + height).
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether this rectangle intersects another.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Compute the intersection of two rectangles.
    #[must_use]
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right > x && bottom > y {
            Some(Rect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Compute the smallest rectangle that contains both rectangles.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Whether this rectangle fully contains another.
    #[must_use]
    pub fn contains_rect(&self, other: &Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    /// Whether the rectangle has positive area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Area in square pixels.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// A zero-size rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
}

/// A CSS filter operation attached to a layer (blur, drop-shadow, etc.).
///
/// Kept as a lightweight, renderer-agnostic descriptor. Concrete pixel
/// processing lives in the renderer crates; the layer only needs to
/// carry the description forward for the compositor to honour it.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOpKind {
    /// Gaussian blur with the given radius in pixels.
    Blur { radius: f32 },
    /// Brightness multiplier (1.0 = identity).
    Brightness { amount: f32 },
    /// Contrast multiplier (1.0 = identity).
    Contrast { amount: f32 },
    /// Grayscale amount (0.0 = identity, 1.0 = full gray).
    Grayscale { amount: f32 },
    /// Hue rotation in degrees.
    HueRotate { degrees: f32 },
    /// Invert amount (0.0 = identity, 1.0 = full invert).
    Invert { amount: f32 },
    /// Opacity multiplier (0.0 = transparent, 1.0 = identity).
    Opacity { amount: f32 },
    /// Saturation multiplier (1.0 = identity).
    Saturate { amount: f32 },
    /// Sepia amount (0.0 = identity, 1.0 = full sepia).
    Sepia { amount: f32 },
    /// Drop shadow with offset + blur + color (RGBA premultiplied).
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        rgba: [u8; 4],
    },
}

/// Ordered chain of filter operations applied to a layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterChain {
    pub ops: Vec<FilterOpKind>,
}

impl FilterChain {
    /// True if the chain has no ops (equivalent to `None`).
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Reference to an image or gradient mask applied to a layer.
///
/// The ID refers to a resource managed by a higher-level crate
/// (style-engine / renderer). Kept opaque here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MaskRef {
    pub resource_id: u64,
    /// Mask mode: 0 = alpha, 1 = luminance (matches CSS `mask-mode`).
    pub mode: u8,
}

/// Reference to a `clip-path` shape (polygon / circle / path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClipPathRef {
    pub resource_id: u64,
}

/// Porter-Duff and CSS blend modes for layer compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    /// Standard alpha compositing (source over destination).
    #[default]
    SrcOver,
    /// Replace destination entirely.
    Src,
    /// Multiply: out = src * dst per channel.
    Multiply,
    /// Screen: out = src + dst - src * dst.
    Screen,
    /// Overlay: multiply or screen based on dst luminance.
    Overlay,
    /// Darken: out = min(src, dst) per channel.
    Darken,
    /// Lighten: out = max(src, dst) per channel.
    Lighten,
    /// Difference: out = |src - dst|.
    Difference,
}

/// The reason an element was promoted to its own compositor layer.
///
/// Promotion allows the compositor to cache the rasterized content and
/// avoid re-painting when only compositor-level properties change (e.g.,
/// transform, opacity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromotionReason {
    /// Root layer — the desktop background surface.
    Root,
    /// CSS `will-change: transform` or `will-change: opacity`.
    WillChange,
    /// Element has a 3D or animated transform.
    HasTransform,
    /// Element has non-1.0 opacity or animated opacity.
    HasOpacity,
    /// Element has CSS filter or backdrop-filter.
    HasFilter,
    /// `position: fixed` — stays in place during scroll.
    FixedPosition,
    /// UI overlay surface (menus, tooltips, notifications).
    Overlay,
    /// Video or canvas content with independent update cadence.
    Video,
    /// Scrollable container — compositor can scroll without repainting.
    ScrollingContent,
    /// Explicitly promoted by the shell or application.
    Explicit,
}

/// The identity transform (no-op affine).
pub const IDENTITY_TRANSFORM: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// A cacheable compositor layer — a rendering surface with metadata for
/// compositing (transform, opacity, blend, clip, z-order).
#[derive(Debug, Clone)]
pub struct Layer {
    /// Unique identifier.
    pub id: LayerId,
    /// Position and size in parent-layer coordinates.
    pub bounds: Rect,
    /// The region of the layer that contains actual content (may be smaller
    /// than bounds if content doesn't fill the entire layer).
    pub content_rect: Rect,
    /// Cached RGBA pixel data. `None` means the layer has not been
    /// rasterized yet and must be painted before it can be composited.
    pub pixels: Option<Vec<u8>>,
    /// Whether the cached pixels are stale and need re-rasterization.
    pub is_dirty: bool,
    /// Layer opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// 2D affine transform matrix `[a, b, c, d, tx, ty]`:
    /// ```text
    /// | a  b  tx |
    /// | c  d  ty |
    /// | 0  0  1  |
    /// ```
    pub transform: [f32; 6],
    /// Compositing blend mode.
    pub blend_mode: BlendMode,
    /// Optional clip rectangle in layer-local coordinates.
    pub clip: Option<Rect>,
    /// Stacking order — higher values draw on top.
    pub z_order: i32,
    /// Why this element was promoted to its own layer.
    pub promotion_reason: PromotionReason,
    /// Number of frames since this layer was last marked dirty.
    /// Used by demotion heuristics to reclaim memory.
    pub frames_since_dirty: u64,
    /// Optional CSS `filter` chain applied to the layer's own content.
    pub filter: Option<FilterChain>,
    /// Optional CSS `backdrop-filter` chain applied to the content
    /// **behind** the layer (read the destination, blur it, re-composite).
    pub backdrop_filter: Option<FilterChain>,
    /// Optional mask image / gradient.
    pub mask: Option<MaskRef>,
    /// Optional `clip-path` shape (overrides the rectangular `clip` when set).
    pub clip_path: Option<ClipPathRef>,
    /// Whether this layer creates an isolated stacking context (CSS
    /// `isolation: isolate`). Affects how blend modes interact with
    /// the surrounding content.
    pub isolation: bool,
}

impl Layer {
    /// Create a new layer with default compositor properties.
    #[must_use]
    pub fn new(id: LayerId, bounds: Rect, reason: PromotionReason) -> Self {
        Self {
            id,
            bounds,
            content_rect: bounds,
            pixels: None,
            is_dirty: true,
            opacity: 1.0,
            transform: IDENTITY_TRANSFORM,
            blend_mode: BlendMode::default(),
            clip: None,
            z_order: 0,
            promotion_reason: reason,
            frames_since_dirty: 0,
            filter: None,
            backdrop_filter: None,
            mask: None,
            clip_path: None,
            isolation: false,
        }
    }

    /// Whether the layer has valid cached pixels.
    #[must_use]
    pub fn has_valid_cache(&self) -> bool {
        self.pixels.is_some() && !self.is_dirty
    }

    /// Pixel buffer size in bytes for this layer's bounds.
    #[must_use]
    pub fn pixel_buffer_size(&self) -> usize {
        let w = self.bounds.width.ceil() as usize;
        let h = self.bounds.height.ceil() as usize;
        w * h * 4 // RGBA
    }

    /// Mark this layer as needing re-rasterization.
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        self.frames_since_dirty = 0;
    }

    /// Clear the dirty flag (called after rasterization).
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
        self.frames_since_dirty = 0;
    }

    /// Apply the layer's affine transform to a point.
    #[must_use]
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, tx, ty] = self.transform;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    /// Apply the layer's affine transform to a rectangle, returning the
    /// axis-aligned bounding box.
    #[must_use]
    pub fn transform_rect(&self, r: Rect) -> Rect {
        let corners = [
            self.transform_point(r.x, r.y),
            self.transform_point(r.right(), r.y),
            self.transform_point(r.x, r.bottom()),
            self.transform_point(r.right(), r.bottom()),
        ];
        let min_x = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let min_y = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let max_x = corners
            .iter()
            .map(|p| p.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = corners
            .iter()
            .map(|p| p.1)
            .fold(f32::NEG_INFINITY, f32::max);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// Whether the transform is the identity (no rotation/scale/skew/translate).
    #[must_use]
    pub fn is_identity_transform(&self) -> bool {
        let [a, b, c, d, tx, ty] = self.transform;
        (a - 1.0).abs() < f32::EPSILON
            && b.abs() < f32::EPSILON
            && c.abs() < f32::EPSILON
            && (d - 1.0).abs() < f32::EPSILON
            && tx.abs() < f32::EPSILON
            && ty.abs() < f32::EPSILON
    }

    /// Whether this layer is fully opaque (no blending needed from opacity).
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        (self.opacity - 1.0).abs() < f32::EPSILON
    }
}

use crate::{Rect, Region, WindowExStyle, WindowFlags, WindowId, WindowStyle};

/// A single window in the tree hierarchy.
///
/// The tree is maintained via linked-list pointers: `first_child` points to
/// the topmost child, and children are chained via `next_sibling` /
/// `prev_sibling` (doubly-linked in z-order, front-to-back).
#[derive(Debug, Clone)]
pub struct WindowNode {
    /// Unique window identifier.
    pub id: WindowId,
    /// Parent window (`None` for the root desktop window).
    pub parent: Option<WindowId>,
    /// First (topmost in z-order) child.
    pub first_child: Option<WindowId>,
    /// Next sibling (lower in z-order).
    pub next_sibling: Option<WindowId>,
    /// Previous sibling (higher in z-order).
    pub prev_sibling: Option<WindowId>,
    /// Owner window (for popup windows — not the same as parent).
    pub owner: Option<WindowId>,
    /// Window rectangle in screen coordinates.
    pub bounds: Rect,
    /// Client area rectangle (inside borders/titlebar), screen coordinates.
    pub client_rect: Rect,
    /// Internal state flags.
    pub flags: WindowFlags,
    /// Window class atom.
    pub class_id: u32,
    /// Window style (WS_*).
    pub style: WindowStyle,
    /// Extended window style (WS_EX_*).
    pub ex_style: WindowExStyle,
    /// Optional custom clip region for hit testing.
    pub clip_region: Option<Region>,
    /// Window title / caption text.
    pub title: String,
    /// Invalid (dirty) region that needs repainting.
    pub(crate) update_region: Option<Rect>,
}

impl WindowNode {
    /// Create a new window node with default state.
    pub(crate) fn new(
        id: WindowId,
        parent: Option<WindowId>,
        class_id: u32,
        style: WindowStyle,
        ex_style: WindowExStyle,
        bounds: Rect,
        title: String,
    ) -> Self {
        // Compute a default client rect (caption=30px, border=1px if present).
        let border = if style.contains(WindowStyle::BORDER) || style.contains(WindowStyle::THICK_FRAME) {
            1
        } else {
            0
        };
        let caption = if style.contains(WindowStyle::CAPTION) { 30 } else { 0 };
        let client_rect = Rect::new(
            bounds.x + border,
            bounds.y + border + caption,
            (bounds.width - 2 * border).max(0),
            (bounds.height - 2 * border - caption).max(0),
        );

        Self {
            id,
            parent,
            first_child: None,
            next_sibling: None,
            prev_sibling: None,
            owner: None,
            bounds,
            client_rect,
            flags: WindowFlags::VISIBLE | WindowFlags::ENABLED,
            class_id,
            style,
            ex_style,
            clip_region: None,
            title,
            update_region: None,
        }
    }

    /// Whether this window is visible.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.flags.contains(WindowFlags::VISIBLE)
    }

    /// Whether this window accepts input.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.flags.contains(WindowFlags::ENABLED)
    }

    /// Whether this window is click-through.
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.flags.contains(WindowFlags::TRANSPARENT)
            || self.ex_style.contains(WindowExStyle::TRANSPARENT)
    }

    /// Whether this window is always-on-top.
    #[inline]
    pub fn is_topmost(&self) -> bool {
        self.flags.contains(WindowFlags::TOPMOST)
            || self.ex_style.contains(WindowExStyle::TOPMOST)
    }

    /// Whether this window is a child window.
    #[inline]
    pub fn is_child(&self) -> bool {
        self.style.contains(WindowStyle::CHILD)
    }

    /// Whether this window is a popup.
    #[inline]
    pub fn is_popup(&self) -> bool {
        self.style.contains(WindowStyle::POPUP)
    }

    /// Check if a screen-coordinate point falls within this window's bounds,
    /// respecting the optional clip region.
    pub fn point_in_window(&self, px: i32, py: i32) -> bool {
        if !self.bounds.contains_point(px, py) {
            return false;
        }
        if let Some(ref region) = self.clip_region {
            return region.contains_point(px, py);
        }
        true
    }

    /// Check if a screen-coordinate point falls within the client area.
    pub fn point_in_client(&self, px: i32, py: i32) -> bool {
        self.client_rect.contains_point(px, py)
    }
}

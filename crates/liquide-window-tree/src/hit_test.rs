use crate::{WindowFlags, WindowId, WindowNode, WindowStyle, WindowTree};

/// Which part of a window was hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitArea {
    /// Inside the client area.
    Client,
    /// On the title bar / caption.
    Caption,
    /// On a border edge (resizable).
    Border(ResizeEdge),
    /// On the close button.
    CloseButton,
    /// On the minimize button.
    MinButton,
    /// On the maximize button.
    MaxButton,
    /// On the system menu icon.
    SysMenu,
    /// On the vertical scroll bar.
    VScroll,
    /// On the horizontal scroll bar.
    HScroll,
    /// Not on any meaningful part.
    Nowhere,
    /// Window is transparent (click-through).
    Transparent,
    /// An error occurred during hit testing.
    Error,
}

/// Which edge of a window border was hit (for resizing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Result of a hit test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitTestResult {
    /// The window that was hit.
    pub window_id: WindowId,
    /// Which part of the window was hit.
    pub hit_area: HitArea,
    /// Point in window-local coordinates (relative to window bounds origin).
    pub local_point: (i32, i32),
}

/// Non-client area metrics for hit testing.
const BORDER_WIDTH: i32 = 4;
const CAPTION_HEIGHT: i32 = 30;
const BUTTON_WIDTH: i32 = 46;
const SCROLLBAR_WIDTH: i32 = 17;

impl WindowTree {
    /// Perform a recursive depth-first hit test at the given screen coordinate.
    ///
    /// Tests children before parent, front-to-back in z-order. The first
    /// (topmost) visible, enabled window that contains the point wins.
    pub fn hit_test(&self, point: (i32, i32)) -> Option<HitTestResult> {
        self.hit_test_window(self.desktop_id, point)
    }

    /// Simplified hit test — just returns the window id.
    pub fn window_at_point(&self, point: (i32, i32)) -> Option<WindowId> {
        self.hit_test(point).map(|r| r.window_id)
    }

    /// Recursive hit test starting at a specific window.
    fn hit_test_window(&self, id: WindowId, point: (i32, i32)) -> Option<HitTestResult> {
        let node = self.nodes.get(&id)?;

        // Skip invisible windows entirely.
        if !node.flags.contains(WindowFlags::VISIBLE) {
            return None;
        }

        // Skip windows being destroyed.
        if node.flags.contains(WindowFlags::IN_DESTROY) {
            return None;
        }

        // For non-desktop windows, check if point is in bounds.
        if id != self.desktop_id && !node.point_in_window(point.0, point.1) {
            return None;
        }

        // Test children first (front-to-back z-order) — depth-first.
        let mut child = node.first_child;
        while let Some(child_id) = child {
            if let Some(result) = self.hit_test_window(child_id, point) {
                return Some(result);
            }
            child = self.nodes.get(&child_id).and_then(|n| n.next_sibling);
        }

        // No child hit — test this window itself (skip desktop).
        if id == self.desktop_id {
            return None;
        }

        // Transparent windows are click-through.
        if node.is_transparent() {
            return Some(HitTestResult {
                window_id: id,
                hit_area: HitArea::Transparent,
                local_point: (point.0 - node.bounds.x, point.1 - node.bounds.y),
            });
        }

        Some(classify_hit(node, point))
    }
}

/// Classify which part of the window a point hits (non-client area detection).
fn classify_hit(node: &WindowNode, point: (i32, i32)) -> HitTestResult {
    let lx = point.0 - node.bounds.x;
    let ly = point.1 - node.bounds.y;
    let local_point = (lx, ly);
    let w = node.bounds.width;
    let h = node.bounds.height;

    // Non-client area classification — checked BEFORE client area because the
    // resize border zone may overlap the client rect (the visual border drawn
    // by the window is thinner than the hit-test grab zone).
    let has_caption = node.style.contains(WindowStyle::CAPTION);
    let has_border =
        node.style.contains(WindowStyle::BORDER) || node.style.contains(WindowStyle::THICK_FRAME);
    let resizable = node.style.contains(WindowStyle::THICK_FRAME);

    // Border / resize edges (highest priority).
    if has_border && resizable {
        if let Some(edge) = detect_resize_edge(lx, ly, w, h) {
            return HitTestResult {
                window_id: node.id,
                hit_area: HitArea::Border(edge),
                local_point,
            };
        }
    }

    // If in client area, it's a client hit.
    if node.point_in_client(point.0, point.1) {
        // Check scrollbars first (they overlap the client rect conceptually
        // but occupy the right/bottom edges).
        if node.style.contains(WindowStyle::VSCROLL) {
            let sb_x =
                node.client_rect.right() - SCROLLBAR_WIDTH - node.client_rect.x + node.bounds.x;
            if lx >= sb_x {
                return HitTestResult {
                    window_id: node.id,
                    hit_area: HitArea::VScroll,
                    local_point,
                };
            }
        }
        if node.style.contains(WindowStyle::HSCROLL) {
            let sb_y =
                node.client_rect.bottom() - SCROLLBAR_WIDTH - node.client_rect.y + node.bounds.y;
            if ly >= sb_y {
                return HitTestResult {
                    window_id: node.id,
                    hit_area: HitArea::HScroll,
                    local_point,
                };
            }
        }
        return HitTestResult {
            window_id: node.id,
            hit_area: HitArea::Client,
            local_point,
        };
    }

    // Caption area and its buttons.
    if has_caption && ly >= 0 && ly < CAPTION_HEIGHT + if has_border { BORDER_WIDTH } else { 0 } {
        // System menu icon (leftmost area of caption).
        if node.style.contains(WindowStyle::SYS_MENU) && lx < CAPTION_HEIGHT {
            return HitTestResult {
                window_id: node.id,
                hit_area: HitArea::SysMenu,
                local_point,
            };
        }

        // Caption buttons (right side): Close, Maximize, Minimize.
        let mut btn_x = w;
        if node.style.contains(WindowStyle::CLOSE_BOX) {
            btn_x -= BUTTON_WIDTH;
            if lx >= btn_x {
                return HitTestResult {
                    window_id: node.id,
                    hit_area: HitArea::CloseButton,
                    local_point,
                };
            }
        }
        if node.style.contains(WindowStyle::MAXIMIZE_BOX) {
            btn_x -= BUTTON_WIDTH;
            if lx >= btn_x {
                return HitTestResult {
                    window_id: node.id,
                    hit_area: HitArea::MaxButton,
                    local_point,
                };
            }
        }
        if node.style.contains(WindowStyle::MINIMIZE_BOX) {
            btn_x -= BUTTON_WIDTH;
            if lx >= btn_x {
                return HitTestResult {
                    window_id: node.id,
                    hit_area: HitArea::MinButton,
                    local_point,
                };
            }
        }

        return HitTestResult {
            window_id: node.id,
            hit_area: HitArea::Caption,
            local_point,
        };
    }

    HitTestResult {
        window_id: node.id,
        hit_area: HitArea::Nowhere,
        local_point,
    }
}

/// Detect which resize edge a point is on (within BORDER_WIDTH of window edges).
fn detect_resize_edge(lx: i32, ly: i32, w: i32, h: i32) -> Option<ResizeEdge> {
    let near_left = lx < BORDER_WIDTH;
    let near_right = lx >= w - BORDER_WIDTH;
    let near_top = ly < BORDER_WIDTH;
    let near_bottom = ly >= h - BORDER_WIDTH;

    match (near_left, near_right, near_top, near_bottom) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (true, _, _, true) => Some(ResizeEdge::BottomLeft),
        (_, true, true, _) => Some(ResizeEdge::TopRight),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Left),
        (_, true, _, _) => Some(ResizeEdge::Right),
        (_, _, true, _) => Some(ResizeEdge::Top),
        (_, _, _, true) => Some(ResizeEdge::Bottom),
        _ => None,
    }
}

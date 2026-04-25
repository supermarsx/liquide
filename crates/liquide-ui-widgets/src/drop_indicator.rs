//! Drop indicator — visual cue rendered during drag-and-drop to show
//! *where* a drop would land. Inspired by Qt's `QAbstractItemView`
//! drop indicators and GTK4's `GtkDropTarget` highlight.
//!
//! Widgets that act as drop targets (list, tree, tab strip, tool bar,
//! splitter gutter) own a [`DropIndicator`] and forward their local
//! drag-over coordinates to [`DropIndicator::update`]. On every paint
//! pass, they call [`DropIndicator::paint`] to render a line, a filled
//! rectangle, or nothing at all.

use liquide_ui_core::{Painter, UiTheme};

/// Which edge of the target rectangle the line indicator hugs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Shape of the drop hint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropShape {
    /// Thin line along one edge of the target — e.g. "insert row here"
    /// in a list view.
    Line { edge: Edge, thickness: f32 },
    /// Filled rectangle over the entire target — e.g. "drop file into
    /// this folder" in a tree view.
    Rect,
    /// Currently hidden.
    None,
}

/// A drop indicator with its own small bit of state (current shape +
/// target rectangle). Owned by the widget that participates in a DnD
/// operation.
#[derive(Debug, Clone)]
pub struct DropIndicator {
    shape: DropShape,
    rect: (f32, f32, f32, f32),
}

impl DropIndicator {
    pub fn new() -> Self {
        Self {
            shape: DropShape::None,
            rect: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Clear the indicator — call when the drag leaves the widget or
    /// the drop completes.
    pub fn hide(&mut self) {
        self.shape = DropShape::None;
    }

    /// Display a line along one edge of `rect`.
    pub fn show_line(&mut self, rect: (f32, f32, f32, f32), edge: Edge, thickness: f32) {
        self.shape = DropShape::Line {
            edge,
            thickness: thickness.max(1.0),
        };
        self.rect = rect;
    }

    /// Display a filled highlight over `rect`.
    pub fn show_rect(&mut self, rect: (f32, f32, f32, f32)) {
        self.shape = DropShape::Rect;
        self.rect = rect;
    }

    /// Whether the indicator is currently visible.
    pub fn is_visible(&self) -> bool {
        !matches!(self.shape, DropShape::None)
    }

    /// Update the indicator from a drag-over point inside `rect`. Uses
    /// a heuristic: if `y` is in the top quarter of the rect, show a
    /// `Top` line; if in the bottom quarter, a `Bottom` line; otherwise
    /// a full rectangle highlight (the "drop into" case).
    pub fn update(&mut self, rect: (f32, f32, f32, f32), y: f32) {
        let (rx, ry, _rw, rh) = rect;
        let local = (y - ry).clamp(0.0, rh);
        let quarter = rh * 0.25;
        if local < quarter {
            self.show_line(rect, Edge::Top, 2.0);
        } else if local > rh - quarter {
            self.show_line(rect, Edge::Bottom, 2.0);
        } else {
            self.show_rect(rect);
        }
        // rx unused beyond pass-through; silence clippy.
        let _ = rx;
    }

    pub fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let (x, y, w, h) = self.rect;
        let colors = &theme.colors;
        match self.shape {
            DropShape::None => {}
            DropShape::Line { edge, thickness } => {
                let (lx, ly, lw, lh) = match edge {
                    Edge::Top => (x, y - thickness * 0.5, w, thickness),
                    Edge::Bottom => (x, y + h - thickness * 0.5, w, thickness),
                    Edge::Left => (x - thickness * 0.5, y, thickness, h),
                    Edge::Right => (x + w - thickness * 0.5, y, thickness, h),
                };
                painter.fill_rounded_rect(lx, ly, lw, lh, thickness * 0.5, colors.accent);
            }
            DropShape::Rect => {
                painter.fill_rounded_rect(
                    x,
                    y,
                    w,
                    h,
                    theme.radius_sm,
                    colors.accent.with_alpha(60),
                );
                painter.stroke_rounded_rect(x, y, w, h, theme.radius_sm, colors.accent, 1.5);
            }
        }
    }
}

impl Default for DropIndicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_near_top_selects_top_edge_line() {
        let mut di = DropIndicator::new();
        di.update((0.0, 100.0, 200.0, 40.0), 102.0);
        assert!(matches!(
            di.shape,
            DropShape::Line {
                edge: Edge::Top,
                ..
            }
        ));
    }

    #[test]
    fn update_near_bottom_selects_bottom_edge_line() {
        let mut di = DropIndicator::new();
        di.update((0.0, 100.0, 200.0, 40.0), 138.0);
        assert!(matches!(
            di.shape,
            DropShape::Line {
                edge: Edge::Bottom,
                ..
            }
        ));
    }

    #[test]
    fn update_center_selects_rect() {
        let mut di = DropIndicator::new();
        di.update((0.0, 100.0, 200.0, 40.0), 120.0);
        assert!(matches!(di.shape, DropShape::Rect));
    }

    #[test]
    fn hide_clears_visibility() {
        let mut di = DropIndicator::new();
        di.show_rect((0.0, 0.0, 10.0, 10.0));
        assert!(di.is_visible());
        di.hide();
        assert!(!di.is_visible());
    }
}

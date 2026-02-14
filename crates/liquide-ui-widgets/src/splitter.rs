//! Splitter widget: resizable split pane container.
//!
//! Divides its area into two or more resizable regions separated by
//! draggable dividers. Supports horizontal and vertical orientations.

use serde::{Deserialize, Serialize};
use liquide_ui_core::WidgetId;

/// Splitter orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Panels are arranged left-to-right.
    Horizontal,
    /// Panels are arranged top-to-bottom.
    Vertical,
}

/// How a panel's size is specified.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PaneSize {
    /// Fixed pixel size.
    Fixed(f32),
    /// Proportional (fraction of remaining space, typically 0.0–1.0).
    Fraction(f32),
    /// Fill remaining space.
    Fill,
}

/// A panel in the splitter.
#[derive(Debug, Clone)]
pub struct SplitPane {
    /// Panel identifier.
    pub id: WidgetId,
    /// How this pane is sized.
    pub size: PaneSize,
    /// Minimum size in pixels.
    pub min_size: f32,
    /// Maximum size in pixels (None = no limit).
    pub max_size: Option<f32>,
    /// Whether this pane is collapsed.
    pub collapsed: bool,
    /// Whether this pane is collapsible.
    pub collapsible: bool,
}

impl SplitPane {
    #[must_use]
    pub fn new(id: WidgetId, size: PaneSize) -> Self {
        Self {
            id,
            size,
            min_size: 50.0,
            max_size: None,
            collapsed: false,
            collapsible: false,
        }
    }

    #[must_use]
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }

    #[must_use]
    pub fn with_min_size(mut self, min: f32) -> Self {
        self.min_size = min;
        self
    }
}

/// The splitter widget.
#[derive(Debug)]
pub struct Splitter {
    pub id: WidgetId,
    /// Panels to display.
    panes: Vec<SplitPane>,
    /// Split direction.
    pub direction: SplitDirection,
    /// Divider thickness in pixels.
    pub divider_size: f32,
    /// Which divider is currently being dragged (index).
    dragging_divider: Option<usize>,
    /// Total available size along the main axis.
    total_size: f32,
}

impl Splitter {
    #[must_use]
    pub fn new(id: WidgetId, direction: SplitDirection) -> Self {
        Self {
            id,
            panes: Vec::new(),
            direction,
            divider_size: 4.0,
            dragging_divider: None,
            total_size: 0.0,
        }
    }

    /// Add a pane.
    pub fn add_pane(&mut self, pane: SplitPane) {
        self.panes.push(pane);
    }

    /// Get the panes.
    #[must_use]
    pub fn panes(&self) -> &[SplitPane] {
        &self.panes
    }

    /// Number of panes.
    #[must_use]
    pub fn count(&self) -> usize {
        self.panes.len()
    }

    /// Set the total available size.
    pub fn set_total_size(&mut self, size: f32) {
        self.total_size = size;
    }

    /// Compute the pixel sizes of each pane.
    #[must_use]
    pub fn compute_sizes(&self) -> Vec<f32> {
        let divider_total = if self.panes.len() > 1 {
            (self.panes.len() - 1) as f32 * self.divider_size
        } else {
            0.0
        };

        let available = (self.total_size - divider_total).max(0.0);

        let mut sizes = vec![0.0_f32; self.panes.len()];
        let mut remaining = available;
        let mut fill_indices = Vec::new();
        let mut fraction_total: f32 = 0.0;

        // First pass: allocate fixed sizes and compute fraction total.
        for (i, pane) in self.panes.iter().enumerate() {
            if pane.collapsed {
                sizes[i] = 0.0;
                continue;
            }
            match pane.size {
                PaneSize::Fixed(px) => {
                    let clamped = clamp_size(px, pane.min_size, pane.max_size);
                    sizes[i] = clamped;
                    remaining -= clamped;
                }
                PaneSize::Fraction(f) => {
                    fraction_total += f;
                }
                PaneSize::Fill => {
                    fill_indices.push(i);
                }
            }
        }

        remaining = remaining.max(0.0);

        // Second pass: allocate fraction sizes.
        let mut after_fractions = remaining;
        for (i, pane) in self.panes.iter().enumerate() {
            if pane.collapsed {
                continue;
            }
            if let PaneSize::Fraction(f) = pane.size {
                let ratio = if fraction_total > 0.0 { f / fraction_total } else { 0.0 };
                let px = remaining * ratio;
                let clamped = clamp_size(px, pane.min_size, pane.max_size);
                sizes[i] = clamped;
                after_fractions -= clamped;
            }
        }

        // Third pass: distribute remaining to Fill panes.
        if !fill_indices.is_empty() {
            let fill_each = (after_fractions / fill_indices.len() as f32).max(0.0);
            for &i in &fill_indices {
                let clamped = clamp_size(fill_each, self.panes[i].min_size, self.panes[i].max_size);
                sizes[i] = clamped;
            }
        }

        sizes
    }

    /// Begin dragging a divider.
    pub fn start_drag(&mut self, divider_index: usize) {
        if divider_index < self.panes.len().saturating_sub(1) {
            self.dragging_divider = Some(divider_index);
        }
    }

    /// Update divider position during drag.
    pub fn update_drag(&mut self, delta: f32) {
        let Some(div) = self.dragging_divider else {
            return;
        };
        let left = div;
        let right = div + 1;

        if left >= self.panes.len() || right >= self.panes.len() {
            return;
        }

        let sizes = self.compute_sizes();
        let new_left = (sizes[left] + delta).max(self.panes[left].min_size);
        let new_right = (sizes[right] - delta).max(self.panes[right].min_size);

        // Only apply if both panes stay within bounds.
        let actual_delta = new_left - sizes[left];
        if actual_delta.abs() > 0.1 {
            self.panes[left].size = PaneSize::Fixed(new_left);
            self.panes[right].size = PaneSize::Fixed(new_right);
        }
    }

    /// End dragging.
    pub fn end_drag(&mut self) {
        self.dragging_divider = None;
    }

    /// Toggle collapse for a pane.
    pub fn toggle_collapse(&mut self, pane_index: usize) {
        if let Some(pane) = self.panes.get_mut(pane_index) {
            if pane.collapsible {
                pane.collapsed = !pane.collapsed;
            }
        }
    }
}

fn clamp_size(size: f32, min: f32, max: Option<f32>) -> f32 {
    let s = size.max(min);
    max.map_or(s, |m| s.min(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_split() {
        let mut sp = Splitter::new(WidgetId::from_raw(1), SplitDirection::Horizontal);
        sp.add_pane(SplitPane::new(WidgetId::from_raw(10), PaneSize::Fraction(0.5)).with_min_size(0.0));
        sp.add_pane(SplitPane::new(WidgetId::from_raw(11), PaneSize::Fraction(0.5)).with_min_size(0.0));
        sp.set_total_size(800.0);

        let sizes = sp.compute_sizes();
        let total: f32 = sizes.iter().sum();
        assert!((total - 796.0).abs() < 1.0, "total={total}"); // 800 - 4 divider
    }

    #[test]
    fn test_fixed_and_fill() {
        let mut sp = Splitter::new(WidgetId::from_raw(1), SplitDirection::Vertical);
        sp.add_pane(SplitPane::new(WidgetId::from_raw(10), PaneSize::Fixed(200.0)));
        sp.add_pane(SplitPane::new(WidgetId::from_raw(11), PaneSize::Fill).with_min_size(0.0));
        sp.set_total_size(600.0);

        let sizes = sp.compute_sizes();
        assert_eq!(sizes[0], 200.0);
        assert!((sizes[1] - 396.0).abs() < 1.0);
    }

    #[test]
    fn test_collapse() {
        let mut sp = Splitter::new(WidgetId::from_raw(1), SplitDirection::Horizontal);
        sp.add_pane(SplitPane::new(WidgetId::from_raw(10), PaneSize::Fixed(200.0)).collapsible());
        sp.add_pane(SplitPane::new(WidgetId::from_raw(11), PaneSize::Fill).with_min_size(0.0));
        sp.set_total_size(800.0);

        sp.toggle_collapse(0);
        let sizes = sp.compute_sizes();
        assert_eq!(sizes[0], 0.0); // collapsed
    }

    #[test]
    fn test_three_panes() {
        let mut sp = Splitter::new(WidgetId::from_raw(1), SplitDirection::Horizontal);
        sp.add_pane(SplitPane::new(WidgetId::from_raw(10), PaneSize::Fraction(0.25)).with_min_size(0.0));
        sp.add_pane(SplitPane::new(WidgetId::from_raw(11), PaneSize::Fraction(0.5)).with_min_size(0.0));
        sp.add_pane(SplitPane::new(WidgetId::from_raw(12), PaneSize::Fraction(0.25)).with_min_size(0.0));
        sp.set_total_size(1000.0);

        let sizes = sp.compute_sizes();
        assert_eq!(sizes.len(), 3);
        let total: f32 = sizes.iter().sum();
        assert!((total - 992.0).abs() < 1.0); // 1000 - 2*4 dividers
    }
}

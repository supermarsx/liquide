//! Focus management for keyboard navigation.

use crate::widget::WidgetId;

/// Direction of focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusDirection {
    /// Move to the next focusable widget.
    Next,
    /// Move to the previous focusable widget.
    Previous,
    /// Move focus upward.
    Up,
    /// Move focus downward.
    Down,
    /// Move focus left.
    Left,
    /// Move focus right.
    Right,
}

/// A linear chain of focusable widgets supporting Tab and directional navigation.
#[derive(Debug, Clone, Default)]
pub struct FocusChain {
    /// Ordered list of focusable widget identifiers.
    ordered_ids: Vec<WidgetId>,
    /// Index of the currently focused widget, if any.
    current_index: Option<usize>,
}

impl FocusChain {
    /// Create a new empty focus chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The currently focused widget, if any.
    #[must_use]
    pub fn current(&self) -> Option<WidgetId> {
        self.current_index.map(|i| self.ordered_ids[i])
    }

    /// Move focus in the given direction and return the newly focused widget.
    ///
    /// For `Next`/`Down`/`Right` the focus moves forward; for
    /// `Previous`/`Up`/`Left` it moves backward.  Wraps around at the
    /// ends of the chain.  Returns `None` if the chain is empty.
    pub fn move_focus(&mut self, direction: FocusDirection) -> Option<WidgetId> {
        if self.ordered_ids.is_empty() {
            return None;
        }

        let forward = matches!(
            direction,
            FocusDirection::Next | FocusDirection::Down | FocusDirection::Right
        );

        let new_index = match self.current_index {
            Some(idx) => {
                if forward {
                    (idx + 1) % self.ordered_ids.len()
                } else if idx == 0 {
                    self.ordered_ids.len() - 1
                } else {
                    idx - 1
                }
            }
            None => {
                if forward {
                    0
                } else {
                    self.ordered_ids.len() - 1
                }
            }
        };

        self.current_index = Some(new_index);
        Some(self.ordered_ids[new_index])
    }

    /// Set focus to a specific widget.
    ///
    /// Returns `true` if the widget was found in the chain.
    pub fn set_focus(&mut self, id: WidgetId) -> bool {
        if let Some(idx) = self.ordered_ids.iter().position(|w| *w == id) {
            self.current_index = Some(idx);
            true
        } else {
            false
        }
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        self.current_index = None;
    }

    /// Add a widget to the end of the focus chain.
    pub fn add(&mut self, id: WidgetId) {
        if !self.ordered_ids.contains(&id) {
            self.ordered_ids.push(id);
        }
    }

    /// Remove a widget from the focus chain.
    pub fn remove(&mut self, id: &WidgetId) {
        if let Some(pos) = self.ordered_ids.iter().position(|w| w == id) {
            self.ordered_ids.remove(pos);
            // Adjust current_index
            match self.current_index {
                Some(idx) if idx == pos => {
                    if self.ordered_ids.is_empty() {
                        self.current_index = None;
                    } else if idx >= self.ordered_ids.len() {
                        self.current_index = Some(self.ordered_ids.len() - 1);
                    }
                }
                Some(idx) if idx > pos => {
                    self.current_index = Some(idx - 1);
                }
                _ => {}
            }
        }
    }

    /// Whether the focus chain contains a widget.
    #[must_use]
    pub fn contains(&self, id: &WidgetId) -> bool {
        self.ordered_ids.contains(id)
    }

    /// Number of widgets in the focus chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ordered_ids.len()
    }

    /// Whether the focus chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ordered_ids.is_empty()
    }
}

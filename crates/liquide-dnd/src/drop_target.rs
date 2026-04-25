//! Drop target API.
//!
//! A drop target is a widget region that can accept data from a drag operation.
//! It inspects offered MIME types to decide whether to accept, and specifies
//! the resulting effect (copy/move/link).

use crate::data_transfer::{DataTransfer, MimeType};
use crate::drag_source::DragAction;
use serde::{Deserialize, Serialize};

/// The visual effect shown when hovering over a drop target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropEffect {
    None,
    Copy,
    Move,
    Link,
}

impl From<DragAction> for DropEffect {
    fn from(action: DragAction) -> Self {
        match action {
            DragAction::None => DropEffect::None,
            DragAction::Copy => DropEffect::Copy,
            DragAction::Move => DropEffect::Move,
            DragAction::Link => DropEffect::Link,
        }
    }
}

/// Events received by a drop target.
#[derive(Debug, Clone)]
pub enum DropTargetEvent {
    /// Drag entered the target area.
    DragEnter {
        /// MIME types offered by the source.
        offered_types: Vec<MimeType>,
        x: f32,
        y: f32,
    },
    /// Drag moved within the target area.
    DragOver { x: f32, y: f32 },
    /// Drag left the target area.
    DragLeave,
    /// Data was dropped.
    Drop {
        data: DataTransfer,
        action: DragAction,
        x: f32,
        y: f32,
    },
}

/// A drop target that can accept dragged data.
pub struct DropTarget {
    /// MIME types this target accepts.
    accepted_types: Vec<String>,
    /// Whether a drag is currently over this target.
    drag_over: bool,
    /// Current drop effect.
    effect: DropEffect,
    /// Whether the target is enabled.
    enabled: bool,
    /// Pending events.
    events: Vec<DropTargetEvent>,
}

impl DropTarget {
    #[must_use]
    pub fn new(accepted_types: Vec<String>) -> Self {
        Self {
            accepted_types,
            drag_over: false,
            effect: DropEffect::None,
            enabled: true,
            events: Vec::new(),
        }
    }

    /// Create a target that accepts text.
    #[must_use]
    pub fn text() -> Self {
        Self::new(vec![MimeType::TEXT_PLAIN.to_string()])
    }

    /// Create a target that accepts files (URI list).
    #[must_use]
    pub fn files() -> Self {
        Self::new(vec![MimeType::TEXT_URI_LIST.to_string()])
    }

    /// Set whether the drop target is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if a drag is currently over this target.
    #[must_use]
    pub fn is_drag_over(&self) -> bool {
        self.drag_over
    }

    /// Get the current drop effect.
    #[must_use]
    pub fn effect(&self) -> DropEffect {
        self.effect
    }

    /// Test whether this target can accept any of the offered types.
    #[must_use]
    pub fn can_accept(&self, offered: &[MimeType]) -> bool {
        if !self.enabled {
            return false;
        }
        offered
            .iter()
            .any(|m| self.accepted_types.iter().any(|a| *a == m.0))
    }

    /// Handle drag entering the target area.
    pub fn handle_drag_enter(
        &mut self,
        offered_types: Vec<MimeType>,
        x: f32,
        y: f32,
    ) -> DropEffect {
        if self.can_accept(&offered_types) {
            self.drag_over = true;
            self.effect = DropEffect::Copy;
            self.events.push(DropTargetEvent::DragEnter {
                offered_types,
                x,
                y,
            });
            DropEffect::Copy
        } else {
            self.effect = DropEffect::None;
            DropEffect::None
        }
    }

    /// Handle drag moving over the target.
    pub fn handle_drag_over(&mut self, x: f32, y: f32) -> DropEffect {
        if self.drag_over {
            self.events.push(DropTargetEvent::DragOver { x, y });
        }
        self.effect
    }

    /// Handle drag leaving the target.
    pub fn handle_drag_leave(&mut self) {
        self.drag_over = false;
        self.effect = DropEffect::None;
        self.events.push(DropTargetEvent::DragLeave);
    }

    /// Handle a drop.
    pub fn handle_drop(&mut self, data: DataTransfer, action: DragAction, x: f32, y: f32) -> bool {
        let accepted = self.drag_over;
        self.drag_over = false;
        self.effect = DropEffect::None;

        if accepted {
            self.events
                .push(DropTargetEvent::Drop { data, action, x, y });
        }
        accepted
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<DropTargetEvent> {
        std::mem::take(&mut self.events)
    }
}

/// The result of a drop operation on a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropResult {
    /// The drop was accepted with the given action.
    Accepted(DropAction),
    /// The drop was rejected.
    Rejected,
}

impl DropResult {
    /// Whether the drop was accepted.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        matches!(self, DropResult::Accepted(_))
    }

    /// Whether the drop was rejected.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, DropResult::Rejected)
    }

    /// Get the action if accepted.
    #[must_use]
    pub fn action(&self) -> Option<DropAction> {
        match self {
            DropResult::Accepted(a) => Some(*a),
            DropResult::Rejected => None,
        }
    }
}

/// Action performed as a result of a drop.
///
/// Distinct from `DragAction` (source-side) and `DropEffect` (visual).
/// This represents the semantic action the target will take on the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropAction {
    /// Copy the data to the target.
    Copy,
    /// Move the data (source should delete its copy).
    Move,
    /// Create a link/reference/shortcut.
    Link,
    /// No action (drop accepted but no-op).
    None,
}

/// A rectangular region registered as a drop target.
#[derive(Debug, Clone)]
pub struct DropTargetRegion {
    /// Unique identifier for this region.
    pub id: u64,
    /// Bounding rectangle: x, y, width, height.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// MIME types this region accepts.
    pub accepted_types: Vec<String>,
    /// The action this target prefers.
    pub preferred_action: DropAction,
    /// Whether the target is currently active/enabled.
    pub enabled: bool,
}

impl DropTargetRegion {
    /// Create a new drop target region.
    #[must_use]
    pub fn new(
        id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        accepted_types: Vec<String>,
    ) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            accepted_types,
            preferred_action: DropAction::Copy,
            enabled: true,
        }
    }

    /// Set the preferred drop action.
    #[must_use]
    pub fn with_action(mut self, action: DropAction) -> Self {
        self.preferred_action = action;
        self
    }

    /// Whether the point is inside this region.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.enabled
            && px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }

    /// Whether this region accepts any of the given MIME types.
    #[must_use]
    pub fn accepts_any(&self, types: &[String]) -> bool {
        self.enabled
            && types
                .iter()
                .any(|t| self.accepted_types.iter().any(|a| a == t))
    }
}

/// Visual indicator shown when a drag is over a valid drop target.
#[derive(Debug, Clone, PartialEq)]
pub enum DropIndicator {
    /// Highlight border around the target region.
    HighlightBorder {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Border color as (r, g, b, a).
        color: (u8, u8, u8, u8),
        /// Border thickness in pixels.
        thickness: f32,
    },
    /// Insertion line between items (for list/grid reordering).
    InsertionLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        /// Line color.
        color: (u8, u8, u8, u8),
        /// Line thickness.
        thickness: f32,
    },
    /// No visual indicator.
    None,
}

impl Default for DropIndicator {
    fn default() -> Self {
        DropIndicator::None
    }
}

/// Registry of spatial drop target regions with hit-testing.
///
/// Regions are tested in reverse insertion order (last registered = highest
/// priority), matching typical z-order where later elements are on top.
pub struct DropTargetRegistry {
    regions: Vec<DropTargetRegion>,
    /// The currently active (hovered) target, if any.
    active_target: Option<u64>,
    /// Visual indicator for the active target.
    indicator: DropIndicator,
}

impl DropTargetRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            active_target: None,
            indicator: DropIndicator::None,
        }
    }

    /// Register a drop target region.
    pub fn register(&mut self, region: DropTargetRegion) {
        // Replace existing region with same id
        self.regions.retain(|r| r.id != region.id);
        self.regions.push(region);
    }

    /// Unregister a region by id.
    pub fn unregister(&mut self, id: u64) {
        self.regions.retain(|r| r.id != id);
        if self.active_target == Some(id) {
            self.active_target = None;
            self.indicator = DropIndicator::None;
        }
    }

    /// Number of registered regions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Find the topmost target at position (x, y).
    ///
    /// Searches in reverse order (highest z-order first).
    #[must_use]
    pub fn find_target_at(&self, x: f32, y: f32) -> Option<&DropTargetRegion> {
        self.regions.iter().rev().find(|r| r.contains(x, y))
    }

    /// Find the topmost target at (x, y) that accepts any of the given types.
    #[must_use]
    pub fn find_accepting_target_at(
        &self,
        x: f32,
        y: f32,
        offered_types: &[String],
    ) -> Option<&DropTargetRegion> {
        self.regions
            .iter()
            .rev()
            .find(|r| r.contains(x, y) && r.accepts_any(offered_types))
    }

    /// Update the active target based on cursor position and offered types.
    ///
    /// Returns the target id if changed (enter/leave), or `None` if unchanged.
    pub fn update_hover(
        &mut self,
        x: f32,
        y: f32,
        offered_types: &[String],
    ) -> Option<HoverChange> {
        let new_target = self
            .find_accepting_target_at(x, y, offered_types)
            .map(|r| r.id);

        if new_target == self.active_target {
            return None;
        }

        let old = self.active_target;
        self.active_target = new_target;

        // Update indicator
        if let Some(id) = new_target {
            if let Some(region) = self.regions.iter().find(|r| r.id == id) {
                self.indicator = DropIndicator::HighlightBorder {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                    color: (66, 133, 244, 200), // blue highlight
                    thickness: 2.0,
                };
            }
        } else {
            self.indicator = DropIndicator::None;
        }

        Some(HoverChange {
            left: old,
            entered: new_target,
        })
    }

    /// Get the current drop indicator visual.
    #[must_use]
    pub fn drop_indicator(&self) -> &DropIndicator {
        &self.indicator
    }

    /// Set a custom drop indicator.
    pub fn set_indicator(&mut self, indicator: DropIndicator) {
        self.indicator = indicator;
    }

    /// Get the currently active (hovered) target id.
    #[must_use]
    pub fn active_target_id(&self) -> Option<u64> {
        self.active_target
    }

    /// Get a region by id.
    #[must_use]
    pub fn get_region(&self, id: u64) -> Option<&DropTargetRegion> {
        self.regions.iter().find(|r| r.id == id)
    }

    /// Clear the active target and indicator.
    pub fn clear_active(&mut self) {
        self.active_target = None;
        self.indicator = DropIndicator::None;
    }

    /// Remove all regions.
    pub fn clear(&mut self) {
        self.regions.clear();
        self.active_target = None;
        self.indicator = DropIndicator::None;
    }
}

impl Default for DropTargetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes a hover change in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverChange {
    /// The target that was left (if any).
    pub left: Option<u64>,
    /// The target that was entered (if any).
    pub entered: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_transfer::DataPayload;

    #[test]
    fn test_accept_check() {
        let target = DropTarget::text();
        assert!(target.can_accept(&[MimeType::text_plain()]));
        assert!(!target.can_accept(&[MimeType::text_html()]));
    }

    #[test]
    fn test_drag_enter_leave() {
        let mut target = DropTarget::text();
        let eff = target.handle_drag_enter(vec![MimeType::text_plain()], 10.0, 20.0);
        assert_eq!(eff, DropEffect::Copy);
        assert!(target.is_drag_over());

        target.handle_drag_leave();
        assert!(!target.is_drag_over());
    }

    #[test]
    fn test_drop_accepted() {
        let mut target = DropTarget::text();
        target.handle_drag_enter(vec![MimeType::text_plain()], 5.0, 5.0);

        let mut data = DataTransfer::new();
        data.add(DataPayload::text("dropped text"));
        let accepted = target.handle_drop(data, DragAction::Copy, 5.0, 5.0);
        assert!(accepted);
        assert!(!target.is_drag_over());
    }

    #[test]
    fn test_disabled_target() {
        let mut target = DropTarget::text();
        target.set_enabled(false);
        assert!(!target.can_accept(&[MimeType::text_plain()]));
    }

    // ---- DropResult tests ----

    #[test]
    fn test_drop_result_accepted() {
        let r = DropResult::Accepted(DropAction::Copy);
        assert!(r.is_accepted());
        assert!(!r.is_rejected());
        assert_eq!(r.action(), Some(DropAction::Copy));
    }

    #[test]
    fn test_drop_result_rejected() {
        let r = DropResult::Rejected;
        assert!(r.is_rejected());
        assert!(!r.is_accepted());
        assert_eq!(r.action(), None);
    }

    // ---- DropTargetRegion tests ----

    #[test]
    fn test_region_contains() {
        let r = DropTargetRegion::new(1, 10.0, 20.0, 100.0, 50.0, vec!["text/plain".into()]);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(110.0, 20.0)); // right edge exclusive
        assert!(!r.contains(9.0, 20.0));
    }

    #[test]
    fn test_region_disabled() {
        let mut r = DropTargetRegion::new(1, 0.0, 0.0, 100.0, 100.0, vec!["text/plain".into()]);
        r.enabled = false;
        assert!(!r.contains(50.0, 50.0));
        assert!(!r.accepts_any(&["text/plain".into()]));
    }

    #[test]
    fn test_region_accepts_any() {
        let r = DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into(), "text/html".into()],
        );
        assert!(r.accepts_any(&["text/plain".into()]));
        assert!(r.accepts_any(&["text/html".into()]));
        assert!(!r.accepts_any(&["image/png".into()]));
    }

    #[test]
    fn test_region_with_action() {
        let r =
            DropTargetRegion::new(1, 0.0, 0.0, 10.0, 10.0, vec![]).with_action(DropAction::Move);
        assert_eq!(r.preferred_action, DropAction::Move);
    }

    // ---- DropTargetRegistry tests ----

    #[test]
    fn test_registry_register_find() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        assert_eq!(reg.len(), 1);
        let t = reg.find_target_at(50.0, 50.0).unwrap();
        assert_eq!(t.id, 1);
    }

    #[test]
    fn test_registry_find_none() {
        let reg = DropTargetRegistry::new();
        assert!(reg.find_target_at(50.0, 50.0).is_none());
    }

    #[test]
    fn test_registry_z_order() {
        let mut reg = DropTargetRegistry::new();
        // Overlapping regions — last registered wins
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        reg.register(DropTargetRegion::new(
            2,
            25.0,
            25.0,
            50.0,
            50.0,
            vec!["text/plain".into()],
        ));
        let t = reg.find_target_at(50.0, 50.0).unwrap();
        assert_eq!(t.id, 2); // region 2 is on top
    }

    #[test]
    fn test_registry_unregister() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(1, 0.0, 0.0, 100.0, 100.0, vec![]));
        reg.unregister(1);
        assert!(reg.is_empty());
    }

    #[test]
    fn test_registry_replace_same_id() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(1, 0.0, 0.0, 50.0, 50.0, vec![]));
        reg.register(DropTargetRegion::new(1, 10.0, 10.0, 80.0, 80.0, vec![]));
        assert_eq!(reg.len(), 1);
        let r = reg.get_region(1).unwrap();
        assert!((r.x - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_registry_find_accepting() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["image/png".into()],
        ));
        reg.register(DropTargetRegion::new(
            2,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        // Looking for text/plain — should skip region 2 (on top) if it matches, but
        // region 2 accepts text/plain, so it wins.
        let t = reg
            .find_accepting_target_at(50.0, 50.0, &["text/plain".into()])
            .unwrap();
        assert_eq!(t.id, 2);

        // Looking for image/png — region 2 doesn't accept, falls through to region 1
        let t = reg
            .find_accepting_target_at(50.0, 50.0, &["image/png".into()])
            .unwrap();
        assert_eq!(t.id, 1);
    }

    #[test]
    fn test_registry_update_hover_enter() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        let change = reg
            .update_hover(50.0, 50.0, &["text/plain".into()])
            .unwrap();
        assert_eq!(change.left, None);
        assert_eq!(change.entered, Some(1));
        assert_eq!(reg.active_target_id(), Some(1));
    }

    #[test]
    fn test_registry_update_hover_leave() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        reg.update_hover(50.0, 50.0, &["text/plain".into()]);
        let change = reg
            .update_hover(200.0, 200.0, &["text/plain".into()])
            .unwrap();
        assert_eq!(change.left, Some(1));
        assert_eq!(change.entered, None);
    }

    #[test]
    fn test_registry_update_hover_no_change() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        reg.update_hover(50.0, 50.0, &["text/plain".into()]);
        // Same target — no change
        let change = reg.update_hover(55.0, 55.0, &["text/plain".into()]);
        assert!(change.is_none());
    }

    #[test]
    fn test_registry_drop_indicator_highlight() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            10.0,
            20.0,
            100.0,
            50.0,
            vec!["text/plain".into()],
        ));
        reg.update_hover(50.0, 40.0, &["text/plain".into()]);
        match reg.drop_indicator() {
            DropIndicator::HighlightBorder {
                x,
                y,
                width,
                height,
                ..
            } => {
                assert!((x - 10.0).abs() < f32::EPSILON);
                assert!((y - 20.0).abs() < f32::EPSILON);
                assert!((width - 100.0).abs() < f32::EPSILON);
                assert!((height - 50.0).abs() < f32::EPSILON);
            }
            other => panic!("expected HighlightBorder, got {:?}", other),
        }
    }

    #[test]
    fn test_registry_indicator_cleared_on_leave() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        reg.update_hover(50.0, 50.0, &["text/plain".into()]);
        reg.update_hover(200.0, 200.0, &["text/plain".into()]);
        assert_eq!(*reg.drop_indicator(), DropIndicator::None);
    }

    #[test]
    fn test_registry_set_custom_indicator() {
        let mut reg = DropTargetRegistry::new();
        reg.set_indicator(DropIndicator::InsertionLine {
            x1: 0.0,
            y1: 50.0,
            x2: 100.0,
            y2: 50.0,
            color: (255, 0, 0, 255),
            thickness: 2.0,
        });
        match reg.drop_indicator() {
            DropIndicator::InsertionLine { y1, y2, .. } => {
                assert!((y1 - 50.0).abs() < f32::EPSILON);
                assert!((y2 - 50.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected InsertionLine"),
        }
    }

    #[test]
    fn test_registry_clear() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(1, 0.0, 0.0, 10.0, 10.0, vec![]));
        reg.register(DropTargetRegion::new(2, 0.0, 0.0, 10.0, 10.0, vec![]));
        reg.clear();
        assert!(reg.is_empty());
        assert!(reg.active_target_id().is_none());
    }

    #[test]
    fn test_registry_clear_active() {
        let mut reg = DropTargetRegistry::new();
        reg.register(DropTargetRegion::new(
            1,
            0.0,
            0.0,
            100.0,
            100.0,
            vec!["text/plain".into()],
        ));
        reg.update_hover(50.0, 50.0, &["text/plain".into()]);
        assert!(reg.active_target_id().is_some());
        reg.clear_active();
        assert!(reg.active_target_id().is_none());
        assert_eq!(*reg.drop_indicator(), DropIndicator::None);
    }
}

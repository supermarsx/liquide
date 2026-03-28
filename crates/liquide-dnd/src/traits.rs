//! Drag source and drop target traits.
//!
//! These traits define the interface that widgets/windows implement to
//! participate in drag-and-drop operations using the typed [`DragData`]
//! payload system.

use crate::drag_data::DragData;
use crate::drop_target::DropEffect;
use crate::preview::DragPreview;

/// Trait for widgets or windows that can initiate drag operations.
///
/// Implementors describe what data they offer and how the drag should
/// be visually represented.
pub trait DragSourceHandler {
    /// Whether this source can currently initiate a drag.
    fn can_drag(&self) -> bool;

    /// The data to offer when a drag starts.
    fn drag_data(&self) -> DragData;

    /// The visual preview to show during the drag.
    fn drag_preview(&self) -> DragPreview;
}

/// Trait for widgets or windows that can receive dropped data.
///
/// The methods are called by the [`DragManager`](crate::manager::DragManager)
/// as the drag cursor moves over registered targets.
pub trait DropTargetHandler {
    /// Whether this target can accept the offered data.
    ///
    /// Called before `on_drag_enter` to quickly filter incompatible drags.
    fn accepts(&self, data: &DragData) -> bool;

    /// Called when a drag first enters this target's bounds.
    ///
    /// Returns the desired [`DropEffect`] (e.g., Copy, Move) or `None` to
    /// reject.
    fn on_drag_enter(&mut self, data: &DragData) -> DropEffect;

    /// Called repeatedly as the drag moves within this target's bounds.
    ///
    /// `x` and `y` are in the target's local coordinate space.
    fn on_drag_over(&mut self, x: f32, y: f32, data: &DragData) -> DropEffect;

    /// Called when the drag leaves this target's bounds.
    fn on_drag_leave(&mut self);

    /// Called when data is dropped on this target.
    ///
    /// `x` and `y` are in the target's local coordinate space.
    /// Returns `true` if the drop was successfully handled.
    fn on_drop(&mut self, x: f32, y: f32, data: DragData) -> bool;
}

/// A simple drop target that accepts formats matching a predicate.
///
/// Useful for tests and simple widgets that just need to accept certain
/// data formats without complex logic.
pub struct SimpleDragSource {
    data: DragData,
    preview: DragPreview,
    enabled: bool,
}

impl SimpleDragSource {
    /// Create a new simple drag source.
    #[must_use]
    pub fn new(data: DragData, preview: DragPreview) -> Self {
        Self {
            data,
            preview,
            enabled: true,
        }
    }

    /// Set whether this source is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl DragSourceHandler for SimpleDragSource {
    fn can_drag(&self) -> bool {
        self.enabled && !self.data.is_empty()
    }

    fn drag_data(&self) -> DragData {
        self.data.clone()
    }

    fn drag_preview(&self) -> DragPreview {
        self.preview.clone()
    }
}

/// A simple drop target that accepts any data containing specific format types.
pub struct SimpleDropTarget {
    /// Which format types to accept (checked via predicate on format name).
    accepted_format_names: Vec<String>,
    /// The effect to report.
    default_effect: DropEffect,
    /// Data received from the last successful drop.
    received: Option<DragData>,
    /// Whether a drag is currently over this target.
    drag_over: bool,
}

impl SimpleDropTarget {
    /// Create a drop target that accepts the named formats.
    ///
    /// Format names correspond to [`DragFormat::format_name()`](crate::drag_data::DragFormat::format_name):
    /// `"text"`, `"file-paths"`, `"uri"`, `"image"`, or a custom MIME type.
    #[must_use]
    pub fn new(accepted: Vec<String>, effect: DropEffect) -> Self {
        Self {
            accepted_format_names: accepted,
            default_effect: effect,
            received: None,
            drag_over: false,
        }
    }

    /// Create a drop target that accepts text.
    #[must_use]
    pub fn text() -> Self {
        Self::new(vec!["text".to_string()], DropEffect::Copy)
    }

    /// Create a drop target that accepts file paths.
    #[must_use]
    pub fn file_paths() -> Self {
        Self::new(vec!["file-paths".to_string()], DropEffect::Copy)
    }

    /// Get the data from the last successful drop.
    #[must_use]
    pub fn received(&self) -> Option<&DragData> {
        self.received.as_ref()
    }

    /// Take the data from the last successful drop.
    pub fn take_received(&mut self) -> Option<DragData> {
        self.received.take()
    }

    /// Whether a drag is currently over this target.
    #[must_use]
    pub fn is_drag_over(&self) -> bool {
        self.drag_over
    }
}

impl DropTargetHandler for SimpleDropTarget {
    fn accepts(&self, data: &DragData) -> bool {
        data.formats().iter().any(|f| {
            self.accepted_format_names
                .iter()
                .any(|a| a == f.format_name())
        })
    }

    fn on_drag_enter(&mut self, data: &DragData) -> DropEffect {
        if self.accepts(data) {
            self.drag_over = true;
            self.default_effect
        } else {
            DropEffect::None
        }
    }

    fn on_drag_over(&mut self, _x: f32, _y: f32, data: &DragData) -> DropEffect {
        if self.drag_over && self.accepts(data) {
            self.default_effect
        } else {
            DropEffect::None
        }
    }

    fn on_drag_leave(&mut self) {
        self.drag_over = false;
    }

    fn on_drop(&mut self, _x: f32, _y: f32, data: DragData) -> bool {
        if self.drag_over {
            self.drag_over = false;
            self.received = Some(data);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drag_data::DragData;

    #[test]
    fn test_simple_drag_source() {
        let source = SimpleDragSource::new(
            DragData::text("hello"),
            DragPreview::text_label("hello"),
        );
        assert!(source.can_drag());
        let data = source.drag_data();
        assert_eq!(data.get_text(), Some("hello"));
    }

    #[test]
    fn test_simple_drag_source_disabled() {
        let mut source = SimpleDragSource::new(
            DragData::text("hello"),
            DragPreview::text_label("hello"),
        );
        source.set_enabled(false);
        assert!(!source.can_drag());
    }

    #[test]
    fn test_simple_drag_source_empty_data() {
        let source = SimpleDragSource::new(DragData::new(), DragPreview::text_label("empty"));
        assert!(!source.can_drag()); // empty data = can't drag
    }

    #[test]
    fn test_simple_drop_target_accepts_text() {
        let target = SimpleDropTarget::text();
        assert!(target.accepts(&DragData::text("hello")));
        assert!(!target.accepts(&DragData::file_paths(vec!["a.txt".into()])));
    }

    #[test]
    fn test_simple_drop_target_lifecycle() {
        let mut target = SimpleDropTarget::text();
        let data = DragData::text("drag me");

        let eff = target.on_drag_enter(&data);
        assert_eq!(eff, DropEffect::Copy);
        assert!(target.is_drag_over());

        let eff = target.on_drag_over(10.0, 20.0, &data);
        assert_eq!(eff, DropEffect::Copy);

        let ok = target.on_drop(10.0, 20.0, data);
        assert!(ok);
        assert!(!target.is_drag_over());

        let received = target.take_received().unwrap();
        assert_eq!(received.get_text(), Some("drag me"));
    }

    #[test]
    fn test_simple_drop_target_leave() {
        let mut target = SimpleDropTarget::text();
        let data = DragData::text("test");
        target.on_drag_enter(&data);
        assert!(target.is_drag_over());

        target.on_drag_leave();
        assert!(!target.is_drag_over());
    }

    #[test]
    fn test_simple_drop_target_reject_incompatible() {
        let mut target = SimpleDropTarget::file_paths();
        let data = DragData::text("not a file");
        let eff = target.on_drag_enter(&data);
        assert_eq!(eff, DropEffect::None);
        assert!(!target.is_drag_over());
    }
}

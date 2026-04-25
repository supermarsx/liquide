//! Bridge between the DnD session and the cursor shape subsystem.
//!
//! While a drag is in progress the cursor needs to reflect the current drop
//! intent: a grabbing-hand while hovering a valid target, a forbidden sign
//! while over an invalid one, and so on. Rather than coupling
//! `liquide-dnd` directly to `liquide-cursor` (which would create a
//! dependency edge the dnd crate does not otherwise need) this module
//! publishes a small, crate-local [`DndCursorShape`] enum and a
//! [`CursorLink`] trait. The shell implements the trait by forwarding to
//! the cursor manager.
//!
//! ```
//! # use liquide_dnd::cursor_link::{DndCursorShape, BufferedCursorLink, CursorLink};
//! let mut link = BufferedCursorLink::default();
//! link.publish(DndCursorShape::Grabbing);
//! assert_eq!(link.last(), Some(DndCursorShape::Grabbing));
//! ```

use crate::drag_source::DragAction;

/// Cursor shape requested by the DnD session.
///
/// Maps 1:1 onto a subset of `liquide_cursor::CursorShape` variants — the
/// translation lives in the shell to keep this crate free of cursor
/// dependencies. Platform bridges for XDND / OLE / NSDrag / Wayland DnD are
/// **deferred** to later tracks (see t9 plan §4.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DndCursorShape {
    /// A closed hand — actively dragging something.
    Grabbing,
    /// Drop target is valid; `move` intent.
    Move,
    /// Drop target is valid; `copy` intent.
    Copy,
    /// Drop target is valid; `link` / alias intent.
    Link,
    /// Drop target will not accept the drop.
    NotAllowed,
    /// No drop target under the cursor (pointer over empty space).
    NoDrop,
}

impl DndCursorShape {
    /// Pick the right cursor shape for the given negotiated [`DragAction`].
    pub fn for_action(action: DragAction) -> Self {
        match action {
            DragAction::Move => Self::Move,
            DragAction::Copy => Self::Copy,
            DragAction::Link => Self::Link,
            DragAction::None => Self::NotAllowed,
        }
    }

    /// Pick a cursor shape from a list of allowed actions. Prefers `Move`
    /// > `Copy` > `Link`, falling back to `NotAllowed` when empty.
    pub fn for_allowed(allowed: &[DragAction]) -> Self {
        if allowed.contains(&DragAction::Move) {
            Self::Move
        } else if allowed.contains(&DragAction::Copy) {
            Self::Copy
        } else if allowed.contains(&DragAction::Link) {
            Self::Link
        } else {
            Self::NotAllowed
        }
    }
}

/// Sink for DnD-driven cursor shape changes.
///
/// Implemented by the shell to forward into the cursor manager.
pub trait CursorLink {
    /// Publish the requested cursor shape.
    ///
    /// Called at most once per frame by the drag manager. The implementation
    /// may coalesce duplicate requests.
    fn publish(&mut self, shape: DndCursorShape);
}

/// A no-op [`CursorLink`] — useful in tests or when the platform has no
/// cursor concept (e.g. touch-only devices).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullCursorLink;

impl CursorLink for NullCursorLink {
    fn publish(&mut self, _shape: DndCursorShape) {}
}

/// A buffering [`CursorLink`] that just records the most recent publish.
///
/// Primarily used in tests and in shells that want to batch cursor updates
/// into their own frame loop.
#[derive(Debug, Default, Clone, Copy)]
pub struct BufferedCursorLink {
    last: Option<DndCursorShape>,
}

impl BufferedCursorLink {
    /// The most recently published shape, or `None` if nothing published yet.
    pub fn last(&self) -> Option<DndCursorShape> {
        self.last
    }

    /// Clear the buffered shape.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

impl CursorLink for BufferedCursorLink {
    fn publish(&mut self, shape: DndCursorShape) {
        self.last = Some(shape);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_action_prefers_move() {
        assert_eq!(
            DndCursorShape::for_action(DragAction::Move),
            DndCursorShape::Move
        );
        assert_eq!(
            DndCursorShape::for_action(DragAction::Copy),
            DndCursorShape::Copy
        );
        assert_eq!(
            DndCursorShape::for_action(DragAction::Link),
            DndCursorShape::Link
        );
        assert_eq!(
            DndCursorShape::for_action(DragAction::None),
            DndCursorShape::NotAllowed
        );
    }

    #[test]
    fn for_allowed_prefers_move() {
        assert_eq!(
            DndCursorShape::for_allowed(&[DragAction::Copy, DragAction::Move]),
            DndCursorShape::Move,
        );
    }

    #[test]
    fn for_allowed_empty_is_not_allowed() {
        assert_eq!(DndCursorShape::for_allowed(&[]), DndCursorShape::NotAllowed,);
    }

    #[test]
    fn buffered_link_records_last() {
        let mut link = BufferedCursorLink::default();
        assert_eq!(link.last(), None);
        link.publish(DndCursorShape::Grabbing);
        link.publish(DndCursorShape::Copy);
        assert_eq!(link.last(), Some(DndCursorShape::Copy));
        link.reset();
        assert_eq!(link.last(), None);
    }

    #[test]
    fn null_link_is_noop() {
        let mut link = NullCursorLink;
        link.publish(DndCursorShape::Grabbing); // must not panic
    }
}

//! Extended grouping functionality: events and tab navigation.

use crate::group::WindowId;
use crate::tabs::TabGroupId;
use crate::group::GroupId;

/// Events emitted by group operations.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupEvent {
    /// A new window group was created.
    Created { group_id: GroupId },
    /// A window was added to a group.
    WindowAdded {
        group_id: GroupId,
        window_id: WindowId,
    },
    /// A window was removed from a group.
    WindowRemoved {
        group_id: GroupId,
        window_id: WindowId,
    },
    /// The active tab changed within a tab group.
    TabChanged {
        tab_group_id: TabGroupId,
        old_index: usize,
        new_index: usize,
        window_id: WindowId,
    },
    /// A group was dissolved (deleted).
    Dissolved { group_id: GroupId },
    /// A tab group was created from a window group.
    TabGroupCreated {
        tab_group_id: TabGroupId,
        group_id: GroupId,
    },
    /// A tab was detached from a tab group.
    TabDetached {
        tab_group_id: TabGroupId,
        window_id: WindowId,
    },
    /// A tab group was dissolved.
    TabGroupDissolved { tab_group_id: TabGroupId },
}

/// An event log that records group events for external consumption.
#[derive(Debug, Default)]
pub struct GroupEventLog {
    events: Vec<GroupEvent>,
}

impl GroupEventLog {
    /// Create a new empty event log.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record an event.
    pub fn push(&mut self, event: GroupEvent) {
        self.events.push(event);
    }

    /// Drain all events, returning them.
    pub fn drain(&mut self) -> Vec<GroupEvent> {
        std::mem::take(&mut self.events)
    }

    /// Peek at the most recent event.
    pub fn last(&self) -> Option<&GroupEvent> {
        self.events.last()
    }

    /// Returns the number of pending events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true if there are no pending events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

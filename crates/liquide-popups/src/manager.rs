//! Central popup orchestrator that owns all open popups and manages their
//! lifecycle.

use crate::Rect;
use crate::dialog_info::DialogInfo;
use crate::events::EventRouter;
use crate::popup::{Popup, PopupConfig, PopupId, PopupType, WindowId};
use crate::position::PopupPositioner;
use crate::stack::PopupStack;

/// Central popup manager that coordinates creation, positioning, z-ordering,
/// and dismissal of all popup windows.
pub struct PopupManager {
    /// All currently open popups, in insertion order.
    popups: Vec<Popup>,
    /// Z-order assignment.
    stack: PopupStack,
    /// Monotonically increasing ID counter.
    next_id: u64,
    /// Screen bounds (for positioning).
    screen: Rect,
}

impl PopupManager {
    /// Create a new popup manager with the given screen dimensions.
    #[must_use]
    pub fn new(screen: Rect) -> Self {
        Self {
            popups: Vec::new(),
            stack: PopupStack::new(),
            next_id: 1,
            screen,
        }
    }

    /// Update the screen dimensions (call on resize).
    pub fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
        self.reposition_all();
    }

    /// Open a new popup with the given configuration.
    ///
    /// The popup's final bounds are computed by [`PopupPositioner`] taking
    /// screen edges and existing popups into account. Returns the assigned
    /// popup ID.
    pub fn open(&mut self, config: PopupConfig) -> PopupId {
        self.open_at_time(config, 0)
    }

    /// Open a new popup with an explicit creation timestamp (microseconds).
    pub fn open_at_time(&mut self, config: PopupConfig, now_us: u64) -> PopupId {
        let id = PopupId::new(self.next_id);
        self.next_id += 1;

        let z = self
            .stack
            .z_order_for_popup(config.popup_type, config.modal);
        let bounds = PopupPositioner::position(&config, self.screen, &self.popups);

        let popup = Popup::from_config(id, &config, bounds, z, now_us);
        self.popups.push(popup);
        id
    }

    /// Close a specific popup by ID. Returns `true` if it was found and removed.
    pub fn close(&mut self, id: PopupId) -> bool {
        let len_before = self.popups.len();
        self.popups.retain(|p| p.id != id);
        let removed = self.popups.len() < len_before;
        if self.popups.is_empty() {
            self.stack.reset();
        }
        removed
    }

    /// Close all open popups.
    pub fn close_all(&mut self) {
        self.popups.clear();
        self.stack.reset();
    }

    /// Close all popups of a specific type.
    pub fn close_type(&mut self, popup_type: PopupType) {
        self.popups.retain(|p| p.popup_type != popup_type);
        if self.popups.is_empty() {
            self.stack.reset();
        }
    }

    /// Close all popups owned by a specific window.
    pub fn close_owned_by(&mut self, window_id: WindowId) {
        self.popups.retain(|p| p.owner != Some(window_id));
        if self.popups.is_empty() {
            self.stack.reset();
        }
    }

    /// All currently open popups sorted by z-order (lowest first).
    #[must_use]
    pub fn active_popups(&self) -> Vec<&Popup> {
        let mut refs: Vec<&Popup> = self.popups.iter().collect();
        PopupStack::sort_by_z_order(&mut refs);
        refs
    }

    /// All popups (unsorted, insertion order).
    #[must_use]
    pub fn popups(&self) -> &[Popup] {
        &self.popups
    }

    /// Number of open popups.
    #[must_use]
    pub fn count(&self) -> usize {
        self.popups.len()
    }

    /// Whether any popups are open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.popups.is_empty()
    }

    /// The topmost popup (highest z-order).
    #[must_use]
    pub fn topmost_popup(&self) -> Option<&Popup> {
        self.popups.iter().max_by_key(|p| p.z_order)
    }

    /// Hit test: find the popup at the given screen point (topmost).
    #[must_use]
    pub fn popup_at_point(&self, x: f32, y: f32) -> Option<PopupId> {
        EventRouter::popup_at_point(&self.popups, x, y)
    }

    /// Whether any modal dialog is currently open.
    #[must_use]
    pub fn is_modal_active(&self) -> bool {
        self.popups.iter().any(|p| p.modal)
    }

    /// The owner of the active modal dialog, if any.
    #[must_use]
    pub fn modal_owner(&self) -> Option<WindowId> {
        self.popups
            .iter()
            .filter(|p| p.modal)
            .max_by_key(|p| p.z_order)
            .and_then(|p| p.owner)
    }

    /// Whether a modal popup is blocking events to `target_window`.
    #[must_use]
    pub fn should_block_event(&self, target_window: WindowId) -> bool {
        EventRouter::should_block_event(&self.popups, target_window)
    }

    /// Determine which popups to dismiss because the user clicked outside.
    #[must_use]
    pub fn handle_click_outside(&self, x: f32, y: f32) -> Vec<PopupId> {
        EventRouter::handle_click_outside(&self.popups, x, y)
    }

    /// Determine which popup to dismiss on Escape.
    #[must_use]
    pub fn handle_escape(&self) -> Option<PopupId> {
        EventRouter::handle_escape(&self.popups)
    }

    /// Determine which popups to dismiss on focus change.
    #[must_use]
    pub fn handle_focus_change(&self, new_focus: WindowId) -> Vec<PopupId> {
        EventRouter::handle_focus_change(&self.popups, new_focus)
    }

    /// Self-invoked lifecycle: compute the set of popups to dismiss for a
    /// click at `(x, y)` *and* close them. Returns the ids that were closed.
    pub fn on_click_outside(&mut self, x: f32, y: f32) -> Vec<PopupId> {
        let ids = EventRouter::handle_click_outside(&self.popups, x, y);
        for id in &ids {
            self.close(*id);
        }
        ids
    }

    /// Self-invoked lifecycle: find the topmost Escape-dismissable popup and
    /// close it. Returns the id closed, if any.
    pub fn on_escape(&mut self) -> Option<PopupId> {
        if let Some(id) = EventRouter::handle_escape(&self.popups) {
            self.close(id);
            Some(id)
        } else {
            None
        }
    }

    /// Self-invoked lifecycle: dismiss popups that should close when focus
    /// moves to `new_focus` and close them. Returns the ids that were closed.
    pub fn on_focus_change(&mut self, new_focus: WindowId) -> Vec<PopupId> {
        let ids = EventRouter::handle_focus_change(&self.popups, new_focus);
        for id in &ids {
            self.close(*id);
        }
        ids
    }

    /// Whether a non-modal popup can currently be shown.
    ///
    /// Non-modal popups should not render while a modal dialog is up —
    /// they would be blocked by the modal overlay and create confusing UX.
    #[must_use]
    pub fn can_show_nonmodal(&self) -> bool {
        !self.is_modal_active()
    }

    /// Open a dialog described by a value implementing [`DialogInfo`].
    ///
    /// The dialog is always opened modally. Returns the assigned popup id.
    pub fn show_dialog<D: DialogInfo>(&mut self, dialog: &D, owner: WindowId) -> PopupId {
        self.open(dialog.popup_config_with_owner(Some(owner)))
    }

    /// Close all popups that should auto-dismiss at the current time.
    /// Returns the IDs of dismissed popups.
    pub fn dismiss_expired(&mut self, now_us: u64) -> Vec<PopupId> {
        let mut dismissed = Vec::new();
        self.popups.retain(|p| {
            if p.should_auto_dismiss(now_us) {
                dismissed.push(p.id);
                false
            } else {
                true
            }
        });
        if self.popups.is_empty() {
            self.stack.reset();
        }
        dismissed
    }

    /// Get a popup by ID.
    #[must_use]
    pub fn get(&self, id: PopupId) -> Option<&Popup> {
        self.popups.iter().find(|p| p.id == id)
    }

    /// Get a mutable reference to a popup by ID.
    #[must_use]
    pub fn get_mut(&mut self, id: PopupId) -> Option<&mut Popup> {
        self.popups.iter_mut().find(|p| p.id == id)
    }

    /// Update a popup's bounds (e.g. after content changes).
    pub fn update_bounds(&mut self, id: PopupId, bounds: Rect) {
        if let Some(index) = self.popups.iter().position(|popup| popup.id == id) {
            let popup = &mut self.popups[index];
            popup.preferred_x = bounds.x;
            popup.preferred_y = bounds.y;
            popup.bounds.width = bounds.width;
            popup.bounds.height = bounds.height;
            self.reposition_popup(index);
        }
    }

    fn reposition_all(&mut self) {
        if self.popups.is_empty() {
            return;
        }

        let popups = std::mem::take(&mut self.popups);
        let mut repositioned = Vec::with_capacity(popups.len());
        for mut popup in popups {
            let config = popup.to_config();
            popup.bounds = PopupPositioner::position(&config, self.screen, &repositioned);
            repositioned.push(popup);
        }
        self.popups = repositioned;
    }

    fn reposition_popup(&mut self, index: usize) {
        let mut popup = self.popups.remove(index);
        let config = popup.to_config();
        popup.bounds = PopupPositioner::position(&config, self.screen, &self.popups);
        self.popups.insert(index, popup);
    }
}

impl Default for PopupManager {
    fn default() -> Self {
        Self::new(Rect::new(0.0, 0.0, 1920.0, 1080.0))
    }
}

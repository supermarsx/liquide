//! Specialized tooltip handling with show/hide delay and single-tooltip policy.

/// Tooltip display state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TooltipState {
    /// No tooltip pending or visible.
    Idle,
    /// Waiting for the show delay to expire.
    PendingShow,
    /// Tooltip is visible.
    Visible,
    /// Waiting for the hide delay to expire (allows moving to tooltip).
    PendingHide,
}

/// Specialized tooltip controller.
///
/// Enforces single-tooltip policy (only one visible at a time) and manages
/// show/hide delay timers. This controller does not own popup instances —
/// it works with [`PopupManager`](crate::PopupManager) to open/close tooltip
/// popups.
pub struct TooltipController {
    /// Delay before showing a tooltip (ms).
    pub show_delay_ms: u32,
    /// Delay before hiding (allows moving cursor to tooltip) (ms).
    pub hide_delay_ms: u32,
    /// Current state.
    state: TooltipState,
    /// Elapsed time in current state (ms).
    elapsed_ms: f32,
    /// Current tooltip text.
    text: String,
    /// Anchor position for the pending/visible tooltip.
    anchor_x: f32,
    anchor_y: f32,
    /// Whether a show or hide action should be taken (consumed by caller).
    pending_action: Option<TooltipAction>,
}

/// Actions the tooltip controller requests from the popup manager.
#[derive(Debug, Clone, PartialEq)]
pub enum TooltipAction {
    /// Show a tooltip with the given text at the given position.
    Show {
        text: String,
        anchor_x: f32,
        anchor_y: f32,
    },
    /// Hide the currently visible tooltip.
    Hide,
}

impl TooltipController {
    /// Create a new tooltip controller with default delays.
    #[must_use]
    pub fn new() -> Self {
        Self {
            show_delay_ms: 500,
            hide_delay_ms: 100,
            state: TooltipState::Idle,
            elapsed_ms: 0.0,
            text: String::new(),
            anchor_x: 0.0,
            anchor_y: 0.0,
            pending_action: None,
        }
    }

    /// Create a tooltip controller with custom delays.
    #[must_use]
    pub fn with_delays(show_delay_ms: u32, hide_delay_ms: u32) -> Self {
        Self {
            show_delay_ms,
            hide_delay_ms,
            ..Self::new()
        }
    }

    /// Request showing a tooltip. Starts the show timer.
    pub fn show_tooltip(&mut self, text: &str, anchor_x: f32, anchor_y: f32) {
        // If already showing the same text at the same position, ignore.
        if self.state == TooltipState::Visible
            && self.text == text
            && (self.anchor_x - anchor_x).abs() < 1.0
            && (self.anchor_y - anchor_y).abs() < 1.0
        {
            return;
        }

        // If we were pending hide, cancel it and stay visible (new tooltip).
        if self.state == TooltipState::PendingHide {
            // Immediately transition to a new tooltip.
            self.text.clear();
            self.text.push_str(text);
            self.anchor_x = anchor_x;
            self.anchor_y = anchor_y;
            self.state = TooltipState::Visible;
            self.elapsed_ms = 0.0;
            self.pending_action = Some(TooltipAction::Show {
                text: self.text.clone(),
                anchor_x,
                anchor_y,
            });
            return;
        }

        self.text.clear();
        self.text.push_str(text);
        self.anchor_x = anchor_x;
        self.anchor_y = anchor_y;
        self.state = TooltipState::PendingShow;
        self.elapsed_ms = 0.0;
    }

    /// Request hiding the tooltip. Starts the hide timer.
    pub fn hide_tooltip(&mut self) {
        match self.state {
            TooltipState::Visible => {
                self.state = TooltipState::PendingHide;
                self.elapsed_ms = 0.0;
            }
            TooltipState::PendingShow => {
                // Cancel pending show.
                self.state = TooltipState::Idle;
                self.elapsed_ms = 0.0;
            }
            _ => {}
        }
    }

    /// Cancel any pending show or hide action immediately.
    pub fn cancel(&mut self) {
        if self.state == TooltipState::Visible {
            self.pending_action = Some(TooltipAction::Hide);
        }
        self.state = TooltipState::Idle;
        self.elapsed_ms = 0.0;
        self.text.clear();
    }

    /// Tick the controller. Call once per frame with the delta time in ms.
    ///
    /// After calling `update`, check `take_action()` to see if the popup
    /// manager needs to open or close a tooltip popup.
    pub fn update(&mut self, dt_ms: f32) {
        self.pending_action = None;

        match self.state {
            TooltipState::Idle | TooltipState::Visible => {}
            TooltipState::PendingShow => {
                self.elapsed_ms += dt_ms;
                if self.elapsed_ms >= self.show_delay_ms as f32 {
                    self.state = TooltipState::Visible;
                    self.elapsed_ms = 0.0;
                    self.pending_action = Some(TooltipAction::Show {
                        text: self.text.clone(),
                        anchor_x: self.anchor_x,
                        anchor_y: self.anchor_y,
                    });
                }
            }
            TooltipState::PendingHide => {
                self.elapsed_ms += dt_ms;
                if self.elapsed_ms >= self.hide_delay_ms as f32 {
                    self.state = TooltipState::Idle;
                    self.elapsed_ms = 0.0;
                    self.text.clear();
                    self.pending_action = Some(TooltipAction::Hide);
                }
            }
        }
    }

    /// Consume and return the pending action, if any.
    #[must_use]
    pub fn take_action(&mut self) -> Option<TooltipAction> {
        self.pending_action.take()
    }

    /// Whether a tooltip is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.state == TooltipState::Visible
    }

    /// Whether a show is pending.
    #[must_use]
    pub fn is_pending_show(&self) -> bool {
        self.state == TooltipState::PendingShow
    }

    /// Whether a hide is pending.
    #[must_use]
    pub fn is_pending_hide(&self) -> bool {
        self.state == TooltipState::PendingHide
    }

    /// Current tooltip text (empty if idle).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Default for TooltipController {
    fn default() -> Self {
        Self::new()
    }
}

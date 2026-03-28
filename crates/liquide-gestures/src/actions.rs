use crate::recognizer::{Edge, GestureEvent, GesturePhase, SwipeDirection};

/// Action triggered by a gesture
#[derive(Debug, Clone)]
pub enum GestureAction {
    WorkspaceLeft,
    WorkspaceRight,
    ShowOverview,
    ShowDesktop,
    ShowLauncher,
    ShowNotifications,
    NavigateBack,
    NavigateForward,
    ZoomIn,
    ZoomOut,
    ScrollUp,
    ScrollDown,
    None,
    Custom(String),
}

/// Mapping from gesture to action
#[derive(Debug, Clone)]
pub struct GestureBinding {
    pub three_finger_left: GestureAction,
    pub three_finger_right: GestureAction,
    pub three_finger_up: GestureAction,
    pub three_finger_down: GestureAction,
    pub four_finger_left: GestureAction,
    pub four_finger_right: GestureAction,
    pub four_finger_up: GestureAction,
    pub four_finger_down: GestureAction,
    pub pinch_in: GestureAction,
    pub pinch_out: GestureAction,
    pub edge_left: GestureAction,
    pub edge_right: GestureAction,
    pub edge_top: GestureAction,
    pub edge_bottom: GestureAction,
}

impl Default for GestureBinding {
    fn default() -> Self {
        Self {
            three_finger_left: GestureAction::WorkspaceRight,
            three_finger_right: GestureAction::WorkspaceLeft,
            three_finger_up: GestureAction::ShowOverview,
            three_finger_down: GestureAction::ShowDesktop,
            four_finger_left: GestureAction::WorkspaceRight,
            four_finger_right: GestureAction::WorkspaceLeft,
            four_finger_up: GestureAction::ShowLauncher,
            four_finger_down: GestureAction::None,
            pinch_in: GestureAction::ShowDesktop,
            pinch_out: GestureAction::ShowOverview,
            edge_left: GestureAction::NavigateBack,
            edge_right: GestureAction::NavigateForward,
            edge_top: GestureAction::ShowNotifications,
            edge_bottom: GestureAction::None,
        }
    }
}

impl GestureBinding {
    /// Map a gesture event to an action
    pub fn map_gesture(&self, event: &GestureEvent) -> GestureAction {
        match event {
            GestureEvent::ThreeFingerSwipe {
                direction, phase, ..
            } if *phase == GesturePhase::Ended => match direction {
                SwipeDirection::Left => self.three_finger_left.clone(),
                SwipeDirection::Right => self.three_finger_right.clone(),
                SwipeDirection::Up => self.three_finger_up.clone(),
                SwipeDirection::Down => self.three_finger_down.clone(),
            },
            GestureEvent::FourFingerSwipe {
                direction, phase, ..
            } if *phase == GesturePhase::Ended => match direction {
                SwipeDirection::Left => self.four_finger_left.clone(),
                SwipeDirection::Right => self.four_finger_right.clone(),
                SwipeDirection::Up => self.four_finger_up.clone(),
                SwipeDirection::Down => self.four_finger_down.clone(),
            },
            GestureEvent::Pinch { scale, phase, .. }
                if *phase == GesturePhase::Ended =>
            {
                if *scale < 0.7 {
                    self.pinch_in.clone()
                } else if *scale > 1.3 {
                    self.pinch_out.clone()
                } else {
                    GestureAction::None
                }
            }
            GestureEvent::EdgeSwipe {
                edge,
                phase,
                progress,
                ..
            } if *phase == GesturePhase::Ended && *progress > 0.3 => match edge {
                Edge::Left => self.edge_left.clone(),
                Edge::Right => self.edge_right.clone(),
                Edge::Top => self.edge_top.clone(),
                Edge::Bottom => self.edge_bottom.clone(),
            },
            _ => GestureAction::None,
        }
    }
}

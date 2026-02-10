//! Input event routing to surfaces via hit-testing and focus/grab tracking.

use liquide_compositor::geometry::{Point, Rect};

use crate::event::InputEvent;
use crate::mouse::MouseEvent;

/// Result of a hit test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitTestResult {
    pub surface_id: u64,
    pub local_x: f32,
    pub local_y: f32,
}

/// Trait for surfaces that can receive input.
pub trait InputTarget {
    /// The unique surface ID.
    fn id(&self) -> u64;

    /// The bounding rectangle in screen space.
    fn bounds(&self) -> Rect;

    /// Hit test: check if a point is within this surface.
    fn hit_test(&self, x: f32, y: f32) -> bool {
        self.bounds().contains(Point::new(x, y))
    }
}

/// Grab modes for input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabMode {
    None,
    Keyboard { surface_id: u64 },
    Pointer { surface_id: u64 },
    Full { surface_id: u64 },
}

impl GrabMode {
    /// Get the surface ID associated with this grab, if any.
    #[must_use]
    pub fn surface_id(&self) -> Option<u64> {
        match self {
            Self::None => None,
            Self::Keyboard { surface_id } | Self::Pointer { surface_id } | Self::Full { surface_id } => Some(*surface_id),
        }
    }
}

impl std::fmt::Display for GrabMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Keyboard { surface_id } => write!(f, "Keyboard(surface={surface_id})"),
            Self::Pointer { surface_id } => write!(f, "Pointer(surface={surface_id})"),
            Self::Full { surface_id } => write!(f, "Full(surface={surface_id})"),
        }
    }
}

/// Routes input events to surfaces based on focus, grab, and hit-testing.
pub struct InputRouter {
    focused_surface: Option<u64>,
    grab: GrabMode,
}

impl InputRouter {
    /// Create a new input router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focused_surface: None,
            grab: GrabMode::None,
        }
    }

    /// Set the focused surface.
    pub fn set_focus(&mut self, surface_id: u64) {
        self.focused_surface = Some(surface_id);
    }

    /// Get the currently focused surface.
    #[must_use]
    pub fn focused(&self) -> Option<u64> {
        self.focused_surface
    }

    /// Clear the focused surface.
    pub fn clear_focus(&mut self) {
        self.focused_surface = None;
    }

    /// Get the current grab mode.
    #[must_use]
    pub fn grab(&self) -> GrabMode {
        self.grab
    }

    /// Set grab mode.
    pub fn set_grab(&mut self, mode: GrabMode) {
        self.grab = mode;
    }

    /// Release any active grab.
    pub fn release_grab(&mut self) {
        self.grab = GrabMode::None;
    }

    /// Route an input event to the appropriate surface.
    ///
    /// Returns the target surface ID and the (possibly transformed) event,
    /// or `None` if no target is found.
    #[must_use]
    pub fn route(&self, event: &InputEvent, surfaces: &[&dyn InputTarget]) -> Option<(u64, InputEvent)> {
        if surfaces.is_empty() {
            return None;
        }

        match event {
            InputEvent::Keyboard(_) => {
                // Keyboard events go to keyboard grab or focused surface
                let target = match self.grab {
                    GrabMode::Keyboard { surface_id } | GrabMode::Full { surface_id } => {
                        Some(surface_id)
                    }
                    _ => self.focused_surface,
                };
                target.map(|id| (id, *event))
            }
            InputEvent::Mouse(me) => {
                // Pointer grab overrides hit-testing
                match self.grab {
                    GrabMode::Pointer { surface_id } | GrabMode::Full { surface_id } => {
                        return Some((surface_id, *event));
                    }
                    _ => {}
                }

                // Hit-test for mouse position
                let (x, y) = mouse_position(me);
                if let Some((x_pos, y_pos)) = x.zip(y) {
                    // Test surfaces in reverse order (top-most first)
                    for &surface in surfaces.iter().rev() {
                        if surface.hit_test(x_pos, y_pos) {
                            return Some((surface.id(), *event));
                        }
                    }
                }

                // Fall back to focused surface
                self.focused_surface.map(|id| (id, *event))
            }
            InputEvent::Touch(te) => {
                // Touch: grab or hit-test
                match self.grab {
                    GrabMode::Pointer { surface_id } | GrabMode::Full { surface_id } => {
                        return Some((surface_id, *event));
                    }
                    _ => {}
                }

                for &surface in surfaces.iter().rev() {
                    if surface.hit_test(te.point.x, te.point.y) {
                        return Some((surface.id(), *event));
                    }
                }
                self.focused_surface.map(|id| (id, *event))
            }
        }
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract mouse position from a mouse event.
fn mouse_position(event: &MouseEvent) -> (Option<f32>, Option<f32>) {
    match event {
        MouseEvent::Move { x, y }
        | MouseEvent::Button { x, y, .. }
        | MouseEvent::Scroll { x, y, .. }
        | MouseEvent::Enter { x, y } => (Some(*x), Some(*y)),
        MouseEvent::Leave => (None, None),
    }
}

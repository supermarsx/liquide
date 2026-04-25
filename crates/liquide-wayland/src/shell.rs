//! Shell surface roles (xdg_shell equivalent).
//!
//! Implements `xdg_surface`, `xdg_toplevel`, and `xdg_popup` roles as
//! defined by the xdg-shell protocol specification. These roles give
//! surfaces window-management semantics.

use crate::protocol::ObjectId;
use bitflags::bitflags;

// ---------------------------------------------------------------------------
// Configure serial tracking
// ---------------------------------------------------------------------------

/// A configure serial number, used to synchronize state between compositor
/// and client. The client must `ack_configure` with the serial before
/// committing the corresponding state.
pub type Serial = u32;

/// A pending configure event waiting for acknowledgement.
#[derive(Debug, Clone)]
pub struct ConfigureEvent {
    /// Serial number.
    pub serial: Serial,
    /// Suggested width (0 = client decides).
    pub width: i32,
    /// Suggested height (0 = client decides).
    pub height: i32,
    /// State flags for toplevels.
    pub states: ToplevelState,
}

// ---------------------------------------------------------------------------
// ToplevelState
// ---------------------------------------------------------------------------

bitflags! {
    /// State flags for an xdg_toplevel.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ToplevelState: u32 {
        const MAXIMIZED   = 1 << 0;
        const FULLSCREEN  = 1 << 1;
        const RESIZING    = 1 << 2;
        const ACTIVATED   = 1 << 3;
        const TILED_LEFT  = 1 << 4;
        const TILED_RIGHT = 1 << 5;
        const TILED_TOP   = 1 << 6;
        const TILED_BOTTOM = 1 << 7;
        const SUSPENDED   = 1 << 8;
    }
}

// ---------------------------------------------------------------------------
// XdgSurface
// ---------------------------------------------------------------------------

/// An xdg_surface: a surface with a window-management role.
///
/// This is the base for both toplevel and popup surfaces. It tracks
/// window geometry and configure serials.
#[derive(Debug)]
pub struct XdgSurface {
    /// The protocol object ID of this xdg_surface.
    id: ObjectId,
    /// The underlying wl_surface.
    surface_id: ObjectId,
    /// User-set window geometry (x, y, width, height).
    window_geometry: Option<(i32, i32, i32, i32)>,
    /// The last configure serial sent.
    last_configure_serial: Serial,
    /// Whether the last configure has been acknowledged.
    configured: bool,
    /// The role assigned to this surface.
    role: XdgRole,
}

/// The role assigned to an xdg_surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdgRole {
    /// No role assigned yet.
    None,
    /// Toplevel window.
    Toplevel,
    /// Popup window.
    Popup,
}

impl XdgSurface {
    /// Create a new xdg_surface wrapping the given wl_surface.
    pub fn new(id: ObjectId, surface_id: ObjectId) -> Self {
        Self {
            id,
            surface_id,
            window_geometry: None,
            last_configure_serial: 0,
            configured: false,
            role: XdgRole::None,
        }
    }

    /// The xdg_surface object ID.
    pub fn id(&self) -> ObjectId {
        self.id
    }

    /// The underlying wl_surface ID.
    pub fn surface_id(&self) -> ObjectId {
        self.surface_id
    }

    /// The current role.
    pub fn role(&self) -> XdgRole {
        self.role
    }

    /// Set the window geometry.
    pub fn set_window_geometry(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.window_geometry = Some((x, y, width, height));
    }

    /// Get the window geometry.
    pub fn window_geometry(&self) -> Option<(i32, i32, i32, i32)> {
        self.window_geometry
    }

    /// Acknowledge a configure event.
    ///
    /// Returns `true` if the serial matches the last sent configure.
    pub fn ack_configure(&mut self, serial: Serial) -> bool {
        if serial == self.last_configure_serial {
            self.configured = true;
            true
        } else {
            false
        }
    }

    /// Whether the surface has been configured and acknowledged.
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// Send a configure event and return the serial.
    pub fn send_configure(&mut self, serial: Serial) -> Serial {
        self.last_configure_serial = serial;
        self.configured = false;
        serial
    }

    /// Create a toplevel role for this surface.
    ///
    /// Returns `None` if a role is already assigned.
    pub fn get_toplevel(&mut self, toplevel_id: ObjectId) -> Option<XdgToplevel> {
        if self.role != XdgRole::None {
            return None;
        }
        self.role = XdgRole::Toplevel;
        Some(XdgToplevel::new(toplevel_id, self.id))
    }

    /// Create a popup role for this surface.
    ///
    /// Returns `None` if a role is already assigned.
    pub fn get_popup(&mut self, popup_id: ObjectId, parent_id: ObjectId) -> Option<XdgPopup> {
        if self.role != XdgRole::None {
            return None;
        }
        self.role = XdgRole::Popup;
        Some(XdgPopup::new(popup_id, self.id, parent_id))
    }
}

// ---------------------------------------------------------------------------
// XdgToplevel
// ---------------------------------------------------------------------------

/// Resize edge flags for interactive resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// An xdg_toplevel: a top-level application window.
#[derive(Debug)]
pub struct XdgToplevel {
    /// The protocol object ID.
    id: ObjectId,
    /// The parent xdg_surface.
    xdg_surface_id: ObjectId,
    /// Window title.
    title: String,
    /// Application ID.
    app_id: String,
    /// Minimum size (0 = no minimum).
    min_size: (i32, i32),
    /// Maximum size (0 = no maximum).
    max_size: (i32, i32),
    /// Current state flags.
    states: ToplevelState,
    /// Parent toplevel (for dialog windows).
    parent_toplevel: Option<ObjectId>,
    /// Pending configure events.
    pending_configures: Vec<ConfigureEvent>,
    /// Next serial for configure events.
    next_serial: Serial,
}

impl XdgToplevel {
    /// Create a new toplevel.
    pub fn new(id: ObjectId, xdg_surface_id: ObjectId) -> Self {
        Self {
            id,
            xdg_surface_id,
            title: String::new(),
            app_id: String::new(),
            min_size: (0, 0),
            max_size: (0, 0),
            states: ToplevelState::empty(),
            parent_toplevel: None,
            pending_configures: Vec::new(),
            next_serial: 1,
        }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn xdg_surface_id(&self) -> ObjectId {
        self.xdg_surface_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    pub fn min_size(&self) -> (i32, i32) {
        self.min_size
    }

    pub fn max_size(&self) -> (i32, i32) {
        self.max_size
    }

    pub fn states(&self) -> ToplevelState {
        self.states
    }

    pub fn parent_toplevel(&self) -> Option<ObjectId> {
        self.parent_toplevel
    }

    /// Set the window title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Set the application identifier.
    pub fn set_app_id(&mut self, app_id: impl Into<String>) {
        self.app_id = app_id.into();
    }

    /// Set the minimum size. Zero means unconstrained.
    pub fn set_min_size(&mut self, width: i32, height: i32) {
        self.min_size = (width, height);
    }

    /// Set the maximum size. Zero means unconstrained.
    pub fn set_max_size(&mut self, width: i32, height: i32) {
        self.max_size = (width, height);
    }

    /// Set the parent toplevel (for dialogs).
    pub fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.parent_toplevel = parent;
    }

    /// Request an interactive move.
    ///
    /// Returns the serial for the move operation.
    pub fn move_request(&self, _seat_id: ObjectId, serial: Serial) -> Serial {
        serial
    }

    /// Request an interactive resize.
    ///
    /// Returns the edge and serial.
    pub fn resize_request(
        &self,
        _seat_id: ObjectId,
        serial: Serial,
        edge: ResizeEdge,
    ) -> (ResizeEdge, Serial) {
        (edge, serial)
    }

    /// Request fullscreen mode.
    pub fn set_fullscreen(&mut self, _output: Option<ObjectId>) {
        self.states.insert(ToplevelState::FULLSCREEN);
    }

    /// Exit fullscreen mode.
    pub fn unset_fullscreen(&mut self) {
        self.states.remove(ToplevelState::FULLSCREEN);
    }

    /// Request maximized mode.
    pub fn set_maximized(&mut self) {
        self.states.insert(ToplevelState::MAXIMIZED);
    }

    /// Exit maximized mode.
    pub fn unset_maximized(&mut self) {
        self.states.remove(ToplevelState::MAXIMIZED);
    }

    /// Set the activated state.
    pub fn set_activated(&mut self, activated: bool) {
        if activated {
            self.states.insert(ToplevelState::ACTIVATED);
        } else {
            self.states.remove(ToplevelState::ACTIVATED);
        }
    }

    /// Set the resizing state.
    pub fn set_resizing(&mut self, resizing: bool) {
        if resizing {
            self.states.insert(ToplevelState::RESIZING);
        } else {
            self.states.remove(ToplevelState::RESIZING);
        }
    }

    /// Set tiling state flags.
    pub fn set_tiled(&mut self, left: bool, right: bool, top: bool, bottom: bool) {
        self.states.set(ToplevelState::TILED_LEFT, left);
        self.states.set(ToplevelState::TILED_RIGHT, right);
        self.states.set(ToplevelState::TILED_TOP, top);
        self.states.set(ToplevelState::TILED_BOTTOM, bottom);
    }

    /// Generate and record a configure event.
    ///
    /// Returns the configure event that should be sent to the client.
    pub fn configure(&mut self, width: i32, height: i32) -> ConfigureEvent {
        let serial = self.next_serial;
        self.next_serial += 1;
        let event = ConfigureEvent {
            serial,
            width,
            height,
            states: self.states,
        };
        self.pending_configures.push(event.clone());
        event
    }

    /// Close the toplevel.
    ///
    /// Returns `true` to indicate a close event should be sent.
    pub fn close(&self) -> bool {
        true
    }

    /// Get all pending configure events.
    pub fn pending_configures(&self) -> &[ConfigureEvent] {
        &self.pending_configures
    }

    /// Clear pending configures up to and including the given serial.
    pub fn ack_configure(&mut self, serial: Serial) {
        self.pending_configures.retain(|c| c.serial > serial);
    }
}

// ---------------------------------------------------------------------------
// Anchor / Gravity / ConstraintAdjustment (for popups)
// ---------------------------------------------------------------------------

/// Anchor point on the parent surface for popup positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for Anchor {
    fn default() -> Self {
        Self::None
    }
}

/// Gravity of the popup relative to the anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravity {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Default for Gravity {
    fn default() -> Self {
        Self::None
    }
}

bitflags! {
    /// Constraint adjustment flags for popup repositioning.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ConstraintAdjustment: u32 {
        const SLIDE_X  = 1 << 0;
        const SLIDE_Y  = 1 << 1;
        const FLIP_X   = 1 << 2;
        const FLIP_Y   = 1 << 3;
        const RESIZE_X = 1 << 4;
        const RESIZE_Y = 1 << 5;
    }
}

/// Positioning rules for a popup.
#[derive(Debug, Clone)]
pub struct PopupPositioner {
    /// Anchor rectangle on parent surface (x, y, width, height).
    pub anchor_rect: (i32, i32, i32, i32),
    /// Size of the popup (width, height).
    pub size: (i32, i32),
    /// Anchor edge on the anchor rectangle.
    pub anchor: Anchor,
    /// Gravity of the popup surface.
    pub gravity: Gravity,
    /// Offset from computed position (x, y).
    pub offset: (i32, i32),
    /// Constraint adjustment flags.
    pub constraint_adjustment: ConstraintAdjustment,
}

impl Default for PopupPositioner {
    fn default() -> Self {
        Self {
            anchor_rect: (0, 0, 0, 0),
            size: (0, 0),
            anchor: Anchor::None,
            gravity: Gravity::None,
            offset: (0, 0),
            constraint_adjustment: ConstraintAdjustment::empty(),
        }
    }
}

impl PopupPositioner {
    /// Compute the anchor point on the anchor rectangle.
    pub fn anchor_point(&self) -> (i32, i32) {
        let (ax, ay, aw, ah) = self.anchor_rect;
        let cx = ax + aw / 2;
        let cy = ay + ah / 2;
        match self.anchor {
            Anchor::None => (cx, cy),
            Anchor::Top => (cx, ay),
            Anchor::Bottom => (cx, ay + ah),
            Anchor::Left => (ax, cy),
            Anchor::Right => (ax + aw, cy),
            Anchor::TopLeft => (ax, ay),
            Anchor::TopRight => (ax + aw, ay),
            Anchor::BottomLeft => (ax, ay + ah),
            Anchor::BottomRight => (ax + aw, ay + ah),
        }
    }

    /// Compute the popup position based on anchor, gravity, and offset.
    pub fn compute_position(&self) -> (i32, i32) {
        let (ax, ay) = self.anchor_point();
        let (pw, ph) = self.size;

        let (gx, gy) = match self.gravity {
            Gravity::None => (-pw / 2, -ph / 2),
            Gravity::Top => (-pw / 2, -ph),
            Gravity::Bottom => (-pw / 2, 0),
            Gravity::Left => (-pw, -ph / 2),
            Gravity::Right => (0, -ph / 2),
            Gravity::TopLeft => (-pw, -ph),
            Gravity::TopRight => (0, -ph),
            Gravity::BottomLeft => (-pw, 0),
            Gravity::BottomRight => (0, 0),
        };

        (ax + gx + self.offset.0, ay + gy + self.offset.1)
    }
}

// ---------------------------------------------------------------------------
// XdgPopup
// ---------------------------------------------------------------------------

/// An xdg_popup: a transient surface anchored to a parent.
#[derive(Debug)]
pub struct XdgPopup {
    /// Protocol object ID.
    id: ObjectId,
    /// The parent xdg_surface.
    xdg_surface_id: ObjectId,
    /// The parent xdg_surface that this popup is anchored to.
    parent_xdg_surface_id: ObjectId,
    /// Positioning rules.
    positioner: PopupPositioner,
    /// Whether the popup has an explicit grab.
    grabbed: bool,
    /// Configure serial tracking.
    pending_configures: Vec<ConfigureEvent>,
    /// Next serial.
    next_serial: Serial,
}

impl XdgPopup {
    /// Create a new popup.
    pub fn new(id: ObjectId, xdg_surface_id: ObjectId, parent_id: ObjectId) -> Self {
        Self {
            id,
            xdg_surface_id,
            parent_xdg_surface_id: parent_id,
            positioner: PopupPositioner::default(),
            grabbed: false,
            pending_configures: Vec::new(),
            next_serial: 1,
        }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn xdg_surface_id(&self) -> ObjectId {
        self.xdg_surface_id
    }

    pub fn parent_xdg_surface_id(&self) -> ObjectId {
        self.parent_xdg_surface_id
    }

    pub fn positioner(&self) -> &PopupPositioner {
        &self.positioner
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    /// Set the positioner.
    pub fn set_positioner(&mut self, positioner: PopupPositioner) {
        self.positioner = positioner;
    }

    /// Grab the popup (for menus that dismiss on click outside).
    pub fn grab(&mut self, _seat_id: ObjectId, _serial: Serial) {
        self.grabbed = true;
    }

    /// Generate a configure event for this popup.
    pub fn configure(&mut self, x: i32, y: i32, width: i32, height: i32) -> ConfigureEvent {
        let serial = self.next_serial;
        self.next_serial += 1;
        let event = ConfigureEvent {
            serial,
            width,
            height,
            states: ToplevelState::empty(),
        };
        let _ = (x, y); // Position is implicit in the configure
        self.pending_configures.push(event.clone());
        event
    }

    /// Acknowledge a configure event.
    pub fn ack_configure(&mut self, serial: Serial) {
        self.pending_configures.retain(|c| c.serial > serial);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_surface_creation() {
        let s = XdgSurface::new(ObjectId(10), ObjectId(5));
        assert_eq!(s.id(), ObjectId(10));
        assert_eq!(s.surface_id(), ObjectId(5));
        assert_eq!(s.role(), XdgRole::None);
        assert!(!s.is_configured());
    }

    #[test]
    fn xdg_surface_window_geometry() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        assert!(s.window_geometry().is_none());
        s.set_window_geometry(0, 0, 800, 600);
        assert_eq!(s.window_geometry(), Some((0, 0, 800, 600)));
    }

    #[test]
    fn xdg_surface_configure_ack() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        let serial = s.send_configure(1);
        assert!(!s.is_configured());
        assert!(s.ack_configure(serial));
        assert!(s.is_configured());
    }

    #[test]
    fn xdg_surface_configure_wrong_serial() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        s.send_configure(1);
        assert!(!s.ack_configure(999)); // wrong serial
        assert!(!s.is_configured());
    }

    #[test]
    fn xdg_surface_get_toplevel() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        let tl = s.get_toplevel(ObjectId(20));
        assert!(tl.is_some());
        assert_eq!(s.role(), XdgRole::Toplevel);

        // Cannot assign a second role
        let tl2 = s.get_toplevel(ObjectId(21));
        assert!(tl2.is_none());
    }

    #[test]
    fn xdg_surface_get_popup() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        let popup = s.get_popup(ObjectId(30), ObjectId(1));
        assert!(popup.is_some());
        assert_eq!(s.role(), XdgRole::Popup);
    }

    #[test]
    fn xdg_surface_role_conflict() {
        let mut s = XdgSurface::new(ObjectId(10), ObjectId(5));
        s.get_toplevel(ObjectId(20));
        let popup = s.get_popup(ObjectId(30), ObjectId(1));
        assert!(popup.is_none()); // already has toplevel role
    }

    #[test]
    fn toplevel_title_and_app_id() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_title("My Window");
        tl.set_app_id("org.example.app");
        assert_eq!(tl.title(), "My Window");
        assert_eq!(tl.app_id(), "org.example.app");
    }

    #[test]
    fn toplevel_min_max_size() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_min_size(200, 100);
        tl.set_max_size(1920, 1080);
        assert_eq!(tl.min_size(), (200, 100));
        assert_eq!(tl.max_size(), (1920, 1080));
    }

    #[test]
    fn toplevel_states() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        assert!(tl.states().is_empty());

        tl.set_maximized();
        assert!(tl.states().contains(ToplevelState::MAXIMIZED));

        tl.set_fullscreen(None);
        assert!(tl.states().contains(ToplevelState::FULLSCREEN));

        tl.unset_maximized();
        assert!(!tl.states().contains(ToplevelState::MAXIMIZED));
        assert!(tl.states().contains(ToplevelState::FULLSCREEN));

        tl.unset_fullscreen();
        assert!(tl.states().is_empty());
    }

    #[test]
    fn toplevel_activated() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_activated(true);
        assert!(tl.states().contains(ToplevelState::ACTIVATED));
        tl.set_activated(false);
        assert!(!tl.states().contains(ToplevelState::ACTIVATED));
    }

    #[test]
    fn toplevel_tiled() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_tiled(true, false, true, false);
        assert!(tl.states().contains(ToplevelState::TILED_LEFT));
        assert!(!tl.states().contains(ToplevelState::TILED_RIGHT));
        assert!(tl.states().contains(ToplevelState::TILED_TOP));
        assert!(!tl.states().contains(ToplevelState::TILED_BOTTOM));
    }

    #[test]
    fn toplevel_configure_sequence() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_maximized();
        tl.set_activated(true);

        let cfg = tl.configure(1920, 1080);
        assert_eq!(cfg.serial, 1);
        assert_eq!(cfg.width, 1920);
        assert_eq!(cfg.height, 1080);
        assert!(cfg.states.contains(ToplevelState::MAXIMIZED));
        assert!(cfg.states.contains(ToplevelState::ACTIVATED));

        assert_eq!(tl.pending_configures().len(), 1);
        tl.ack_configure(1);
        assert!(tl.pending_configures().is_empty());
    }

    #[test]
    fn toplevel_multiple_configures() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.configure(800, 600);
        tl.configure(1024, 768);
        tl.configure(1920, 1080);
        assert_eq!(tl.pending_configures().len(), 3);

        // Ack serial 2 should clear 1 and 2
        tl.ack_configure(2);
        assert_eq!(tl.pending_configures().len(), 1);
        assert_eq!(tl.pending_configures()[0].serial, 3);
    }

    #[test]
    fn toplevel_parent() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        assert!(tl.parent_toplevel().is_none());
        tl.set_parent(Some(ObjectId(15)));
        assert_eq!(tl.parent_toplevel(), Some(ObjectId(15)));
    }

    #[test]
    fn toplevel_close() {
        let tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        assert!(tl.close());
    }

    #[test]
    fn toplevel_resizing_state() {
        let mut tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        tl.set_resizing(true);
        assert!(tl.states().contains(ToplevelState::RESIZING));
        tl.set_resizing(false);
        assert!(!tl.states().contains(ToplevelState::RESIZING));
    }

    #[test]
    fn popup_creation() {
        let popup = XdgPopup::new(ObjectId(30), ObjectId(10), ObjectId(5));
        assert_eq!(popup.id(), ObjectId(30));
        assert_eq!(popup.xdg_surface_id(), ObjectId(10));
        assert_eq!(popup.parent_xdg_surface_id(), ObjectId(5));
        assert!(!popup.is_grabbed());
    }

    #[test]
    fn popup_grab() {
        let mut popup = XdgPopup::new(ObjectId(30), ObjectId(10), ObjectId(5));
        popup.grab(ObjectId(1), 1);
        assert!(popup.is_grabbed());
    }

    #[test]
    fn popup_positioner_anchor_center() {
        let pos = PopupPositioner {
            anchor_rect: (100, 200, 50, 30),
            size: (120, 80),
            anchor: Anchor::None,
            gravity: Gravity::None,
            offset: (0, 0),
            constraint_adjustment: ConstraintAdjustment::empty(),
        };
        // Center of anchor rect: (125, 215)
        assert_eq!(pos.anchor_point(), (125, 215));
        // Gravity None centers the popup: (125-60, 215-40) = (65, 175)
        assert_eq!(pos.compute_position(), (65, 175));
    }

    #[test]
    fn popup_positioner_bottom_right() {
        let pos = PopupPositioner {
            anchor_rect: (0, 0, 100, 50),
            size: (200, 100),
            anchor: Anchor::BottomRight,
            gravity: Gravity::BottomRight,
            offset: (5, 5),
            constraint_adjustment: ConstraintAdjustment::empty(),
        };
        assert_eq!(pos.anchor_point(), (100, 50));
        // Gravity BottomRight: offset (0, 0) + anchor (100, 50) + user offset (5, 5)
        assert_eq!(pos.compute_position(), (105, 55));
    }

    #[test]
    fn popup_positioner_top_left() {
        let pos = PopupPositioner {
            anchor_rect: (50, 50, 100, 100),
            size: (80, 60),
            anchor: Anchor::TopLeft,
            gravity: Gravity::TopLeft,
            offset: (0, 0),
            constraint_adjustment: ConstraintAdjustment::empty(),
        };
        assert_eq!(pos.anchor_point(), (50, 50));
        // TopLeft gravity: (-pw, -ph) = (-80, -60)
        assert_eq!(pos.compute_position(), (-30, -10));
    }

    #[test]
    fn popup_configure() {
        let mut popup = XdgPopup::new(ObjectId(30), ObjectId(10), ObjectId(5));
        let cfg = popup.configure(50, 100, 200, 150);
        assert_eq!(cfg.serial, 1);
        assert_eq!(cfg.width, 200);
        assert_eq!(cfg.height, 150);

        popup.ack_configure(1);
        assert!(popup.pending_configures.is_empty());
    }

    #[test]
    fn toplevel_move_resize_requests() {
        let tl = XdgToplevel::new(ObjectId(20), ObjectId(10));
        let serial = tl.move_request(ObjectId(1), 42);
        assert_eq!(serial, 42);

        let (edge, s) = tl.resize_request(ObjectId(1), 43, ResizeEdge::BottomRight);
        assert_eq!(edge, ResizeEdge::BottomRight);
        assert_eq!(s, 43);
    }
}

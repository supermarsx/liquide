//! X11 window mapping into the compositor scene graph.

/// Unique identifier for an X11 window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X11WindowId(pub u32);

/// X11 window type (from `_NET_WM_WINDOW_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X11WindowType {
    Normal,
    Dialog,
    Utility,
    Toolbar,
    Splash,
    Dropdown,
    Popup,
    Tooltip,
    Notification,
    Combo,
    Dnd,
    Desktop,
    Dock,
    Unknown,
}

/// X11 window map state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X11WindowState {
    Unmapped,
    Mapped,
    Iconified,
    Withdrawn,
}

/// An X11 window tracked by the XWayland bridge.
#[derive(Debug)]
pub struct X11Window {
    id: X11WindowId,
    parent_id: Option<X11WindowId>,
    window_type: X11WindowType,
    state: X11WindowState,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    override_redirect: bool,
    title: String,
    wm_class: String,
    /// Corresponding Wayland surface id, if mapped.
    surface_id: Option<u32>,
    mapped: bool,
}

impl X11Window {
    /// Create a new X11 window with the given geometry.
    pub fn new(id: X11WindowId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            parent_id: None,
            window_type: X11WindowType::Normal,
            state: X11WindowState::Unmapped,
            x,
            y,
            width,
            height,
            override_redirect: false,
            title: String::new(),
            wm_class: String::new(),
            surface_id: None,
            mapped: false,
        }
    }

    pub fn id(&self) -> X11WindowId {
        self.id
    }

    pub fn parent_id(&self) -> Option<X11WindowId> {
        self.parent_id
    }

    pub fn set_parent_id(&mut self, parent: Option<X11WindowId>) {
        self.parent_id = parent;
    }

    pub fn window_type(&self) -> X11WindowType {
        self.window_type
    }

    pub fn set_window_type(&mut self, wt: X11WindowType) {
        self.window_type = wt;
    }

    pub fn state(&self) -> X11WindowState {
        self.state
    }

    pub fn set_state(&mut self, state: X11WindowState) {
        self.state = state;
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn set_geometry(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn override_redirect(&self) -> bool {
        self.override_redirect
    }

    pub fn set_override_redirect(&mut self, or: bool) {
        self.override_redirect = or;
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn wm_class(&self) -> &str {
        &self.wm_class
    }

    pub fn set_wm_class(&mut self, wm_class: String) {
        self.wm_class = wm_class;
    }

    pub fn surface_id(&self) -> Option<u32> {
        self.surface_id
    }

    pub fn set_surface_id(&mut self, id: Option<u32>) {
        self.surface_id = id;
    }

    pub fn mapped(&self) -> bool {
        self.mapped
    }

    pub fn set_mapped(&mut self, mapped: bool) {
        self.mapped = mapped;
    }
}

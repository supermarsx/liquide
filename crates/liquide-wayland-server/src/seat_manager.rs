use liquide_wayland::SeatCapability;

#[derive(Debug)]
pub struct SeatManager {
    capabilities: SeatCapability,
    keyboard_focused: Option<u32>,
    pointer_focused: Option<u32>,
    pointer_x: f64,
    pointer_y: f64,
}

impl SeatManager {
    pub fn new() -> Self {
        Self {
            capabilities: SeatCapability::POINTER | SeatCapability::KEYBOARD,
            keyboard_focused: None,
            pointer_focused: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
        }
    }

    pub fn set_keyboard_focus(&mut self, surface_id: Option<u32>) {
        self.keyboard_focused = surface_id;
    }

    pub fn set_pointer_focus(&mut self, surface_id: Option<u32>) {
        self.pointer_focused = surface_id;
    }

    pub fn update_pointer(&mut self, x: f64, y: f64) {
        self.pointer_x = x;
        self.pointer_y = y;
    }

    pub fn keyboard_focused(&self) -> Option<u32> {
        self.keyboard_focused
    }

    pub fn pointer_focused(&self) -> Option<u32> {
        self.pointer_focused
    }

    pub fn pointer_position(&self) -> (f64, f64) {
        (self.pointer_x, self.pointer_y)
    }

    pub fn capabilities(&self) -> SeatCapability {
        self.capabilities
    }

    pub fn set_capabilities(&mut self, caps: SeatCapability) {
        self.capabilities = caps;
    }
}

impl Default for SeatManager {
    fn default() -> Self {
        Self::new()
    }
}

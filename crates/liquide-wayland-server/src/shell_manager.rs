use std::collections::HashMap;

use liquide_wayland::ToplevelState;

/// State for an XDG toplevel window.
#[derive(Debug)]
pub struct ToplevelInfo {
    pub surface_id: u32,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub states: ToplevelState,
    pub configured: bool,
    pub pending_width: u32,
    pub pending_height: u32,
    /// Serials of configure events sent to this surface that the client has
    /// not yet acknowledged, in ascending (send) order. Per xdg-shell, an
    /// `ack_configure` for serial `S` acknowledges that configure and discards
    /// every older pending serial for this surface.
    pub pending_configures: Vec<u32>,
}

/// State for an XDG popup surface.
#[derive(Debug)]
pub struct PopupState {
    pub surface_id: u32,
    pub parent_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Manages XDG shell toplevel and popup surfaces.
#[derive(Debug)]
pub struct ShellManager {
    toplevels: HashMap<u32, ToplevelInfo>,
    popups: HashMap<u32, PopupState>,
    next_serial: u32,
}

impl ShellManager {
    pub fn new() -> Self {
        Self {
            toplevels: HashMap::new(),
            popups: HashMap::new(),
            next_serial: 1,
        }
    }

    pub fn create_toplevel(&mut self, surface_id: u32) -> u32 {
        let id = surface_id;
        self.toplevels.insert(
            id,
            ToplevelInfo {
                surface_id,
                title: None,
                app_id: None,
                states: ToplevelState::empty(),
                configured: false,
                pending_width: 0,
                pending_height: 0,
                pending_configures: Vec::new(),
            },
        );
        id
    }

    pub fn configure_toplevel(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        states: ToplevelState,
    ) -> u32 {
        let serial = self.next_serial;
        self.next_serial += 1;
        if let Some(tl) = self.toplevels.get_mut(&id) {
            tl.pending_width = width;
            tl.pending_height = height;
            tl.states = states;
            tl.pending_configures.push(serial);
        }
        serial
    }

    /// Acknowledge a configure event for a specific surface.
    ///
    /// Per xdg-shell semantics, the client acks a *specific* configure by its
    /// serial on a *specific* surface. We mark only that surface as configured
    /// (and only if it actually had `serial` pending), discarding that serial
    /// and every older pending serial for that surface. Other surfaces — and
    /// other clients — are left untouched. Unknown serials are ignored.
    ///
    /// Returns `true` if a matching pending configure was found and acked.
    pub fn ack_configure(&mut self, surface_id: u32, serial: u32) -> bool {
        let Some(tl) = self.toplevels.get_mut(&surface_id) else {
            return false;
        };
        // Only honor a serial this surface was actually waiting on.
        if !tl.pending_configures.contains(&serial) {
            return false;
        }
        // Drop the acked serial and every older one for this surface.
        tl.pending_configures.retain(|&s| s > serial);
        tl.configured = true;
        true
    }

    pub fn destroy_toplevel(&mut self, id: u32) -> Option<ToplevelInfo> {
        self.toplevels.remove(&id)
    }

    pub fn get_toplevel(&self, id: u32) -> Option<&ToplevelInfo> {
        self.toplevels.get(&id)
    }

    pub fn toplevel_count(&self) -> usize {
        self.toplevels.len()
    }

    pub fn create_popup(&mut self, surface_id: u32, parent_id: u32) -> u32 {
        let id = surface_id;
        self.popups.insert(
            id,
            PopupState {
                surface_id,
                parent_id,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        );
        id
    }

    pub fn destroy_popup(&mut self, id: u32) -> Option<PopupState> {
        self.popups.remove(&id)
    }
}

impl Default for ShellManager {
    fn default() -> Self {
        Self::new()
    }
}

use std::collections::HashMap;

use crate::buffer::BufferRef;
use crate::client::ClientId;
use crate::error::{Result, WaylandServerError};

/// Role assigned to a Wayland surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    None,
    XdgToplevel,
    XdgPopup,
    Subsurface,
    Cursor,
    LayerSurface,
}

/// Server-side state for a Wayland surface, combining the protocol
/// surface object with compositor metadata (role, buffer, callbacks).
#[derive(Debug)]
pub struct ManagedSurface {
    pub id: u32,
    pub client_id: ClientId,
    pub surface: liquide_wayland::Surface,
    pub role: SurfaceRole,
    pub buffer: Option<BufferRef>,
    pub pending_frame_callbacks: Vec<u32>,
}

/// Tracks all surfaces created by connected clients.
#[derive(Debug)]
pub struct SurfaceManager {
    surfaces: HashMap<u32, ManagedSurface>,
    next_id: u32,
}

impl SurfaceManager {
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn create_surface(&mut self, client_id: ClientId) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let surface = ManagedSurface {
            id,
            client_id,
            surface: liquide_wayland::Surface::new(liquide_wayland::ObjectId(id)),
            role: SurfaceRole::None,
            buffer: None,
            pending_frame_callbacks: Vec::new(),
        };
        self.surfaces.insert(id, surface);
        id
    }

    pub fn destroy_surface(&mut self, id: u32) -> Option<ManagedSurface> {
        self.surfaces.remove(&id)
    }

    pub fn get_surface(&self, id: u32) -> Option<&ManagedSurface> {
        self.surfaces.get(&id)
    }

    pub fn get_surface_mut(&mut self, id: u32) -> Option<&mut ManagedSurface> {
        self.surfaces.get_mut(&id)
    }

    pub fn commit(&mut self, id: u32) -> Result<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or(WaylandServerError::Surface(format!(
                "surface {id} not found"
            )))?;
        surface.surface.commit();
        Ok(())
    }

    pub fn attach_buffer(&mut self, surface_id: u32, buffer: BufferRef) {
        if let Some(surface) = self.surfaces.get_mut(&surface_id) {
            surface.buffer = Some(buffer);
        }
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }
}

impl Default for SurfaceManager {
    fn default() -> Self {
        Self::new()
    }
}

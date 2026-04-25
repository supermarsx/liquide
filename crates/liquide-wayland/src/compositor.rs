//! Compositor globals (wl_compositor equivalent).
//!
//! Manages the creation and tracking of surfaces and regions,
//! and maintains surface z-ordering for composition.

use crate::protocol::ObjectId;
use crate::surface::{Region, Surface};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// WlCompositor
// ---------------------------------------------------------------------------

/// The compositor global: manages surfaces, regions, and z-ordering.
///
/// Corresponds to the `wl_compositor` global in the Wayland protocol.
/// Provides `create_surface()` and `create_region()` requests.
#[derive(Debug)]
pub struct WlCompositor {
    /// All surfaces, keyed by object ID.
    surfaces: HashMap<ObjectId, Surface>,

    /// All regions, keyed by object ID.
    regions: HashMap<ObjectId, Region>,

    /// Surface IDs in z-order (bottom to top).
    z_order: Vec<ObjectId>,

    /// Next ID to allocate.
    next_id: u32,
}

impl WlCompositor {
    /// Create a new compositor.
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            regions: HashMap::new(),
            z_order: Vec::new(),
            next_id: 2, // 1 is reserved for wl_display
        }
    }

    /// Allocate the next object ID.
    fn alloc_id(&mut self) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Create a new surface and return its object ID.
    pub fn create_surface(&mut self) -> ObjectId {
        let id = self.alloc_id();
        let surface = Surface::new(id);
        self.surfaces.insert(id, surface);
        self.z_order.push(id);
        id
    }

    /// Create a new region and return its object ID.
    pub fn create_region(&mut self) -> ObjectId {
        let id = self.alloc_id();
        self.regions.insert(id, Region::new());
        id
    }

    /// Get a reference to a surface by ID.
    pub fn get_surface(&self, id: ObjectId) -> Option<&Surface> {
        self.surfaces.get(&id)
    }

    /// Get a mutable reference to a surface by ID.
    pub fn get_surface_mut(&mut self, id: ObjectId) -> Option<&mut Surface> {
        self.surfaces.get_mut(&id)
    }

    /// Get a reference to a region by ID.
    pub fn get_region(&self, id: ObjectId) -> Option<&Region> {
        self.regions.get(&id)
    }

    /// Get a mutable reference to a region by ID.
    pub fn get_region_mut(&mut self, id: ObjectId) -> Option<&mut Region> {
        self.regions.get_mut(&id)
    }

    /// Destroy a surface, removing it from tracking and z-order.
    pub fn destroy_surface(&mut self, id: ObjectId) -> bool {
        if self.surfaces.remove(&id).is_some() {
            self.z_order.retain(|z| *z != id);
            true
        } else {
            false
        }
    }

    /// Destroy a region, removing it from tracking.
    pub fn destroy_region(&mut self, id: ObjectId) -> bool {
        self.regions.remove(&id).is_some()
    }

    /// The current z-order of surfaces (bottom to top).
    pub fn z_order(&self) -> &[ObjectId] {
        &self.z_order
    }

    /// Total number of tracked surfaces.
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    /// Total number of tracked regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Raise a surface to the top of the z-order.
    pub fn raise_surface(&mut self, id: ObjectId) {
        if self.surfaces.contains_key(&id) {
            self.z_order.retain(|z| *z != id);
            self.z_order.push(id);
        }
    }

    /// Lower a surface to the bottom of the z-order.
    pub fn lower_surface(&mut self, id: ObjectId) {
        if self.surfaces.contains_key(&id) {
            self.z_order.retain(|z| *z != id);
            self.z_order.insert(0, id);
        }
    }

    /// Place `surface_id` directly above `sibling_id` in the z-order.
    ///
    /// Returns `true` if successful.
    pub fn place_above(&mut self, surface_id: ObjectId, sibling_id: ObjectId) -> bool {
        if !self.surfaces.contains_key(&surface_id) || !self.surfaces.contains_key(&sibling_id) {
            return false;
        }
        self.z_order.retain(|z| *z != surface_id);
        if let Some(pos) = self.z_order.iter().position(|z| *z == sibling_id) {
            self.z_order.insert(pos + 1, surface_id);
            true
        } else {
            self.z_order.push(surface_id);
            false
        }
    }

    /// Place `surface_id` directly below `sibling_id` in the z-order.
    ///
    /// Returns `true` if successful.
    pub fn place_below(&mut self, surface_id: ObjectId, sibling_id: ObjectId) -> bool {
        if !self.surfaces.contains_key(&surface_id) || !self.surfaces.contains_key(&sibling_id) {
            return false;
        }
        self.z_order.retain(|z| *z != surface_id);
        if let Some(pos) = self.z_order.iter().position(|z| *z == sibling_id) {
            self.z_order.insert(pos, surface_id);
            true
        } else {
            self.z_order.insert(0, surface_id);
            false
        }
    }

    /// Create a subsurface relationship: `child_id` becomes a subsurface
    /// of `parent_id`.
    ///
    /// Returns `true` if both surfaces exist and the relationship was created.
    pub fn create_subsurface(&mut self, child_id: ObjectId, parent_id: ObjectId) -> bool {
        if !self.surfaces.contains_key(&child_id) || !self.surfaces.contains_key(&parent_id) {
            return false;
        }

        // Remove child from top-level z-order
        self.z_order.retain(|z| *z != child_id);

        // Set parent on child (need split borrows)
        // We use a temporary to avoid double-borrow
        let parent_exists = self.surfaces.contains_key(&parent_id);
        if parent_exists {
            if let Some(child) = self.surfaces.get_mut(&child_id) {
                child.set_parent(parent_id);
            }
            if let Some(parent) = self.surfaces.get_mut(&parent_id) {
                parent.add_child(child_id);
            }
        }

        true
    }
}

impl Default for WlCompositor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_surface_assigns_id() {
        let mut c = WlCompositor::new();
        let id = c.create_surface();
        assert!(!id.is_null());
        assert!(c.get_surface(id).is_some());
        assert_eq!(c.surface_count(), 1);
    }

    #[test]
    fn create_multiple_surfaces() {
        let mut c = WlCompositor::new();
        let id1 = c.create_surface();
        let id2 = c.create_surface();
        let id3 = c.create_surface();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(c.surface_count(), 3);
        assert_eq!(c.z_order(), &[id1, id2, id3]);
    }

    #[test]
    fn destroy_surface() {
        let mut c = WlCompositor::new();
        let id = c.create_surface();
        assert!(c.destroy_surface(id));
        assert_eq!(c.surface_count(), 0);
        assert!(c.get_surface(id).is_none());
        assert!(c.z_order().is_empty());
    }

    #[test]
    fn destroy_nonexistent_surface() {
        let mut c = WlCompositor::new();
        assert!(!c.destroy_surface(ObjectId(999)));
    }

    #[test]
    fn create_region() {
        let mut c = WlCompositor::new();
        let id = c.create_region();
        assert!(c.get_region(id).is_some());
        assert_eq!(c.region_count(), 1);
    }

    #[test]
    fn modify_region() {
        let mut c = WlCompositor::new();
        let id = c.create_region();
        {
            let region = c.get_region_mut(id).unwrap();
            region.add(0, 0, 100, 100);
            region.subtract(25, 25, 50, 50);
        }
        let region = c.get_region(id).unwrap();
        assert!(region.contains(10, 10));
        assert!(!region.contains(50, 50));
    }

    #[test]
    fn destroy_region() {
        let mut c = WlCompositor::new();
        let id = c.create_region();
        assert!(c.destroy_region(id));
        assert_eq!(c.region_count(), 0);
    }

    #[test]
    fn raise_surface() {
        let mut c = WlCompositor::new();
        let id1 = c.create_surface();
        let id2 = c.create_surface();
        let id3 = c.create_surface();
        c.raise_surface(id1);
        assert_eq!(c.z_order(), &[id2, id3, id1]);
    }

    #[test]
    fn lower_surface() {
        let mut c = WlCompositor::new();
        let id1 = c.create_surface();
        let id2 = c.create_surface();
        let id3 = c.create_surface();
        c.lower_surface(id3);
        assert_eq!(c.z_order(), &[id3, id1, id2]);
    }

    #[test]
    fn place_above_sibling() {
        let mut c = WlCompositor::new();
        let id1 = c.create_surface();
        let id2 = c.create_surface();
        let id3 = c.create_surface();
        // Move id1 above id2: order becomes [id2, id1, id3]
        c.place_above(id1, id2);
        assert_eq!(c.z_order(), &[id2, id1, id3]);
    }

    #[test]
    fn place_below_sibling() {
        let mut c = WlCompositor::new();
        let id1 = c.create_surface();
        let id2 = c.create_surface();
        let id3 = c.create_surface();
        // Move id3 below id2: order becomes [id1, id3, id2]
        c.place_below(id3, id2);
        assert_eq!(c.z_order(), &[id1, id3, id2]);
    }

    #[test]
    fn surface_commit_through_compositor() {
        let mut c = WlCompositor::new();
        let id = c.create_surface();
        {
            let s = c.get_surface_mut(id).unwrap();
            s.attach(Some(ObjectId(200)), 0, 0);
            s.commit();
        }
        let s = c.get_surface(id).unwrap();
        assert_eq!(s.buffer(), Some(ObjectId(200)));
    }

    #[test]
    fn create_subsurface() {
        let mut c = WlCompositor::new();
        let parent = c.create_surface();
        let child = c.create_surface();
        assert!(c.create_subsurface(child, parent));

        // Child removed from top-level z-order
        assert_eq!(c.z_order(), &[parent]);

        // Parent has child
        let p = c.get_surface(parent).unwrap();
        assert_eq!(p.children(), &[child]);

        // Child knows its parent
        let ch = c.get_surface(child).unwrap();
        assert_eq!(ch.parent(), Some(parent));
    }

    #[test]
    fn create_subsurface_invalid_ids() {
        let mut c = WlCompositor::new();
        let parent = c.create_surface();
        assert!(!c.create_subsurface(ObjectId(999), parent));
        assert!(!c.create_subsurface(parent, ObjectId(999)));
    }
}

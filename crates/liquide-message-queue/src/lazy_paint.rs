//! Lazy paint system inspired by NT's WM_PAINT.
//!
//! NT never queues WM_PAINT as a real message. Instead:
//! 1. InvalidateRect() sets a flag + accumulates the invalid region
//! 2. GetMessage() checks the flag when the queue is empty
//! 3. Synthesizes WM_PAINT with the accumulated region
//! 4. BeginPaint() clears the flag and region
//!
//! This naturally coalesces multiple invalidations into one repaint.

use std::collections::HashMap;

pub type SurfaceId = u64;

/// Accumulated damage for a paintable surface.
#[derive(Debug, Clone)]
pub enum PaintDamage {
    /// Nothing to paint.
    None,
    /// Single rect (common case, avoids Vec alloc).
    Single([f32; 4]),
    /// Multiple accumulated rects.
    Multi(Vec<[f32; 4]>),
    /// Entire surface.
    Full,
}

impl PaintDamage {
    pub fn is_empty(&self) -> bool {
        matches!(self, PaintDamage::None)
    }

    pub fn add_rect(&mut self, rect: [f32; 4]) {
        match self {
            PaintDamage::None => *self = PaintDamage::Single(rect),
            PaintDamage::Single(existing) => {
                let e = *existing;
                *self = PaintDamage::Multi(vec![e, rect]);
            }
            PaintDamage::Multi(rects) => rects.push(rect),
            PaintDamage::Full => {} // Already full, no-op
        }
    }

    pub fn mark_full(&mut self) {
        *self = PaintDamage::Full;
    }

    pub fn bounding_rect(&self) -> Option<[f32; 4]> {
        match self {
            PaintDamage::None => None,
            PaintDamage::Single(r) => Some(*r),
            PaintDamage::Multi(rects) => {
                if rects.is_empty() {
                    return None;
                }
                let mut x0 = f32::MAX;
                let mut y0 = f32::MAX;
                let mut x1 = f32::MIN;
                let mut y1 = f32::MIN;
                for r in rects {
                    x0 = x0.min(r[0]);
                    y0 = y0.min(r[1]);
                    x1 = x1.max(r[0] + r[2]);
                    y1 = y1.max(r[1] + r[3]);
                }
                Some([x0, y0, x1 - x0, y1 - y0])
            }
            PaintDamage::Full => None, // Caller should use surface bounds
        }
    }

    pub fn rect_count(&self) -> usize {
        match self {
            PaintDamage::None => 0,
            PaintDamage::Single(_) => 1,
            PaintDamage::Multi(r) => r.len(),
            PaintDamage::Full => 1,
        }
    }

    /// Merge rects if count exceeds threshold by combining into bounding box.
    pub fn simplify(&mut self, max_rects: usize) {
        if let PaintDamage::Multi(rects) = self {
            if rects.len() > max_rects {
                if let Some(bbox) = self.bounding_rect() {
                    *self = PaintDamage::Single(bbox);
                }
            }
        }
    }
}

/// A synthesized paint request.
#[derive(Debug, Clone)]
pub struct PaintRequest {
    pub surface_id: SurfaceId,
    pub damage: PaintDamage,
    pub generation: u64,
    pub erase_background: bool,
}

struct SurfacePaintState {
    damage: PaintDamage,
    needs_paint: bool,
    needs_erase: bool,
    generation: u64,
    opaque: bool,
}

/// Lazy paint manager. Accumulates invalidations, synthesizes paint requests on demand.
///
/// Multiple invalidate() calls between frames are naturally coalesced into ONE paint.
/// Paint is always lowest priority — processed only when no real input is pending.
pub struct LazyPaintManager {
    surfaces: HashMap<SurfaceId, SurfacePaintState>,
    generation: u64,
    max_damage_rects: usize,
    // Stats
    invalidate_count: u64,
    paint_count: u64,
    coalesced_count: u64,
}

impl LazyPaintManager {
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            generation: 0,
            max_damage_rects: 8,
            invalidate_count: 0,
            paint_count: 0,
            coalesced_count: 0,
        }
    }

    pub fn with_max_damage_rects(mut self, max: usize) -> Self {
        self.max_damage_rects = max;
        self
    }

    /// Register a paintable surface.
    pub fn register_surface(&mut self, id: SurfaceId, opaque: bool) {
        self.surfaces.insert(id, SurfacePaintState {
            damage: PaintDamage::None,
            needs_paint: false,
            needs_erase: false,
            generation: 0,
            opaque,
        });
    }

    /// Unregister a surface (window closed).
    pub fn unregister_surface(&mut self, id: SurfaceId) {
        self.surfaces.remove(&id);
    }

    /// Mark a rect as needing repaint. Does NOT generate a paint message.
    pub fn invalidate(&mut self, surface_id: SurfaceId, rect: [f32; 4]) {
        self.invalidate_count += 1;
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            if state.needs_paint {
                self.coalesced_count += 1;
            }
            state.damage.add_rect(rect);
            state.needs_paint = true;
            // Simplify if too many rects
            if state.damage.rect_count() > self.max_damage_rects {
                state.damage.simplify(self.max_damage_rects);
            }
        }
    }

    /// Mark entire surface as needing repaint.
    pub fn invalidate_full(&mut self, surface_id: SurfaceId) {
        self.invalidate_count += 1;
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            if state.needs_paint {
                self.coalesced_count += 1;
            }
            state.damage.mark_full();
            state.needs_paint = true;
        }
    }

    /// Mark surface as needing erase + repaint.
    pub fn invalidate_erase(&mut self, surface_id: SurfaceId, rect: [f32; 4]) {
        self.invalidate_count += 1;
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            if state.needs_paint {
                self.coalesced_count += 1;
            }
            state.damage.add_rect(rect);
            state.needs_paint = true;
            state.needs_erase = true;
        }
    }

    /// Clear damage for a surface after painting (like NT's BeginPaint).
    pub fn validate(&mut self, surface_id: SurfaceId) {
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            state.damage = PaintDamage::None;
            state.needs_paint = false;
            state.needs_erase = false;
        }
    }

    /// Check if ANY surface needs painting. O(n) scan but n is typically small.
    pub fn has_pending_paints(&self) -> bool {
        self.surfaces.values().any(|s| s.needs_paint)
    }

    /// Count of surfaces needing paint.
    pub fn pending_count(&self) -> usize {
        self.surfaces.values().filter(|s| s.needs_paint).count()
    }

    /// Synthesize paint requests for all dirty surfaces.
    /// Called by the message pump when no higher-priority work exists.
    pub fn synthesize(&mut self) -> Vec<PaintRequest> {
        self.generation += 1;
        let current_gen = self.generation;
        let mut requests = Vec::new();

        for (&id, state) in self.surfaces.iter_mut() {
            if state.needs_paint {
                self.paint_count += 1;
                state.generation = current_gen;
                requests.push(PaintRequest {
                    surface_id: id,
                    damage: std::mem::replace(&mut state.damage, PaintDamage::None),
                    generation: current_gen,
                    erase_background: state.needs_erase && !state.opaque,
                });
                state.needs_paint = false;
                state.needs_erase = false;
            }
        }

        requests
    }

    /// Synthesize for a single surface only.
    pub fn synthesize_for(&mut self, surface_id: SurfaceId) -> Option<PaintRequest> {
        self.generation += 1;
        let current_gen = self.generation;

        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            if state.needs_paint {
                self.paint_count += 1;
                state.generation = current_gen;
                let req = PaintRequest {
                    surface_id,
                    damage: std::mem::replace(&mut state.damage, PaintDamage::None),
                    generation: current_gen,
                    erase_background: state.needs_erase && !state.opaque,
                };
                state.needs_paint = false;
                state.needs_erase = false;
                return Some(req);
            }
        }
        None
    }

    pub fn stats(&self) -> LazyPaintStats {
        let total = self.invalidate_count;
        LazyPaintStats {
            surface_count: self.surfaces.len(),
            pending_paints: self.pending_count(),
            total_invalidations: self.invalidate_count,
            total_paints: self.paint_count,
            coalesced: self.coalesced_count,
            coalesce_ratio: if total > 0 {
                self.coalesced_count as f64 / total as f64
            } else {
                0.0
            },
        }
    }
}

impl Default for LazyPaintManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct LazyPaintStats {
    pub surface_count: usize,
    pub pending_paints: usize,
    pub total_invalidations: u64,
    pub total_paints: u64,
    pub coalesced: u64,
    pub coalesce_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager_no_paints() {
        let mut mgr = LazyPaintManager::new();
        assert!(!mgr.has_pending_paints());
        assert!(mgr.synthesize().is_empty());
    }

    #[test]
    fn invalidate_triggers_paint() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 100.0, 100.0]);
        assert!(mgr.has_pending_paints());
        let reqs = mgr.synthesize();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].surface_id, 1);
    }

    #[test]
    fn multiple_invalidations_coalesce() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 50.0, 50.0]);
        mgr.invalidate(1, [50.0, 50.0, 50.0, 50.0]);
        mgr.invalidate(1, [25.0, 25.0, 50.0, 50.0]);
        // Only ONE paint request, not three
        let reqs = mgr.synthesize();
        assert_eq!(reqs.len(), 1);
        assert_eq!(mgr.stats().coalesced, 2);
    }

    #[test]
    fn invalidate_full_overrides_rects() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [10.0, 10.0, 20.0, 20.0]);
        mgr.invalidate_full(1);
        let reqs = mgr.synthesize();
        assert_eq!(reqs.len(), 1);
        assert!(matches!(reqs[0].damage, PaintDamage::Full));
    }

    #[test]
    fn synthesize_clears_pending() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 100.0, 100.0]);
        let _ = mgr.synthesize();
        assert!(!mgr.has_pending_paints());
        assert!(mgr.synthesize().is_empty());
    }

    #[test]
    fn validate_clears_damage() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 100.0, 100.0]);
        mgr.validate(1);
        assert!(!mgr.has_pending_paints());
    }

    #[test]
    fn unregistered_surface_ignored() {
        let mut mgr = LazyPaintManager::new();
        mgr.invalidate(999, [0.0, 0.0, 10.0, 10.0]);
        assert!(!mgr.has_pending_paints());
    }

    #[test]
    fn opaque_surface_no_erase() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, true); // opaque
        mgr.invalidate_erase(1, [0.0, 0.0, 100.0, 100.0]);
        let reqs = mgr.synthesize();
        assert!(!reqs[0].erase_background); // Opaque = no erase needed
    }

    #[test]
    fn transparent_surface_erase() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false); // transparent
        mgr.invalidate_erase(1, [0.0, 0.0, 100.0, 100.0]);
        let reqs = mgr.synthesize();
        assert!(reqs[0].erase_background);
    }

    #[test]
    fn synthesize_for_specific() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.register_surface(2, false);
        mgr.invalidate(1, [0.0, 0.0, 10.0, 10.0]);
        mgr.invalidate(2, [0.0, 0.0, 20.0, 20.0]);
        let req = mgr.synthesize_for(1);
        assert!(req.is_some());
        assert_eq!(req.unwrap().surface_id, 1);
        // Surface 2 still pending
        assert!(mgr.has_pending_paints());
    }

    #[test]
    fn damage_bounding_rect() {
        let mut d = PaintDamage::None;
        d.add_rect([10.0, 20.0, 30.0, 40.0]);
        d.add_rect([50.0, 60.0, 10.0, 10.0]);
        let bbox = d.bounding_rect().unwrap();
        assert_eq!(bbox[0], 10.0); // x
        assert_eq!(bbox[1], 20.0); // y
        assert_eq!(bbox[2], 50.0); // width (60-10)
        assert_eq!(bbox[3], 50.0); // height (70-20)
    }

    #[test]
    fn damage_simplify() {
        let mut d = PaintDamage::None;
        for i in 0..20 {
            d.add_rect([i as f32 * 10.0, 0.0, 10.0, 10.0]);
        }
        assert_eq!(d.rect_count(), 20);
        d.simplify(5);
        assert!(d.rect_count() <= 5);
    }

    #[test]
    fn generation_increments() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 10.0, 10.0]);
        let r1 = mgr.synthesize();
        mgr.invalidate(1, [0.0, 0.0, 10.0, 10.0]);
        let r2 = mgr.synthesize();
        assert!(r2[0].generation > r1[0].generation);
    }

    #[test]
    fn stats_tracking() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.invalidate(1, [0.0, 0.0, 10.0, 10.0]);
        mgr.invalidate(1, [10.0, 0.0, 10.0, 10.0]);
        let _ = mgr.synthesize();
        let stats = mgr.stats();
        assert_eq!(stats.total_invalidations, 2);
        assert_eq!(stats.total_paints, 1);
        assert_eq!(stats.coalesced, 1);
    }

    #[test]
    fn multiple_surfaces() {
        let mut mgr = LazyPaintManager::new();
        mgr.register_surface(1, false);
        mgr.register_surface(2, false);
        mgr.register_surface(3, true);
        mgr.invalidate(1, [0.0, 0.0, 10.0, 10.0]);
        mgr.invalidate(3, [0.0, 0.0, 10.0, 10.0]);
        let reqs = mgr.synthesize();
        assert_eq!(reqs.len(), 2);
        assert_eq!(mgr.pending_count(), 0);
    }
}

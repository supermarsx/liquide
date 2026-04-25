//! Surface management (wl_surface / wl_subsurface equivalent).
//!
//! A `Surface` represents a rectangular area of pixels that can be composited.
//! State changes are accumulated in a pending state and applied atomically
//! via `commit()`, implementing Wayland's double-buffered state model.

use crate::protocol::ObjectId;

// ---------------------------------------------------------------------------
// DamageRect
// ---------------------------------------------------------------------------

/// A rectangular damage region in surface-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl DamageRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns true if this damage rect overlaps with `other`.
    pub fn intersects(&self, other: &DamageRect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

// ---------------------------------------------------------------------------
// Region
// ---------------------------------------------------------------------------

/// A region composed of rectangles, used for opaque/input regions.
#[derive(Debug, Clone, Default)]
pub struct Region {
    rects: Vec<RegionOp>,
}

/// A single add or subtract operation on a region.
#[derive(Debug, Clone)]
enum RegionOp {
    Add(DamageRect),
    Subtract(DamageRect),
}

impl Region {
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Add a rectangle to the region.
    pub fn add(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.rects
            .push(RegionOp::Add(DamageRect::new(x, y, width, height)));
    }

    /// Subtract a rectangle from the region.
    pub fn subtract(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.rects
            .push(RegionOp::Subtract(DamageRect::new(x, y, width, height)));
    }

    /// Returns true if the region has any operations.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Returns the number of operations in this region.
    pub fn op_count(&self) -> usize {
        self.rects.len()
    }

    /// Test whether a point is inside the region.
    ///
    /// Evaluates all operations in order: adds include the point,
    /// subtracts exclude it.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        let mut inside = false;
        for op in &self.rects {
            match op {
                RegionOp::Add(r) => {
                    if px >= r.x && px < r.x + r.width && py >= r.y && py < r.y + r.height {
                        inside = true;
                    }
                }
                RegionOp::Subtract(r) => {
                    if px >= r.x && px < r.x + r.width && py >= r.y && py < r.y + r.height {
                        inside = false;
                    }
                }
            }
        }
        inside
    }
}

// ---------------------------------------------------------------------------
// SurfaceState
// ---------------------------------------------------------------------------

/// Lifecycle state of a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceState {
    /// Created but no role assigned or buffer ever committed.
    Initial,
    /// A buffer has been committed; the surface is visible.
    Mapped,
    /// The surface was mapped but has been explicitly unmapped
    /// (e.g. by attaching a null buffer).
    Unmapped,
}

// ---------------------------------------------------------------------------
// OutputTransform (for surface transform)
// ---------------------------------------------------------------------------

/// Transform applied to the surface buffer before compositing.
///
/// Matches the wl_output.transform enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    FlippedRotate90,
    FlippedRotate180,
    FlippedRotate270,
}

impl Default for Transform {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// SubsurfaceMode
// ---------------------------------------------------------------------------

/// Synchronization mode for subsurfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsurfaceMode {
    /// State is applied when the parent commits.
    Sync,
    /// State is applied immediately on own commit.
    Desync,
}

impl Default for SubsurfaceMode {
    fn default() -> Self {
        Self::Sync
    }
}

// ---------------------------------------------------------------------------
// SubsurfacePosition
// ---------------------------------------------------------------------------

/// Stacking order relative to a sibling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackingOrder {
    Above(ObjectId),
    Below(ObjectId),
}

// ---------------------------------------------------------------------------
// PendingSurfaceState
// ---------------------------------------------------------------------------

/// Accumulated changes that are applied atomically on `commit()`.
#[derive(Debug, Clone, Default)]
struct PendingState {
    buffer: Option<Option<ObjectId>>, // Some(Some(id)) = attach, Some(None) = null attach
    buffer_offset: Option<(i32, i32)>,
    damage: Vec<DamageRect>,
    opaque_region: Option<Region>,
    input_region: Option<Region>,
    transform: Option<Transform>,
    scale: Option<i32>,
    frame_callback: Option<ObjectId>,
}

// ---------------------------------------------------------------------------
// Surface
// ---------------------------------------------------------------------------

/// A compositable surface with double-buffered state.
///
/// Mirrors the semantics of `wl_surface`: changes are staged in pending state
/// and applied atomically when `commit()` is called.
#[derive(Debug)]
pub struct Surface {
    /// Protocol object ID.
    id: ObjectId,

    /// Current lifecycle state.
    state: SurfaceState,

    /// Currently attached buffer (None = no buffer).
    buffer: Option<ObjectId>,

    /// Buffer offset for attach.
    buffer_offset: (i32, i32),

    /// Accumulated damage since last commit.
    damage: Vec<DamageRect>,

    /// Opaque region hint.
    opaque_region: Option<Region>,

    /// Input region (area that receives pointer/touch events).
    input_region: Option<Region>,

    /// Buffer transform.
    transform: Transform,

    /// Buffer scale factor.
    scale: i32,

    /// Active frame callbacks (cleared after the compositor fires them).
    frame_callbacks: Vec<ObjectId>,

    /// Pending state waiting for commit.
    pending: PendingState,

    // -- Subsurface fields --
    /// Parent surface, if this is a subsurface.
    parent: Option<ObjectId>,

    /// Position relative to parent origin.
    subsurface_position: (i32, i32),

    /// Synchronization mode.
    subsurface_mode: SubsurfaceMode,

    /// Stacking order changes (applied on parent commit for sync mode).
    stacking_ops: Vec<StackingOrder>,

    /// Child subsurface IDs, in stacking order.
    children: Vec<ObjectId>,
}

impl Surface {
    /// Create a new surface with the given object ID.
    pub fn new(id: ObjectId) -> Self {
        Self {
            id,
            state: SurfaceState::Initial,
            buffer: None,
            buffer_offset: (0, 0),
            damage: Vec::new(),
            opaque_region: None,
            input_region: None,
            transform: Transform::Normal,
            scale: 1,
            frame_callbacks: Vec::new(),
            pending: PendingState::default(),
            parent: None,
            subsurface_position: (0, 0),
            subsurface_mode: SubsurfaceMode::Sync,
            stacking_ops: Vec::new(),
            children: Vec::new(),
        }
    }

    /// The object ID of this surface.
    #[inline]
    pub fn id(&self) -> ObjectId {
        self.id
    }

    /// Current lifecycle state.
    #[inline]
    pub fn state(&self) -> SurfaceState {
        self.state
    }

    /// Currently attached buffer, if any.
    #[inline]
    pub fn buffer(&self) -> Option<ObjectId> {
        self.buffer
    }

    /// Buffer offset.
    #[inline]
    pub fn buffer_offset(&self) -> (i32, i32) {
        self.buffer_offset
    }

    /// Current damage regions.
    pub fn damage(&self) -> &[DamageRect] {
        &self.damage
    }

    /// Current buffer transform.
    #[inline]
    pub fn transform(&self) -> Transform {
        self.transform
    }

    /// Current buffer scale factor.
    #[inline]
    pub fn scale(&self) -> i32 {
        self.scale
    }

    /// Active frame callbacks.
    pub fn frame_callbacks(&self) -> &[ObjectId] {
        &self.frame_callbacks
    }

    /// Clear frame callbacks after the compositor has fired them.
    pub fn clear_frame_callbacks(&mut self) {
        self.frame_callbacks.clear();
    }

    /// Opaque region hint.
    pub fn opaque_region(&self) -> Option<&Region> {
        self.opaque_region.as_ref()
    }

    /// Input region.
    pub fn input_region(&self) -> Option<&Region> {
        self.input_region.as_ref()
    }

    /// Parent surface (for subsurfaces).
    pub fn parent(&self) -> Option<ObjectId> {
        self.parent
    }

    /// Position relative to parent.
    pub fn subsurface_position(&self) -> (i32, i32) {
        self.subsurface_position
    }

    /// Subsurface synchronization mode.
    pub fn subsurface_mode(&self) -> SubsurfaceMode {
        self.subsurface_mode
    }

    /// Children in stacking order.
    pub fn children(&self) -> &[ObjectId] {
        &self.children
    }

    // -- Pending-state mutations (double buffered) --

    /// Attach a buffer to the surface.
    ///
    /// The attachment takes effect on the next `commit()`.
    /// Pass `None` to unmap the surface.
    pub fn attach(&mut self, buffer_id: Option<ObjectId>, dx: i32, dy: i32) {
        self.pending.buffer = Some(buffer_id);
        self.pending.buffer_offset = Some((dx, dy));
    }

    /// Mark a rectangular region as damaged.
    ///
    /// Damage is accumulated and applied on `commit()`.
    pub fn damage_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.pending
            .damage
            .push(DamageRect::new(x, y, width, height));
    }

    /// Set the opaque region hint.
    pub fn set_opaque_region(&mut self, region: Option<Region>) {
        self.pending.opaque_region = Some(region.unwrap_or_default());
    }

    /// Set the input region.
    pub fn set_input_region(&mut self, region: Option<Region>) {
        self.pending.input_region = Some(region.unwrap_or_default());
    }

    /// Set the buffer transform.
    pub fn set_transform(&mut self, transform: Transform) {
        self.pending.transform = Some(transform);
    }

    /// Set the buffer scale factor.
    pub fn set_scale(&mut self, scale: i32) {
        self.pending.scale = Some(scale);
    }

    /// Request a frame callback.
    pub fn frame(&mut self, callback_id: ObjectId) {
        self.pending.frame_callback = Some(callback_id);
    }

    /// Commit pending state atomically.
    ///
    /// All accumulated changes are applied to the current state.
    pub fn commit(&mut self) {
        // Buffer attachment
        if let Some(buf) = self.pending.buffer.take() {
            self.buffer = buf;
            if let Some(offset) = self.pending.buffer_offset.take() {
                self.buffer_offset = offset;
            }
            // Update lifecycle state
            if self.buffer.is_some() {
                self.state = SurfaceState::Mapped;
            } else {
                self.state = SurfaceState::Unmapped;
            }
        }

        // Damage
        if !self.pending.damage.is_empty() {
            self.damage = std::mem::take(&mut self.pending.damage);
        }

        // Opaque region
        if let Some(region) = self.pending.opaque_region.take() {
            self.opaque_region = Some(region);
        }

        // Input region
        if let Some(region) = self.pending.input_region.take() {
            self.input_region = Some(region);
        }

        // Transform
        if let Some(t) = self.pending.transform.take() {
            self.transform = t;
        }

        // Scale
        if let Some(s) = self.pending.scale.take() {
            self.scale = s;
        }

        // Frame callback
        if let Some(cb) = self.pending.frame_callback.take() {
            self.frame_callbacks.push(cb);
        }
    }

    // -- Subsurface operations --

    /// Set this surface as a subsurface of `parent_id`.
    pub fn set_parent(&mut self, parent_id: ObjectId) {
        self.parent = Some(parent_id);
    }

    /// Set the position of this subsurface relative to its parent.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.subsurface_position = (x, y);
    }

    /// Request that this subsurface be placed above `sibling`.
    pub fn place_above(&mut self, sibling: ObjectId) {
        self.stacking_ops.push(StackingOrder::Above(sibling));
    }

    /// Request that this subsurface be placed below `sibling`.
    pub fn place_below(&mut self, sibling: ObjectId) {
        self.stacking_ops.push(StackingOrder::Below(sibling));
    }

    /// Set synchronization mode.
    pub fn set_sync_mode(&mut self, mode: SubsurfaceMode) {
        self.subsurface_mode = mode;
    }

    /// Add a child subsurface.
    pub fn add_child(&mut self, child_id: ObjectId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Remove a child subsurface.
    pub fn remove_child(&mut self, child_id: ObjectId) {
        self.children.retain(|c| *c != child_id);
    }

    /// Consume and clear stacking order operations.
    pub fn take_stacking_ops(&mut self) -> Vec<StackingOrder> {
        std::mem::take(&mut self.stacking_ops)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_initial_state() {
        let s = Surface::new(ObjectId(10));
        assert_eq!(s.id(), ObjectId(10));
        assert_eq!(s.state(), SurfaceState::Initial);
        assert!(s.buffer().is_none());
        assert_eq!(s.scale(), 1);
        assert_eq!(s.transform(), Transform::Normal);
    }

    #[test]
    fn attach_and_commit_maps_surface() {
        let mut s = Surface::new(ObjectId(1));
        s.attach(Some(ObjectId(100)), 0, 0);
        // Not yet committed
        assert_eq!(s.state(), SurfaceState::Initial);
        assert!(s.buffer().is_none());

        s.commit();
        assert_eq!(s.state(), SurfaceState::Mapped);
        assert_eq!(s.buffer(), Some(ObjectId(100)));
    }

    #[test]
    fn attach_null_unmaps_surface() {
        let mut s = Surface::new(ObjectId(1));
        s.attach(Some(ObjectId(100)), 0, 0);
        s.commit();
        assert_eq!(s.state(), SurfaceState::Mapped);

        s.attach(None, 0, 0);
        s.commit();
        assert_eq!(s.state(), SurfaceState::Unmapped);
        assert!(s.buffer().is_none());
    }

    #[test]
    fn damage_accumulates_before_commit() {
        let mut s = Surface::new(ObjectId(1));
        s.damage_rect(0, 0, 100, 100);
        s.damage_rect(50, 50, 50, 50);
        assert!(s.damage().is_empty()); // not yet committed

        s.commit();
        assert_eq!(s.damage().len(), 2);
        assert_eq!(s.damage()[0], DamageRect::new(0, 0, 100, 100));
    }

    #[test]
    fn damage_replaced_on_next_commit() {
        let mut s = Surface::new(ObjectId(1));
        s.damage_rect(0, 0, 10, 10);
        s.commit();
        assert_eq!(s.damage().len(), 1);

        s.damage_rect(5, 5, 20, 20);
        s.commit();
        assert_eq!(s.damage().len(), 1);
        assert_eq!(s.damage()[0], DamageRect::new(5, 5, 20, 20));
    }

    #[test]
    fn set_transform_pending() {
        let mut s = Surface::new(ObjectId(1));
        s.set_transform(Transform::Rotate90);
        assert_eq!(s.transform(), Transform::Normal); // not yet committed
        s.commit();
        assert_eq!(s.transform(), Transform::Rotate90);
    }

    #[test]
    fn set_scale_pending() {
        let mut s = Surface::new(ObjectId(1));
        s.set_scale(2);
        assert_eq!(s.scale(), 1); // not yet committed
        s.commit();
        assert_eq!(s.scale(), 2);
    }

    #[test]
    fn frame_callback_accumulates() {
        let mut s = Surface::new(ObjectId(1));
        s.frame(ObjectId(50));
        s.commit();
        assert_eq!(s.frame_callbacks(), &[ObjectId(50)]);

        s.frame(ObjectId(51));
        s.commit();
        assert_eq!(s.frame_callbacks(), &[ObjectId(50), ObjectId(51)]);

        s.clear_frame_callbacks();
        assert!(s.frame_callbacks().is_empty());
    }

    #[test]
    fn opaque_region_pending() {
        let mut s = Surface::new(ObjectId(1));
        let mut region = Region::new();
        region.add(0, 0, 640, 480);
        s.set_opaque_region(Some(region));
        assert!(s.opaque_region().is_none()); // not committed
        s.commit();
        assert!(s.opaque_region().is_some());
        assert!(s.opaque_region().unwrap().contains(320, 240));
    }

    #[test]
    fn input_region_pending() {
        let mut s = Surface::new(ObjectId(1));
        let mut region = Region::new();
        region.add(10, 10, 100, 100);
        s.set_input_region(Some(region));
        s.commit();
        let ir = s.input_region().unwrap();
        assert!(ir.contains(50, 50));
        assert!(!ir.contains(5, 5));
    }

    #[test]
    fn buffer_offset() {
        let mut s = Surface::new(ObjectId(1));
        s.attach(Some(ObjectId(100)), 10, 20);
        s.commit();
        assert_eq!(s.buffer_offset(), (10, 20));
    }

    #[test]
    fn subsurface_parent_and_position() {
        let mut s = Surface::new(ObjectId(2));
        s.set_parent(ObjectId(1));
        s.set_position(50, 100);
        assert_eq!(s.parent(), Some(ObjectId(1)));
        assert_eq!(s.subsurface_position(), (50, 100));
    }

    #[test]
    fn subsurface_sync_mode() {
        let mut s = Surface::new(ObjectId(2));
        assert_eq!(s.subsurface_mode(), SubsurfaceMode::Sync);
        s.set_sync_mode(SubsurfaceMode::Desync);
        assert_eq!(s.subsurface_mode(), SubsurfaceMode::Desync);
    }

    #[test]
    fn subsurface_stacking() {
        let mut s = Surface::new(ObjectId(2));
        s.place_above(ObjectId(3));
        s.place_below(ObjectId(4));
        let ops = s.take_stacking_ops();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0], StackingOrder::Above(ObjectId(3)));
        assert_eq!(ops[1], StackingOrder::Below(ObjectId(4)));
        // After take, should be empty
        assert!(s.take_stacking_ops().is_empty());
    }

    #[test]
    fn parent_surface_children() {
        let mut parent = Surface::new(ObjectId(1));
        parent.add_child(ObjectId(2));
        parent.add_child(ObjectId(3));
        assert_eq!(parent.children(), &[ObjectId(2), ObjectId(3)]);

        // No duplicates
        parent.add_child(ObjectId(2));
        assert_eq!(parent.children().len(), 2);

        parent.remove_child(ObjectId(2));
        assert_eq!(parent.children(), &[ObjectId(3)]);
    }

    #[test]
    fn commit_without_pending_is_noop() {
        let mut s = Surface::new(ObjectId(1));
        s.attach(Some(ObjectId(100)), 0, 0);
        s.commit();
        let state_before = s.state();
        let buf_before = s.buffer();

        // Second commit with nothing pending
        s.commit();
        assert_eq!(s.state(), state_before);
        assert_eq!(s.buffer(), buf_before);
    }

    #[test]
    fn damage_rect_intersects() {
        let a = DamageRect::new(0, 0, 100, 100);
        let b = DamageRect::new(50, 50, 100, 100);
        assert!(a.intersects(&b));

        let c = DamageRect::new(200, 200, 10, 10);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn damage_rect_no_intersect_adjacent() {
        let a = DamageRect::new(0, 0, 100, 100);
        let b = DamageRect::new(100, 0, 100, 100); // adjacent, not overlapping
        assert!(!a.intersects(&b));
    }

    #[test]
    fn region_add_subtract() {
        let mut r = Region::new();
        r.add(0, 0, 100, 100);
        assert!(r.contains(50, 50));
        assert!(!r.contains(150, 150));

        r.subtract(40, 40, 20, 20);
        assert!(!r.contains(50, 50)); // inside subtracted rect
        assert!(r.contains(10, 10)); // still in add rect
    }

    #[test]
    fn region_empty() {
        let r = Region::new();
        assert!(r.is_empty());
        assert_eq!(r.op_count(), 0);
        assert!(!r.contains(0, 0));
    }

    #[test]
    fn region_multiple_adds() {
        let mut r = Region::new();
        r.add(0, 0, 50, 50);
        r.add(100, 100, 50, 50);
        assert!(r.contains(25, 25));
        assert!(r.contains(125, 125));
        assert!(!r.contains(75, 75));
    }

    #[test]
    fn multiple_commits_preserve_state() {
        let mut s = Surface::new(ObjectId(1));
        s.attach(Some(ObjectId(100)), 0, 0);
        s.set_scale(2);
        s.set_transform(Transform::Rotate180);
        s.commit();

        // Verify all state persists across a no-op commit
        s.commit();
        assert_eq!(s.buffer(), Some(ObjectId(100)));
        assert_eq!(s.scale(), 2);
        assert_eq!(s.transform(), Transform::Rotate180);
        assert_eq!(s.state(), SurfaceState::Mapped);
    }
}

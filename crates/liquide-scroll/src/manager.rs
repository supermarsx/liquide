use std::collections::HashMap;

use crate::momentum::MomentumScroller;
use crate::overscroll::OverscrollEffect;
use crate::scrollbar::{self, AutoHideController, Orientation, Rect, ScrollbarHit, ScrollbarState, ScrollbarStyle};
use crate::smooth::SmoothScroller;
use crate::snap::{self, SnapConfig};
use crate::state::ScrollState;
use crate::wheel::WheelConfig;

/// Per-container scroll data managed by the ScrollManager.
struct ScrollContainer {
    state: ScrollState,
    smooth: SmoothScroller,
    momentum: MomentumScroller,
    overscroll_x: OverscrollEffect,
    overscroll_y: OverscrollEffect,
    snap_x: Option<SnapConfig>,
    snap_y: Option<SnapConfig>,
    auto_hide_v: AutoHideController,
    auto_hide_h: AutoHideController,
    /// Visual overscroll displacement (x, y) for rendering.
    overscroll_displacement: (f32, f32),
}

/// Manages multiple scroll containers, coordinating smooth scrolling,
/// momentum, overscroll, snapping, and scrollbar state.
pub struct ScrollManager {
    containers: HashMap<u64, ScrollContainer>,
    /// Global wheel configuration.
    pub wheel_config: WheelConfig,
    /// Global scrollbar style.
    pub scrollbar_style: ScrollbarStyle,
}

impl ScrollManager {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            wheel_config: WheelConfig::new(),
            scrollbar_style: ScrollbarStyle::Overlay,
        }
    }

    /// Register a new scroll container. Returns a clone of the initial scroll state.
    pub fn register(
        &mut self,
        id: u64,
        content_size: (f32, f32),
        viewport_size: (f32, f32),
    ) -> ScrollState {
        let state = ScrollState::new(content_size, viewport_size);
        let container = ScrollContainer {
            state: state.clone(),
            smooth: SmoothScroller::new(),
            momentum: MomentumScroller::new(),
            overscroll_x: OverscrollEffect::new(),
            overscroll_y: OverscrollEffect::new(),
            snap_x: None,
            snap_y: None,
            auto_hide_v: AutoHideController::new(self.scrollbar_style),
            auto_hide_h: AutoHideController::new(self.scrollbar_style),
            overscroll_displacement: (0.0, 0.0),
        };
        let out = container.state.clone();
        self.containers.insert(id, container);
        out
    }

    /// Unregister a scroll container.
    pub fn unregister(&mut self, id: u64) {
        self.containers.remove(&id);
    }

    /// Get the current scroll state for a container.
    pub fn state(&self, id: u64) -> Option<&ScrollState> {
        self.containers.get(&id).map(|c| &c.state)
    }

    /// Get a mutable reference to the scroll state for a container.
    pub fn state_mut(&mut self, id: u64) -> Option<&mut ScrollState> {
        self.containers.get_mut(&id).map(|c| &mut c.state)
    }

    /// Set snap configuration for a container.
    pub fn set_snap_config(&mut self, id: u64, x: Option<SnapConfig>, y: Option<SnapConfig>) {
        if let Some(c) = self.containers.get_mut(&id) {
            c.snap_x = x;
            c.snap_y = y;
        }
    }

    /// Handle a mouse wheel event for a container.
    ///
    /// `delta` is the wheel tick count (x, y). Positive = scroll down/right.
    /// `smooth` overrides `wheel_config.smooth_wheel` if `true`.
    pub fn handle_wheel(&mut self, id: u64, delta: (f32, f32), smooth: bool) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };

        // Cancel any active momentum when user scrolls with wheel.
        c.momentum.cancel();

        let dx = self.wheel_config.compute_delta(delta.0, false, c.state.viewport_size.0);
        let dy = self.wheel_config.compute_delta(delta.1, false, c.state.viewport_size.1);

        let use_smooth = smooth || self.wheel_config.smooth_wheel;

        if use_smooth {
            let duration = self.wheel_config.smooth_duration_ms();
            let current = c.state.offset;
            let target = (current.0 + dx, current.1 + dy);
            // Clamp target to valid range.
            let max = c.state.max_scroll();
            let clamped = (target.0.clamp(0.0, max.0), target.1.clamp(0.0, max.1));
            c.smooth.scroll_to(current, clamped, duration);
        } else {
            c.state.scroll_by(dx, dy);
        }

        c.auto_hide_v.on_activity();
        c.auto_hide_h.on_activity();
    }

    /// Begin touch/trackpad tracking for a container.
    pub fn handle_touch_start(&mut self, id: u64, pos: (f32, f32)) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };
        // Cancel smooth scroll on touch start.
        c.smooth.cancel();
        c.momentum.begin_touch(pos);
    }

    /// Record a touch move for a container.
    pub fn handle_touch_move(&mut self, id: u64, pos: (f32, f32), timestamp_ms: u64) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };
        c.momentum.move_touch(pos, timestamp_ms);
        c.auto_hide_v.on_activity();
        c.auto_hide_h.on_activity();
    }

    /// End touch for a container.
    pub fn handle_touch_end(&mut self, id: u64) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };
        let started = c.momentum.end_touch();

        // If momentum didn't start, check for snap points.
        if !started {
            try_snap(c);
        }
    }

    /// Handle a click on a scrollbar element.
    ///
    /// For `Track` hits, scrolls by one page in the appropriate direction.
    /// For `UpArrow`/`DownArrow`, scrolls by a few lines.
    pub fn handle_scrollbar_click(&mut self, id: u64, hit: ScrollbarHit, orientation: Orientation) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };

        let page_size = match orientation {
            Orientation::Vertical => c.state.viewport_size.1,
            Orientation::Horizontal => c.state.viewport_size.0,
        };
        let line_delta = 60.0; // 3 lines * 20px

        let delta = match hit {
            ScrollbarHit::Track { before_thumb } => {
                if before_thumb {
                    -page_size
                } else {
                    page_size
                }
            }
            ScrollbarHit::UpArrow => -line_delta,
            ScrollbarHit::DownArrow => line_delta,
            ScrollbarHit::Thumb | ScrollbarHit::None => return,
        };

        match orientation {
            Orientation::Vertical => c.state.scroll_by(0.0, delta),
            Orientation::Horizontal => c.state.scroll_by(delta, 0.0),
        }

        c.auto_hide_v.on_activity();
        c.auto_hide_h.on_activity();
    }

    /// Handle dragging the scrollbar thumb.
    ///
    /// `delta` is the pixel movement along the scrollbar track.
    pub fn handle_scrollbar_drag(&mut self, id: u64, delta: f32, orientation: Orientation) {
        let Some(c) = self.containers.get_mut(&id) else {
            return;
        };

        let (content, viewport) = match orientation {
            Orientation::Vertical => (c.state.content_size.1, c.state.viewport_size.1),
            Orientation::Horizontal => (c.state.content_size.0, c.state.viewport_size.0),
        };

        if content <= viewport {
            return;
        }

        // Convert track-space delta to content-space delta.
        // Track length approximation: viewport size (the scrollbar track typically spans the viewport).
        let track_length = viewport;
        let ratio = (content - viewport) / (track_length - 30.0_f32.max(track_length * viewport / content));
        let content_delta = delta * ratio.max(1.0);

        match orientation {
            Orientation::Vertical => c.state.scroll_by(0.0, content_delta),
            Orientation::Horizontal => c.state.scroll_by(content_delta, 0.0),
        }

        c.auto_hide_v.on_activity();
        c.auto_hide_h.on_activity();
    }

    /// Scroll to make an element rectangle visible within a container.
    ///
    /// `element_rect` is in content coordinates: (x, y, width, height).
    pub fn scroll_to_element(&mut self, container_id: u64, element_rect: (f32, f32, f32, f32)) {
        let Some(c) = self.containers.get_mut(&container_id) else {
            return;
        };

        let (ex, ey, ew, eh) = element_rect;
        let (ox, oy) = c.state.offset;
        let (vw, vh) = c.state.viewport_size;

        let mut new_x = ox;
        let mut new_y = oy;

        // Horizontal: make element fully visible.
        if ex < ox {
            new_x = ex;
        } else if ex + ew > ox + vw {
            new_x = ex + ew - vw;
        }

        // Vertical: make element fully visible.
        if ey < oy {
            new_y = ey;
        } else if ey + eh > oy + vh {
            new_y = ey + eh - vh;
        }

        // Smooth scroll to the target.
        let current = c.state.offset;
        let max = c.state.max_scroll();
        let target = (new_x.clamp(0.0, max.0), new_y.clamp(0.0, max.1));
        c.smooth.scroll_to(current, target, 300);

        c.auto_hide_v.on_activity();
        c.auto_hide_h.on_activity();
    }

    /// Tick all active animations. Returns a list of (container_id, new_offset) for
    /// any containers whose offset changed this frame.
    pub fn tick(&mut self, elapsed_ms: u32) -> Vec<(u64, (f32, f32))> {
        let mut updates = Vec::new();

        for (&id, c) in self.containers.iter_mut() {
            let prev = c.state.offset;
            let mut changed = false;

            // Smooth scroller.
            if c.smooth.is_animating() {
                let pos = c.smooth.tick(elapsed_ms);
                c.state.set_offset(pos.0, pos.1);
                changed = true;

                // When smooth scroll finishes, check snap points.
                if !c.smooth.is_animating() {
                    try_snap(c);
                }
            }

            // Momentum scroller.
            if c.momentum.is_active() {
                let delta = c.momentum.tick(elapsed_ms);
                c.state.scroll_by(delta.0, delta.1);
                changed = true;

                // When momentum ends, check snap points.
                if !c.momentum.is_active() {
                    try_snap(c);
                }
            }

            // Auto-hide timers.
            c.auto_hide_v.tick(elapsed_ms);
            c.auto_hide_h.tick(elapsed_ms);

            if changed && (c.state.offset.0 != prev.0 || c.state.offset.1 != prev.1) {
                updates.push((id, c.state.offset));
            }
        }

        updates
    }

    /// Compute the vertical scrollbar state for a container.
    pub fn scrollbar_v(&self, id: u64, track_length: f32) -> Option<ScrollbarState> {
        let c = self.containers.get(&id)?;
        Some(scrollbar::compute(&c.state, track_length, Orientation::Vertical))
    }

    /// Compute the horizontal scrollbar state for a container.
    pub fn scrollbar_h(&self, id: u64, track_length: f32) -> Option<ScrollbarState> {
        let c = self.containers.get(&id)?;
        Some(scrollbar::compute(&c.state, track_length, Orientation::Horizontal))
    }

    /// Get the auto-hide opacity for the vertical scrollbar.
    pub fn scrollbar_v_opacity(&self, id: u64) -> f32 {
        self.containers
            .get(&id)
            .map(|c| c.auto_hide_v.opacity())
            .unwrap_or(0.0)
    }

    /// Get the auto-hide opacity for the horizontal scrollbar.
    pub fn scrollbar_h_opacity(&self, id: u64) -> f32 {
        self.containers
            .get(&id)
            .map(|c| c.auto_hide_h.opacity())
            .unwrap_or(0.0)
    }

    /// Number of registered containers.
    pub fn container_count(&self) -> usize {
        self.containers.len()
    }

    /// Whether any container has active animations.
    pub fn has_active_animations(&self) -> bool {
        self.containers.values().any(|c| {
            c.smooth.is_animating() || c.momentum.is_active()
        })
    }

    /// Get the overscroll visual displacement for rendering.
    /// Returns (x_displacement, y_displacement) which should be added to the
    /// scroll offset for visual rendering only (not logical scroll state).
    pub fn overscroll_displacement(&self, id: u64) -> (f32, f32) {
        self.containers
            .get(&id)
            .map(|c| c.overscroll_displacement)
            .unwrap_or((0.0, 0.0))
    }

    /// Enable or disable overscroll effect for a container.
    pub fn set_overscroll_enabled(&mut self, id: u64, enabled: bool) {
        if let Some(c) = self.containers.get_mut(&id) {
            c.overscroll_x.enabled = enabled;
            c.overscroll_y.enabled = enabled;
        }
    }

    /// Set maximum overscroll distance for a container.
    pub fn set_max_overscroll(&mut self, id: u64, max_px: f32) {
        if let Some(c) = self.containers.get_mut(&id) {
            c.overscroll_x.max_overscroll = max_px;
            c.overscroll_y.max_overscroll = max_px;
        }
    }

    /// Hit-test a point against a scrollbar for a given container.
    pub fn hit_test_scrollbar(
        &self,
        id: u64,
        point: (f32, f32),
        scrollbar_rect: Rect,
        orientation: Orientation,
        track_length: f32,
    ) -> ScrollbarHit {
        let Some(c) = self.containers.get(&id) else {
            return ScrollbarHit::None;
        };
        let sb_state = scrollbar::compute(&c.state, track_length, orientation);
        scrollbar::hit_test(point, scrollbar_rect, &sb_state)
    }
}

impl Default for ScrollManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Try to snap a container to a snap point after scrolling ends.
fn try_snap(c: &mut ScrollContainer) {
    let mut snapped = false;
    let mut target = c.state.offset;

    if let Some(ref snap_cfg) = c.snap_x {
        if let Some(snap_x) = snap::find_snap_target(
            c.state.offset.0,
            c.momentum.velocity().0,
            c.state.viewport_size.0,
            snap_cfg,
        ) {
            target.0 = snap_x;
            snapped = true;
        }
    }

    if let Some(ref snap_cfg) = c.snap_y {
        if let Some(snap_y) = snap::find_snap_target(
            c.state.offset.1,
            c.momentum.velocity().1,
            c.state.viewport_size.1,
            snap_cfg,
        ) {
            target.1 = snap_y;
            snapped = true;
        }
    }

    if snapped {
        let current = c.state.offset;
        c.smooth.scroll_to(current, target, 250);
    }
}

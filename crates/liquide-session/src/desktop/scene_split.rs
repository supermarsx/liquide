//! Scene splitting — categorizes flattened scene nodes by UI component.
//!
//! This module enables per-component metrics (node counts) and prepares for
//! future per-component render dispatch via `liquide-render-coordinator`.

use liquide_compositor::scene::FlatNode;

/// Well-known node ID ranges from `liquide-shell::scene_builder`.
const NODE_BACKGROUND: u64 = 1;
const NODE_STATUS_BAR: u64 = 1_000;
const NODE_STATUS_BAR_END: u64 = 2_000;
const NODE_DOCK: u64 = 2_000;
const NODE_DOCK_END: u64 = 10_000;
const NODE_WINDOW_BASE: u64 = 10_000;
const NODE_WINDOW_END: u64 = 100_000;

/// Per-component breakdown of a flattened scene.
#[derive(Debug, Default)]
pub(super) struct SplitScene {
    /// Background node count (wallpaper, desktop color).
    pub background_count: usize,
    /// Status bar node count.
    pub statusbar_count: usize,
    /// Dock node count.
    pub dock_count: usize,
    /// Window node count (all managed windows combined).
    pub window_count: usize,
    /// Number of distinct windows detected.
    pub window_ids: usize,
    /// Overlay node count (notifications, launcher, menus, cursor).
    pub overlay_count: usize,
}

impl SplitScene {
    /// Total node count across all components.
    pub fn total(&self) -> usize {
        self.background_count
            + self.statusbar_count
            + self.dock_count
            + self.window_count
            + self.overlay_count
    }
}

/// Categorize flattened nodes into UI components by their ID ranges.
pub(super) fn split_flat_nodes(nodes: &[FlatNode]) -> SplitScene {
    let mut scene = SplitScene::default();
    let mut last_window_id: Option<u64> = None;

    for node in nodes {
        let id = node.id;
        match id {
            0..=NODE_BACKGROUND => scene.background_count += 1,
            NODE_STATUS_BAR..NODE_STATUS_BAR_END => scene.statusbar_count += 1,
            NODE_DOCK..NODE_DOCK_END => scene.dock_count += 1,
            NODE_WINDOW_BASE..NODE_WINDOW_END => {
                scene.window_count += 1;
                // Detect distinct windows by stride boundaries.
                let window_id = (id - NODE_WINDOW_BASE) / 10;
                if last_window_id != Some(window_id) {
                    scene.window_ids += 1;
                    last_window_id = Some(window_id);
                }
            }
            _ => scene.overlay_count += 1,
        }
    }

    scene
}

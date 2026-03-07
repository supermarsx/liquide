//! CSS pipeline bridge — runs DOM → Style → Layout → Paint → SceneNode.
//!
//! This module bridges the new DOM-based CSS rendering pipeline with the
//! existing compositor scene graph. It takes the [`DesktopDocument`],
//! runs the full pipeline, and converts the resulting [`DisplayList`]
//! into compositor [`SceneNode`]s that the renderer already knows how to
//! draw.
//!
//! ## Pipeline stages
//!
//! 1. **Style** — `StyleEngine::restyle_all()` → `StyleMap`
//! 2. **Layout** — `LayoutEngine::layout()` → `LayoutTree`
//! 3. **Paint** — `Painter::paint()` → `DisplayList`
//! 4. **Bridge** — `DisplayList` → `Vec<SceneNode>` (this module)

mod helpers;
mod property_trees;
mod scene_bridge;
mod stages;

#[cfg(test)]
mod tests;

use std::sync::{Arc, RwLock};

use liquide_font_rasterizer::database::FontDatabase;
use liquide_layout::{LayoutEngine, LayoutTree};
use liquide_paint::{DisplayList, Painter};
use liquide_style_engine::{StyleEngine, StyleMap};

/// Holds the full pipeline state.
pub struct DesktopPipeline {
    /// CSS style engine with loaded stylesheets.
    pub style_engine: StyleEngine,
    /// Layout engine with viewport and base font.
    pub layout_engine: LayoutEngine,
    /// The painter (stateless).
    pub painter: Painter,
    /// Monotonic id counter for scene nodes generated from the pipeline.
    next_scene_id: u64,
    /// Last computed styles (cached for hit-testing).
    pub last_styles: Option<StyleMap>,
    /// Last computed layout tree (cached for hit-testing).
    pub last_layout: Option<LayoutTree>,
    /// Last computed display list for paint reuse.
    pub last_display_list: Option<DisplayList>,
    /// Image URLs referenced during the last scene build, mapped to their hashed image_id.
    /// The host should load these and register them with the renderer.
    pending_images: Vec<(u64, String)>,
    /// Optional font database for real text measurement.
    font_db: Option<Arc<RwLock<FontDatabase>>>,
}

/// Configuration for the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Viewport width in logical pixels.
    pub width: f32,
    /// Viewport height in logical pixels.
    pub height: f32,
    /// Base font size in pixels.
    pub base_font_size: f32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
            base_font_size: 14.0,
        }
    }
}

/// Output of a full pipeline run.
pub struct PipelineOutput {
    /// Computed styles per node.
    pub styles: StyleMap,
    /// Computed layout boxes.
    pub layout: LayoutTree,
    /// Flat paint commands.
    pub display_list: DisplayList,
}

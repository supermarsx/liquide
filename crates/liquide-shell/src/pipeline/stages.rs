//! Pipeline execution — construction, configuration, and the Style → Layout → Paint stages.

use std::sync::{Arc, RwLock};

use liquide_compositor::scene::SceneNode;
use liquide_dom::Document;
use liquide_font_rasterizer::database::FontDatabase;
use liquide_layout::{DefaultTextMeasurer, LayoutInput, LayoutTree, Size};
use liquide_paint::DisplayList;

use liquide_animation::{AnimationScheduler, TransitionEngine};
use liquide_style_engine::StyleEngine;
use liquide_style_engine::engine::ViewportSize;

use crate::font_text_measurer::FontTextMeasurer;
use crate::theme_loader;

use super::helpers::to_compositor_rect;
use super::{DesktopPipeline, PipelineConfig, PipelineOutput};

impl DesktopPipeline {
    /// Create a new pipeline with the default Liquid Glass theme loaded.
    pub fn new(config: &PipelineConfig) -> Self {
        let viewport = ViewportSize {
            width: config.width,
            height: config.height,
        };

        let mut style_engine = StyleEngine::new(viewport, config.base_font_size);

        // Load the default theme (Night)
        style_engine.add_stylesheet(theme_loader::default_theme_css());

        let layout_engine = liquide_layout::LayoutEngine::new(
            Size {
                width: config.width,
                height: config.height,
            },
            config.base_font_size,
        );

        Self {
            style_engine,
            layout_engine,
            painter: liquide_paint::Painter::new(),
            next_scene_id: 1_000_000,
            frame_counter: 0,
            last_styles: None,
            last_layout: None,
            last_display_list: None,
            pending_images: Vec::new(),
            font_db: None,
            transition_engine: TransitionEngine::new(),
            animation_scheduler: AnimationScheduler::new(),
            prev_styles: std::collections::HashMap::new(),
        }
    }

    /// Return the list of image URLs referenced during the last scene build.
    /// Each entry is `(image_id, url)`. The host should load each image and
    /// call `renderer.register_image(image_id, data)` with the decoded bytes.
    pub fn pending_images(&self) -> &[(u64, String)] {
        &self.pending_images
    }

    /// Load an additional stylesheet (e.g. a user theme override).
    pub fn add_stylesheet(&mut self, css: &str) {
        self.style_engine.add_stylesheet(css);
        self.invalidate_cached_output();
    }

    /// Get the list of @font-face rules parsed from loaded stylesheets.
    /// The caller (e.g. DesktopCompositor) can iterate these and load fonts
    /// into the FontDatabase.
    pub fn font_faces(&self) -> &[liquide_style_engine::engine::PreparedFontFace] {
        self.style_engine.font_faces()
    }

    /// Replace styles with a named theme preset.
    pub fn set_theme(&mut self, preset_css: &str) {
        self.style_engine =
            StyleEngine::new(self.style_engine.viewport, self.style_engine.base_font_size);
        self.style_engine.add_stylesheet(preset_css);
        self.prev_styles.clear();
        self.transition_engine = TransitionEngine::new();
        self.animation_scheduler = AnimationScheduler::new();
        self.invalidate_cached_output();
    }

    /// Update viewport dimensions (e.g. on monitor resolution change).
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        let viewport_changed = (self.style_engine.viewport.width - width).abs() > f32::EPSILON
            || (self.style_engine.viewport.height - height).abs() > f32::EPSILON
            || (self.layout_engine.viewport.width - width).abs() > f32::EPSILON
            || (self.layout_engine.viewport.height - height).abs() > f32::EPSILON;

        self.style_engine
            .set_viewport(ViewportSize { width, height });
        self.layout_engine.viewport = Size { width, height };

        if viewport_changed {
            self.invalidate_cached_output();
        }
    }

    /// Set preferred color scheme used by style media queries.
    pub fn set_preferred_color_scheme(&mut self, scheme: &str) {
        self.style_engine.set_preferred_color_scheme(scheme);
        self.invalidate_cached_output();
    }

    fn invalidate_cached_output(&mut self) {
        self.last_styles = None;
        self.last_layout = None;
        self.last_display_list = None;
    }

    /// Whether the pipeline's cached chrome output is stable and reusable this
    /// frame: every cache (styles / layout / display-list) is populated AND no
    /// animation or transition is running.
    ///
    /// This is the *chrome* half of the shell-level full-scene cache predicate
    /// (`Shell::build_scene`). Combined by the caller with "sync_dom mutated
    /// nothing this frame" (the shell's chrome-changed signal), a `true` here
    /// means the CSS chrome subtree would be byte-identical to the previous
    /// frame, so the shell may reuse its cached assembled root instead of
    /// re-running the pipeline + scene bridge + hit-test rebuild + root
    /// reassembly. We do NOT check `doc.dirty` here: in the shell flow that set
    /// is monotonic (never cleared per-frame), so emptiness is not a reliable
    /// idle signal — the shell tracks chrome changes via `sync_dom`'s return
    /// instead. A theme / viewport / color-scheme change calls
    /// `invalidate_cached_output`, which clears the caches and makes this return
    /// `false`, so those paths correctly force a rebuild.
    #[must_use]
    pub fn chrome_output_stable(&self) -> bool {
        let caches_populated = self.last_styles.is_some()
            && self.last_layout.is_some()
            && self.last_display_list.is_some();
        let animating =
            self.transition_engine.active_count() > 0 || self.animation_scheduler.active_count() > 0;
        caches_populated && !animating
    }

    /// Set the font database for real text measurement.
    ///
    /// When set, the pipeline will use real glyph metrics from loaded
    /// fonts instead of the approximate `char_width = font_size * 0.6`
    /// fallback.
    pub fn set_font_db(&mut self, db: Arc<RwLock<FontDatabase>>) {
        self.font_db = Some(db);
    }

    /// Run the full pipeline: Style → Layout → Paint.
    ///
    /// Returns the style map, layout tree, and display list, plus a flag
    /// indicating whether animations are active (caller should schedule a
    /// follow-up frame when `true`).
    pub fn run(&mut self, doc: &Document, dt_ms: f32) -> (PipelineOutput, bool) {
        // Use real font metrics when a font database is available.
        let font_measurer: Option<FontTextMeasurer> = self
            .font_db
            .as_ref()
            .map(|db| FontTextMeasurer::new(Arc::clone(db)));
        let default_measurer = DefaultTextMeasurer;
        let text_measurer: &dyn liquide_layout::TextMeasurer = match &font_measurer {
            Some(fm) => fm,
            None => &default_measurer,
        };
        let image_measurer = liquide_layout::DefaultImageMeasurer;

        let has_style_work = !doc.dirty.style.is_empty();
        let has_layout_work = !doc.dirty.layout.is_empty();
        let has_paint_work = !doc.dirty.paint.is_empty();

        let caches_populated = self.last_styles.is_some()
            && self.last_layout.is_some()
            && self.last_display_list.is_some();
        let animating =
            self.transition_engine.active_count() > 0 || self.animation_scheduler.active_count() > 0;

        // Fast path: when nothing is dirty, nothing is animating, and all
        // caches are populated, return Arc clones (16-byte pointer copy)
        // without running any pipeline stage.
        if !has_style_work && !has_layout_work && !has_paint_work && caches_populated && !animating {
            return (
                PipelineOutput {
                    styles: Arc::clone(self.last_styles.as_ref().unwrap()),
                    layout: Arc::clone(self.last_layout.as_ref().unwrap()),
                    display_list: Arc::clone(self.last_display_list.as_ref().unwrap()),
                },
                false,
            );
        }

        // Scoped-animation path: an animation/transition is running but NOTHING
        // in the DOM is dirty and every cache is populated. Previously this fell
        // through to the FULL pipeline (restyle_all + full layout + full paint)
        // every frame for the whole tree — so a single 1-element fade re-styled
        // and re-laid-out all static chrome each frame (t68-perf cause #3).
        //
        // Instead, reuse the cached styles/layout and re-derive ONLY the
        // animating subtrees: apply the per-frame animation/transition overrides
        // (already scoped to animating nodes by `tick_and_apply`), then relayout
        // just those nodes' subtrees against the cached layout. The style and
        // layout caches for every NON-animating node are kept verbatim.
        if !has_style_work
            && !has_layout_work
            && !has_paint_work
            && caches_populated
            && animating
        {
            if let Some(out) =
                self.run_scoped_animation(doc, dt_ms, text_measurer, &image_measurer)
            {
                return out;
            }
            // Fall through to the full pipeline if scoping was not applicable
            // (e.g. an animating node's subtree could not be relayout-ed
            // incrementally — correctness first).
        }

        // 1. Style — unwrap Arc for mutation (try_unwrap succeeds when we're
        //    the sole owner, otherwise falls back to clone).
        let mut styles = if has_style_work {
            if let Some(arc) = self.last_styles.take() {
                let mut cached = match Arc::try_unwrap(arc) {
                    Ok(s) => s,
                    Err(a) => (*a).clone(),
                };
                let changed: Vec<liquide_dom::NodeId> = doc.dirty.style.iter().copied().collect();
                self.style_engine.invalidate(doc, &changed, &mut cached);
                cached
            } else {
                self.style_engine.restyle_all(doc)
            }
        } else if let Some(arc) = self.last_styles.take() {
            match Arc::try_unwrap(arc) {
                Ok(s) => s,
                Err(a) => (*a).clone(),
            }
        } else {
            self.style_engine.restyle_all(doc)
        };

        // 2. Layout — unwrap Arc for mutation
        let recompute_layout = has_style_work || has_layout_work || self.last_layout.is_none();
        // `layout_was_full` gates the TODO-11 container second pass: a fresh full
        // style+layout is the only case whose first style pass used viewport-
        // fallback container sizes (an incremental relayout reuses prior styles
        // that already carried measured container sizes).
        let layout_was_full = has_style_work || self.last_layout.is_none();
        let layout = if has_style_work || self.last_layout.is_none() {
            // Full style recompute invalidates layout cache
            let _ = self.last_layout.take();
            self.layout_engine
                .layout(doc, &styles, text_measurer, &image_measurer)
        } else if has_layout_work {
            let mut layout = match self.last_layout.take() {
                Some(arc) => match Arc::try_unwrap(arc) {
                    Ok(l) => l,
                    Err(a) => (*a).clone(),
                },
                None => LayoutTree::default(),
            };
            let input = LayoutInput::new(doc, &styles, text_measurer, &image_measurer);

            let mut dirty_layout_nodes: Vec<liquide_dom::NodeId> =
                doc.dirty.layout.iter().copied().collect();
            dirty_layout_nodes.sort_by_key(|node_id| doc.ancestors(*node_id).len());

            // If both an ancestor and descendant are dirty, relayout the ancestor only.
            let mut relayout_roots: Vec<liquide_dom::NodeId> = Vec::new();
            for node_id in dirty_layout_nodes {
                let ancestors = doc.ancestors(node_id);
                if relayout_roots
                    .iter()
                    .any(|selected| ancestors.iter().any(|a| a == selected))
                {
                    continue;
                }
                relayout_roots.push(node_id);
            }

            for node_id in relayout_roots {
                layout = self
                    .layout_engine
                    .relayout_subtree(&input, node_id, &layout);
            }

            layout
        } else {
            match self.last_layout.take() {
                Some(arc) => match Arc::try_unwrap(arc) {
                    Ok(l) => l,
                    Err(a) => (*a).clone(),
                },
                None => LayoutTree::default(),
            }
        };

        // 2b. Populate container sizes for the next @container evaluation.
        // Elements with container-type != normal get their resolved dimensions
        // stored in the StyleMap so that `evaluate_container_condition` can use
        // real dimensions instead of falling back to the viewport.
        let mut container_hosts: Vec<liquide_dom::NodeId> = Vec::new();
        for layout_box in &layout.boxes {
            if let Some(style) = styles.get(layout_box.node) {
                if style.is_container_query_host() {
                    container_hosts.push(layout_box.node);
                    styles.set_container_size(
                        layout_box.node,
                        layout_box.content_rect.width,
                        layout_box.content_rect.height,
                    );
                }
            }
        }

        // 2b-ii. TODO 11 — forced container-query SECOND PASS.
        //
        // The first style pass (step 1) evaluated every `@container` rule with NO
        // container sizes recorded yet, so `find_matching_container` fell back to
        // the VIEWPORT size (media.rs). Step 2b has now recorded the REAL
        // resolved container dimensions. When those differ from the viewport the
        // first pass used, the `@container` rules were evaluated against the wrong
        // size, so we must re-style the container subtrees (now that the real
        // sizes are present in `styles`) and re-run layout.
        //
        // This is BOUNDED: a single extra style+layout pass per frame. We do NOT
        // loop — a container whose own size depends on its descendants' restyled
        // sizes could otherwise oscillate; one corrective pass is the documented
        // contract (CSS container queries are explicitly single-pass per the
        // spec's "containment" requirement, but our hosts are not size-contained,
        // so one re-evaluation against the measured size is the pragmatic fix).
        let needs_container_pass = layout_was_full
            && !container_hosts.is_empty()
            && container_hosts.iter().any(|&host| {
                styles
                    .container_size(host)
                    .is_some_and(|(cw, ch)| {
                        (cw - self.layout_engine.viewport.width).abs() > 0.5
                            || (ch - self.layout_engine.viewport.height).abs() > 0.5
                    })
            });

        let layout = if needs_container_pass {
            // Re-cascade each container host subtree WITH the measured sizes now
            // present in `styles`, so descendant `@container` rules re-evaluate.
            for &host in &container_hosts {
                self.style_engine.restyle_subtree(doc, host, &mut styles);
            }
            // Re-run layout against the corrected styles.
            let relaid = self
                .layout_engine
                .layout(doc, &styles, text_measurer, &image_measurer);
            // Refresh stored container sizes from the corrected layout so the
            // cached output (and any consumer) reflects the second-pass geometry.
            for layout_box in &relaid.boxes {
                if let Some(style) = styles.get(layout_box.node) {
                    if style.is_container_query_host() {
                        styles.set_container_size(
                            layout_box.node,
                            layout_box.content_rect.width,
                            layout_box.content_rect.height,
                        );
                    }
                }
            }
            relaid
        } else {
            layout
        };

        // 2c. Animation — detect transitions/animations and tick.
        // Must run after style computation but before paint so that
        // interpolated values are visible in the display list.

        // Bridge @keyframes from style engine → animation scheduler
        for (_name, kf_rule) in &self.style_engine.keyframes {
            if !self.animation_scheduler.has_keyframes(&kf_rule.name) {
                self.animation_scheduler.register_keyframes(kf_rule.clone());
            }
        }

        self.detect_transitions(&styles);
        self.detect_animations(&styles);
        let animations_active = self.tick_and_apply(dt_ms, &mut styles);
        if animations_active || has_style_work {
            self.snapshot_styles(&styles);
        }

        // 3. Paint — unwrap Arc for mutation
        let recompute_paint =
            recompute_layout || has_paint_work || self.last_display_list.is_none();
        let display_list = if recompute_paint {
            let _ = self.last_display_list.take();
            self.painter.paint(doc, &layout, &styles)
        } else {
            match self.last_display_list.take() {
                Some(arc) => match Arc::try_unwrap(arc) {
                    Ok(dl) => dl,
                    Err(a) => (*a).clone(),
                },
                None => DisplayList::default(),
            }
        };

        // Wrap in Arc and cache for next frame (Arc::clone is a 16-byte pointer copy)
        let styles = Arc::new(styles);
        let layout = Arc::new(layout);
        let display_list = Arc::new(display_list);
        self.last_styles = Some(Arc::clone(&styles));
        self.last_layout = Some(Arc::clone(&layout));
        self.last_display_list = Some(Arc::clone(&display_list));

        (
            PipelineOutput {
                styles,
                layout,
                display_list,
            },
            animations_active,
        )
    }

    /// Scoped per-frame advance for active animations/transitions when no DOM
    /// mutation occurred.
    ///
    /// Reuses the cached `StyleMap` and `LayoutTree` and only re-derives the
    /// animating subtrees:
    ///   1. Clone the cached styles (so non-animating nodes keep their exact
    ///      cached `ComputedStyle`), then run `tick_and_apply`, which writes the
    ///      interpolated transition/animation values onto the animating nodes
    ///      ONLY.
    ///   2. Relayout just the animating nodes' subtrees against the cached
    ///      layout tree (the layout engine keeps every other box untouched and
    ///      falls back to a full pass only if a subtree cannot be relaid
    ///      incrementally).
    ///   3. Repaint into a fresh display list (the painter has no public partial
    ///      API; the win here is skipping the whole-tree restyle + full layout
    ///      that the old path did every animation frame).
    ///
    /// Returns `None` to signal the caller should run the full pipeline instead
    /// (no animating nodes resolved, or the cache was unexpectedly empty).
    fn run_scoped_animation(
        &mut self,
        doc: &Document,
        dt_ms: f32,
        text_measurer: &dyn liquide_layout::TextMeasurer,
        image_measurer: &liquide_layout::DefaultImageMeasurer,
    ) -> Option<(PipelineOutput, bool)> {
        // Clone cached styles — non-animating nodes are preserved verbatim;
        // `tick_and_apply` mutates only the animating nodes.
        let cached_styles = self.last_styles.take()?;
        let mut styles = match Arc::try_unwrap(cached_styles) {
            Ok(s) => s,
            Err(a) => (*a).clone(),
        };

        // Bridge @keyframes from style engine → scheduler (cheap no-op when
        // already registered) so newly-registered keyframes resolve.
        for (_name, kf_rule) in &self.style_engine.keyframes {
            if !self.animation_scheduler.has_keyframes(&kf_rule.name) {
                self.animation_scheduler.register_keyframes(kf_rule.clone());
            }
        }

        // Snapshot the set of animating nodes BEFORE the tick (their cached
        // styles still carry `animation_name`, and the transition engine still
        // lists their running properties). These are the ONLY subtrees we will
        // relayout.
        let mut animating_nodes: std::collections::HashSet<liquide_dom::NodeId> =
            std::collections::HashSet::new();
        for (node_id, _prop, _val) in self.transition_engine.active_overrides() {
            animating_nodes.insert(node_id);
        }
        for (node_id, style) in styles.iter() {
            if style.animation_name.is_some() {
                animating_nodes.insert(*node_id);
            }
        }

        // Apply this frame's interpolated values onto the animating nodes only.
        let animations_active = self.tick_and_apply(dt_ms, &mut styles);
        // Snapshot for next-frame transition detection (mirrors the full path).
        self.snapshot_styles(&styles);

        if animating_nodes.is_empty() {
            // Nothing actually animating after the tick — let the full pipeline
            // (or the next-frame fast path) take over. Restore the cache.
            self.last_styles = Some(Arc::new(styles));
            return None;
        }

        // Relayout ONLY the animating subtrees against the cached layout.
        let cached_layout = self.last_layout.take()?;
        let mut layout = match Arc::try_unwrap(cached_layout) {
            Ok(l) => l,
            Err(a) => (*a).clone(),
        };
        let input = LayoutInput::new(doc, &styles, text_measurer, image_measurer);

        // Collapse animating descendants under animating ancestors so we never
        // relayout the same subtree twice.
        let mut roots: Vec<liquide_dom::NodeId> = Vec::new();
        let mut sorted: Vec<liquide_dom::NodeId> = animating_nodes.iter().copied().collect();
        sorted.sort_by_key(|node_id| doc.ancestors(*node_id).len());
        for node_id in sorted {
            let ancestors = doc.ancestors(node_id);
            if roots
                .iter()
                .any(|selected| ancestors.iter().any(|a| a == selected))
            {
                continue;
            }
            roots.push(node_id);
        }
        for node_id in roots {
            layout = self
                .layout_engine
                .relayout_subtree(&input, node_id, &layout);
        }

        // Repaint (whole-tree paint; the avoided cost is the full restyle +
        // full layout the old animating path ran every frame).
        let display_list = self.painter.paint(doc, &layout, &styles);

        let styles = Arc::new(styles);
        let layout = Arc::new(layout);
        let display_list = Arc::new(display_list);
        self.last_styles = Some(Arc::clone(&styles));
        self.last_layout = Some(Arc::clone(&layout));
        self.last_display_list = Some(Arc::clone(&display_list));

        Some((
            PipelineOutput {
                styles,
                layout,
                display_list,
            },
            animations_active,
        ))
    }

    /// Run the full pipeline and convert the result to compositor SceneNodes.
    ///
    /// Glass SceneNodes are generated for elements with `blur-radius` CSS
    /// property. These are placed *before* the element's normal paint output
    /// so the blur effect renders behind the content.
    pub fn render_to_scene(
        &mut self,
        doc: &Document,
        base_z: u32,
        dt_ms: f32,
    ) -> (Vec<SceneNode>, bool) {
        let (nodes, _output, animations_active) =
            self.render_to_scene_with_output(doc, base_z, dt_ms);
        (nodes, animations_active)
    }

    /// Like [`render_to_scene`] but also returns the pipeline output
    /// (styles + layout) for downstream use (e.g. hit-testing).
    pub fn render_to_scene_with_output(
        &mut self,
        doc: &Document,
        base_z: u32,
        dt_ms: f32,
    ) -> (Vec<SceneNode>, PipelineOutput, bool) {
        // Use a frame-based offset so each frame gets a unique ID range.
        // Prevents cross-frame aliasing in the blur_worker cache.
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.next_scene_id = 1_000_000 + (self.frame_counter % 1000) * 100_000;

        let (output, animations_active) = self.run(doc, dt_ms);

        // Collect Glass nodes from elements with x_blur_radius > 0.
        let glass_nodes = self.extract_glass_nodes(&output, base_z);
        let glass_count = glass_nodes.len() as u32;

        // Convert paint output to scene nodes, offset z by glass count.
        let mut nodes = glass_nodes;
        let paint_nodes = self.display_list_to_scene(&output.display_list, base_z + glass_count);
        nodes.extend(paint_nodes);

        (nodes, output, animations_active)
    }

    /// Generate Glass SceneNodes for DOM elements that have `x_blur_radius > 0`
    /// in their computed style. Uses the layout tree to get the element's rect.
    fn extract_glass_nodes(&mut self, output: &PipelineOutput, base_z: u32) -> Vec<SceneNode> {
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNodeKind};

        let mut glass_nodes = Vec::new();
        let mut z = base_z;

        for layout_box in &output.layout.boxes {
            if let Some(style) = output.styles.get(layout_box.node) {
                if style.x_blur_radius > 0.0 {
                    let abs_border = output.layout.absolute_border_rect(layout_box.id);
                    let rect = to_compositor_rect(&abs_border);
                    // Skip zero-area boxes
                    if rect.width <= 0.0 || rect.height <= 0.0 {
                        continue;
                    }

                    let tint_color = style.x_glass_tint.unwrap_or_else(|| {
                        // Fall back to background_color if no glass-tint
                        style.background_color
                    });

                    let id = self.alloc_id();
                    let glass = SceneNode::new(
                        id,
                        SceneNodeKind::Glass(GlassParams {
                            blur_radius: style.x_blur_radius as u32,
                            tint_color,
                            inner_glow: true,
                            parallax: false,
                        }),
                        NodeProperties::new(rect).with_z_order(z),
                    );
                    glass_nodes.push(glass);
                    z += 1;
                }
            }
        }

        glass_nodes
    }

    pub(super) fn alloc_id(&mut self) -> u64 {
        let id = self.next_scene_id;
        self.next_scene_id += 1;
        id
    }
}

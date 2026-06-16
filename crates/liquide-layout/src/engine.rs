//! Layout engine — the main entry point for computing layout.

use liquide_dom::{Document, NodeId};
use liquide_layout_cache::{
    DirtyPropagation, LayoutCache, LayoutConstraints, LayoutDirtyFlags,
    LayoutResult as CachedLayoutResult,
};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, Position};

use crate::geometry::{Rect, Size};
use crate::tree::{LayoutBox, LayoutBoxId, LayoutTree};
use crate::writing_mode::WritingModeContext;
use crate::{ImageMeasurer, TextMeasurer};

/// Map the 5-variant style-engine `WritingMode` to the cache's writing mode.
fn map_writing_mode(
    wm: liquide_style_engine::computed::WritingMode,
) -> liquide_layout_cache::WritingMode {
    use liquide_layout_cache::WritingMode as C;
    use liquide_style_engine::computed::WritingMode as S;
    match wm {
        S::HorizontalTb => C::HorizontalTb,
        S::VerticalRl => C::VerticalRl,
        S::VerticalLr => C::VerticalLr,
        S::SidewaysRl => C::SidewaysRl,
        S::SidewaysLr => C::SidewaysLr,
    }
}

/// Map style-engine `Direction` to the cache's direction enum.
fn map_direction(d: liquide_style_engine::computed::Direction) -> liquide_layout_cache::Direction {
    use liquide_layout_cache::Direction as C;
    use liquide_style_engine::computed::Direction as S;
    match d {
        S::Ltr => C::LTR,
        S::Rtl => C::RTL,
    }
}

/// The layout engine. Computes geometry for all elements in the document.
pub struct LayoutEngine {
    /// Viewport size.
    pub viewport: Size,
    /// Root font size for `rem` units.
    pub base_font_size: f32,
    /// Per-node layout result cache for incremental optimization.
    cache: LayoutCache,
    /// Tracks which nodes need re-layout for incremental optimization.
    dirty: DirtyPropagation,
    /// When `true`, skip all cache lookups (useful for debugging/testing).
    bypass_cache: bool,
    /// Writing-mode context captured from the root element on each layout
    /// pass. Populated by [`LayoutEngine::layout`] from the root element's
    /// computed `writing-mode` + `direction`. Downstream consumers (paint,
    /// scroll, selection) can read this to honour vertical writing modes
    /// at the page root.
    root_writing_mode: WritingModeContext,
    /// Root layout constraints — the parent-pass input that would be used
    /// for the root box. Carries writing-mode, direction, and root font
    /// size so that incremental restyle against the root can participate
    /// in the same cache-key scheme as nested boxes.
    root_constraints: LayoutConstraints,
}

/// Bundled input for layout and relayout APIs.
pub struct LayoutInput<
    'a,
    TM: TextMeasurer + ?Sized = dyn TextMeasurer,
    IM: ImageMeasurer + ?Sized = dyn ImageMeasurer,
> {
    pub doc: &'a Document,
    pub styles: &'a StyleMap,
    pub text_measurer: &'a TM,
    pub image_measurer: &'a IM,
}

impl<'a, TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized> LayoutInput<'a, TM, IM> {
    pub fn new(
        doc: &'a Document,
        styles: &'a StyleMap,
        text_measurer: &'a TM,
        image_measurer: &'a IM,
    ) -> Self {
        Self {
            doc,
            styles,
            text_measurer,
            image_measurer,
        }
    }
}

impl LayoutEngine {
    /// Create a new layout engine.
    pub fn new(viewport: Size, base_font_size: f32) -> Self {
        Self {
            viewport,
            base_font_size,
            cache: LayoutCache::new(),
            dirty: DirtyPropagation::new(),
            bypass_cache: false,
            root_writing_mode: WritingModeContext::default(),
            root_constraints: LayoutConstraints::fixed(viewport.width, viewport.height)
                .with_font_size(base_font_size),
        }
    }

    /// Writing-mode context captured from the root element on the most
    /// recent layout pass. Default until [`Self::layout`] runs at least once.
    pub fn root_writing_mode(&self) -> WritingModeContext {
        self.root_writing_mode
    }

    /// Root layout constraints carrying the root writing-mode/direction
    /// and font-size used for the most recent layout pass.
    pub fn root_constraints(&self) -> &LayoutConstraints {
        &self.root_constraints
    }

    /// Run layout on the entire document.
    pub fn layout<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &mut self,
        doc: &Document,
        styles: &StyleMap,
        text_measurer: &TM,
        image_measurer: &IM,
    ) -> LayoutTree {
        // Advance the cache generation and evict entries older than 3 frames.
        self.cache.advance_generation(3);

        let mut tree = LayoutTree::new();

        // Reset the thread-local counter registry for this layout pass.
        crate::counter::COUNTER_REGISTRY.with(|reg| {
            *reg.borrow_mut() = crate::counter::CounterRegistry::new();
        });

        let root = doc.root();

        let root_style = styles.get(root).cloned().unwrap_or_default();

        // Read writing-mode and direction from the root element's computed style.
        // These determine the document's inline/block axis mapping. The
        // resolved context is stored on the engine so downstream consumers
        // (paint / scroll / selection) can observe vertical writing modes
        // at the page root, and so incremental relayouts re-key the cache
        // correctly via `root_constraints`.
        let root_wm =
            WritingModeContext::with_direction(root_style.writing_mode, root_style.direction);
        self.root_writing_mode = root_wm;
        self.root_constraints = LayoutConstraints::fixed(self.viewport.width, self.viewport.height)
            .with_writing_mode(
                map_writing_mode(root_style.writing_mode),
                map_direction(root_style.direction),
            )
            .with_font_size(if root_style.font_size > 0.0 {
                root_style.font_size
            } else {
                self.base_font_size
            });

        // Root layout starts as block
        // display: contents on root — treat as block (root must generate a box)
        let root_box = if root_style.is_flex_container() {
            crate::flex::layout_flex(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_grid_container() {
            crate::grid::layout_grid(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_table() {
            crate::table::layout_table(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_multicol() {
            crate::multicol::layout_multicol(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if matches!(
            root_style.display,
            liquide_style_engine::computed::Display::Inline
        ) {
            crate::inline::layout_inline(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                self.viewport.width,
                0.0,
                0.0,
            )
        } else {
            crate::block::layout_block(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        };

        tree.root = root_box;

        // Ensure root box has viewport dimensions for hit testing
        // (root may have height 0 if all children are positioned out of flow)
        if let Some(root_box_ref) = tree.get_mut(root_box) {
            let vp_rect = Rect {
                x: 0.0,
                y: 0.0,
                width: self.viewport.width,
                height: self.viewport.height,
            };
            root_box_ref.content_rect = vp_rect;
            root_box_ref.padding_rect = vp_rect;
            root_box_ref.border_rect = vp_rect;
            root_box_ref.margin_rect = vp_rect;
        }

        // Second pass: register anchor names for anchor positioning
        self.register_anchors(doc, root, styles, &mut tree);

        // Third pass: layout positioned elements
        self.layout_positioned_elements(
            doc,
            root,
            styles,
            &mut tree,
            text_measurer,
            image_measurer,
        );

        // Third pass: apply relative positioning offsets
        Self::apply_relative_offsets(&mut tree, styles, self.base_font_size);

        // Fourth pass: adjust sticky-positioned elements based on scroll offsets
        Self::apply_sticky_offsets(&mut tree, styles, doc, self.base_font_size);

        // Populate the layout cache with results from this pass.
        self.populate_cache_from_tree(&tree);

        // All nodes have been laid out; clear dirty flags for next frame.
        self.dirty.clear_all();

        tree
    }

    /// Run layout using a bundled input object.
    pub fn layout_with_input<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &mut self,
        input: &LayoutInput<'_, TM, IM>,
    ) -> LayoutTree {
        self.layout(
            input.doc,
            input.styles,
            input.text_measurer,
            input.image_measurer,
        )
    }

    /// Incremental relayout entrypoint.
    pub fn relayout_subtree<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &mut self,
        input: &LayoutInput<'_, TM, IM>,
        node_id: NodeId,
        previous_tree: &LayoutTree,
    ) -> LayoutTree {
        if node_id == input.doc.root()
            || !self.supports_incremental_relayout(input.doc, input.styles, node_id)
        {
            return self.layout_with_input(input);
        }

        let Some(old_box_id) = previous_tree.find_box_id_by_node(node_id) else {
            return self.layout_with_input(input);
        };
        let Some(old_box) = previous_tree.get(old_box_id) else {
            return self.layout_with_input(input);
        };
        let Some(parent_box_id) = old_box.parent else {
            return self.layout_with_input(input);
        };
        let Some(parent_box) = previous_tree.get(parent_box_id) else {
            return self.layout_with_input(input);
        };
        let Some(replace_index) = parent_box.children.iter().position(|&id| id == old_box_id)
        else {
            return self.layout_with_input(input);
        };

        let container_width = parent_box.content_rect.width;
        let container_height = parent_box.content_rect.height;
        let old_origin = old_box.margin_rect;
        let old_margin_height = old_box.margin_rect.height;

        // Relayout only the requested subtree in a temporary tree.
        let mut relaid_subtree = LayoutTree::new();
        let relaid_root = self.layout_node_in_context(
            input,
            node_id,
            &mut relaid_subtree,
            container_width,
            container_height,
            old_origin.x,
            old_origin.y,
        );
        relaid_subtree.root = relaid_root;
        self.layout_positioned_elements(
            input.doc,
            node_id,
            input.styles,
            &mut relaid_subtree,
            input.text_measurer,
            input.image_measurer,
        );
        Self::apply_relative_offsets(&mut relaid_subtree, input.styles, self.base_font_size);

        // Align the newly generated subtree to the original subtree origin.
        if let Some(new_root_box) = relaid_subtree.get(relaid_root) {
            let dx = old_origin.x - new_root_box.margin_rect.x;
            let dy = old_origin.y - new_root_box.margin_rect.y;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                Self::shift_subtree(&mut relaid_subtree, relaid_root, dx, dy);
            }
        }

        // Replace the old subtree with the newly laid out one.
        let mut result = previous_tree.clone();
        let mut old_ids = Vec::new();
        Self::collect_subtree_box_ids(&result, old_box_id, &mut old_ids);

        if let Some(parent) = result.get_mut(parent_box_id) {
            if replace_index < parent.children.len() && parent.children[replace_index] == old_box_id
            {
                parent.children.remove(replace_index);
            } else {
                parent.children.retain(|&id| id != old_box_id);
            }
        } else {
            return self.layout_with_input(input);
        }

        for old_id in &old_ids {
            if let Some(old_layout_box) = result.get(*old_id) {
                result.clear_node_box_if(old_layout_box.node, *old_id);
            }
        }

        let new_root_id = Self::clone_subtree_into(
            &relaid_subtree,
            relaid_root,
            &mut result,
            Some(parent_box_id),
        );
        if let Some(parent) = result.get_mut(parent_box_id) {
            let insert_at = replace_index.min(parent.children.len());
            parent.children.insert(insert_at, new_root_id);
        }

        let new_margin_height = result
            .get(new_root_id)
            .map(|b| b.margin_rect.height)
            .unwrap_or(old_margin_height);
        let delta_h = new_margin_height - old_margin_height;

        if delta_h.abs() > 0.001 {
            Self::propagate_block_flow_delta(
                &mut result,
                parent_box_id,
                replace_index,
                delta_h,
                input.doc,
                input.styles,
            );
        }

        Self::apply_sticky_offsets(&mut result, input.styles, input.doc, self.base_font_size);
        result
    }

    fn supports_incremental_relayout(
        &self,
        doc: &Document,
        styles: &StyleMap,
        node_id: NodeId,
    ) -> bool {
        let node_style = styles.get(node_id).cloned().unwrap_or_default();
        if matches!(
            node_style.position,
            Position::Absolute | Position::Fixed | Position::Sticky
        ) {
            return false;
        }

        let Some(mut current) = doc.parent(node_id) else {
            return false;
        };

        loop {
            let style = styles.get(current).cloned().unwrap_or_default();
            if !Self::is_simple_block_flow_container(&style) {
                return false;
            }
            match doc.parent(current) {
                Some(parent) => current = parent,
                None => break,
            }
        }

        true
    }

    fn is_simple_block_flow_container(
        style: &liquide_style_engine::computed::ComputedStyle,
    ) -> bool {
        matches!(
            style.display,
            Display::Block | Display::FlowRoot | Display::ListItem
        ) && !style.is_flex_container()
            && !style.is_grid_container()
            && !style.is_table()
            && !style.is_multicol()
            && !matches!(
                style.position,
                Position::Absolute | Position::Fixed | Position::Sticky
            )
    }

    fn layout_node_in_context<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &mut self,
        input: &LayoutInput<'_, TM, IM>,
        node_id: NodeId,
        tree: &mut LayoutTree,
        container_width: f32,
        container_height: f32,
        offset_x: f32,
        offset_y: f32,
    ) -> LayoutBoxId {
        // ── Cache-accelerated fast path (leaf-only, fail-safe) ────────
        //
        // If dirty tracking is active (at least one node has been marked)
        // and this node — plus all its descendants — are clean, try the
        // cache.  On hit we reconstruct the box and skip the expensive
        // recursive layout entirely.
        //
        // SAFETY (t49-e3-F2 fix): `CachedLayoutResult` only records child
        // *offsets*, not child node identities or their box subtrees, so a
        // cache hit cannot faithfully rebuild a node that produced children
        // — reconstructing a childless box here would silently DROP the
        // entire subtree.  We therefore only honor a cache hit when the
        // stored result describes a true leaf (`child_offsets.is_empty()`);
        // any node that laid out children falls through to full layout.
        // This keeps the fast path sound regardless of how dirty/cache keys
        // are driven (the wiring itself is still staged — see `dirty.rs`).
        if !self.bypass_cache && self.dirty.dirty_count() > 0 {
            let needs_layout = self.dirty.needs_layout(node_id);
            let has_any_dirty = self.dirty.has_dirty_flags(node_id);
            if !needs_layout && !has_any_dirty {
                // STAGED-KEY LIMITATION (t49-e3-F3/F4): this key only carries the
                // available width/height — it drops writing-mode, direction, and
                // font-size, all of which `LayoutConstraints` *can* carry and the
                // populate-from-tree path also currently omits. While the cache is
                // not driven in production this is latent, but a future wirer must
                // build a full key (writing mode + direction + font-size) on both
                // store and lookup before enabling this path, or a leaf measured
                // under one writing mode could be returned under another.
                let constraints = LayoutConstraints::fixed(container_width, container_height);
                if let Some(cached) = self.cache.lookup(node_id, &constraints) {
                    if cached.child_offsets.is_empty() {
                        let cached: liquide_layout_cache::LayoutResult = cached.clone();
                        let box_id = tree.alloc(node_id, crate::tree::BoxType::Block);
                        if let Some(b) = tree.get_mut(box_id) {
                            let (w, h) = cached.size;
                            let (mt, mr, mb, ml) = cached.margins;
                            b.border_rect = Rect::new(offset_x + ml, offset_y + mt, w, h);
                            b.margin_rect = Rect::new(offset_x, offset_y, ml + w + mr, mt + h + mb);
                            b.padding_rect = b.border_rect;
                            b.content_rect = b.border_rect;
                            b.baseline = cached.baseline;
                        }
                        return box_id;
                    }
                    // Non-leaf cache entry: cannot faithfully reconstruct the
                    // subtree from offsets alone — fall through to full layout.
                }
            }
        }

        let style = input.styles.get(node_id).cloned().unwrap_or_default();

        // display: contents — skip this node, promote children.
        // Create a wrapper block box to hold the promoted children.
        if matches!(style.display, Display::Contents) {
            let wrapper = tree.alloc(node_id, crate::tree::BoxType::Block);
            let children = input.doc.children(node_id).to_vec();
            let mut child_y = 0.0f32;
            for &child_id in &children {
                let child_style = input.styles.get(child_id).cloned().unwrap_or_default();
                if child_style.display == Display::None {
                    continue;
                }
                let child_box = self.layout_node_in_context(
                    input,
                    child_id,
                    tree,
                    container_width,
                    container_height,
                    offset_x,
                    offset_y + child_y,
                );
                tree.add_child(wrapper, child_box);
                if let Some(cb) = tree.get(child_box) {
                    child_y += cb.margin_rect.height;
                }
            }
            if let Some(b) = tree.get_mut(wrapper) {
                b.content_rect = Rect::new(offset_x, offset_y, container_width, child_y);
                b.padding_rect = b.content_rect;
                b.border_rect = b.content_rect;
                b.margin_rect = b.content_rect;
            }

            // Store display:contents wrapper in cache before returning.
            if !self.bypass_cache {
                let constraints = LayoutConstraints::fixed(container_width, container_height);
                if let Some(layout_box) = tree.get(wrapper) {
                    let cached_result = Self::extract_cached_result(layout_box, tree);
                    self.cache.store(node_id, constraints, cached_result);
                }
                self.dirty.clear(node_id);
            }
            return wrapper;
        }

        let result_box_id = if style.is_flex_container() {
            crate::flex::layout_flex(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                input.image_measurer,
                container_width,
                container_height,
                offset_x,
                offset_y,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if style.is_grid_container() {
            crate::grid::layout_grid(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                input.image_measurer,
                container_width,
                container_height,
                offset_x,
                offset_y,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if style.is_table() {
            crate::table::layout_table(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                input.image_measurer,
                container_width,
                container_height,
                offset_x,
                offset_y,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if style.is_multicol() {
            crate::multicol::layout_multicol(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                input.image_measurer,
                container_width,
                container_height,
                offset_x,
                offset_y,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if matches!(style.display, Display::Inline) {
            crate::inline::layout_inline(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                container_width,
                offset_x,
                offset_y,
            )
        } else {
            crate::block::layout_block(
                input.doc,
                node_id,
                input.styles,
                tree,
                input.text_measurer,
                input.image_measurer,
                container_width,
                container_height,
                offset_x,
                offset_y,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        };

        // ── Post-layout: store result in cache and clear dirty flag ──
        if !self.bypass_cache {
            let constraints = LayoutConstraints::fixed(container_width, container_height);
            if let Some(layout_box) = tree.get(result_box_id) {
                let cached_result = Self::extract_cached_result(layout_box, tree);
                self.cache.store(node_id, constraints, cached_result);
            }
            self.dirty.clear(node_id);
        }

        result_box_id
    }

    fn collect_subtree_box_ids(tree: &LayoutTree, box_id: LayoutBoxId, out: &mut Vec<LayoutBoxId>) {
        out.push(box_id);
        if let Some(layout_box) = tree.get(box_id) {
            for &child in &layout_box.children {
                Self::collect_subtree_box_ids(tree, child, out);
            }
        }
    }

    fn shift_subtree(tree: &mut LayoutTree, box_id: LayoutBoxId, dx: f32, dy: f32) {
        let children = if let Some(layout_box) = tree.get_mut(box_id) {
            layout_box.content_rect.x += dx;
            layout_box.content_rect.y += dy;
            layout_box.padding_rect.x += dx;
            layout_box.padding_rect.y += dy;
            layout_box.border_rect.x += dx;
            layout_box.border_rect.y += dy;
            layout_box.margin_rect.x += dx;
            layout_box.margin_rect.y += dy;
            layout_box.children.clone()
        } else {
            return;
        };

        for child in children {
            Self::shift_subtree(tree, child, dx, dy);
        }
    }

    fn clone_subtree_into(
        src: &LayoutTree,
        src_box_id: LayoutBoxId,
        dst: &mut LayoutTree,
        parent: Option<LayoutBoxId>,
    ) -> LayoutBoxId {
        let src_box = src
            .get(src_box_id)
            .cloned()
            .expect("incremental relayout attempted to clone a missing source box");

        let new_id = dst.boxes.len();
        let src_children = src_box.children.clone();

        let mut new_box = src_box;
        new_box.id = new_id;
        new_box.parent = parent;
        new_box.children.clear();

        dst.boxes.push(new_box);
        let node_id = dst.boxes[new_id].node;
        dst.set_node_box(node_id, new_id);

        let mut mapped_children = Vec::with_capacity(src_children.len());
        for child in src_children {
            let mapped = Self::clone_subtree_into(src, child, dst, Some(new_id));
            mapped_children.push(mapped);
        }

        if let Some(mapped_box) = dst.get_mut(new_id) {
            mapped_box.children = mapped_children;
        }

        new_id
    }

    fn propagate_block_flow_delta(
        tree: &mut LayoutTree,
        mut parent_box_id: LayoutBoxId,
        mut changed_child_index: usize,
        delta_h: f32,
        _doc: &Document,
        styles: &StyleMap,
    ) {
        const MAX_PROPAGATION_DEPTH: usize = 64;
        let outward_delta = delta_h;
        let mut depth = 0usize;
        while outward_delta.abs() > 0.001 {
            depth += 1;
            if depth > MAX_PROPAGATION_DEPTH {
                tracing::warn!(
                    "propagate_block_flow_delta: exceeded max depth {}, aborting propagation",
                    MAX_PROPAGATION_DEPTH,
                );
                break;
            }
            let Some(parent_snapshot) = tree.get(parent_box_id).cloned() else {
                break;
            };

            let following_siblings: Vec<LayoutBoxId> = parent_snapshot
                .children
                .iter()
                .skip(changed_child_index + 1)
                .copied()
                .collect();
            for sibling_id in following_siblings {
                Self::shift_subtree(tree, sibling_id, 0.0, outward_delta);
            }

            let parent_style = styles
                .get(parent_snapshot.node)
                .cloned()
                .unwrap_or_default();
            if parent_style.height.is_definite() {
                break;
            }

            if let Some(parent) = tree.get_mut(parent_box_id) {
                parent.content_rect.height = (parent.content_rect.height + outward_delta).max(0.0);
                parent.padding_rect.height = (parent.padding_rect.height + outward_delta).max(0.0);
                parent.border_rect.height = (parent.border_rect.height + outward_delta).max(0.0);
                parent.margin_rect.height = (parent.margin_rect.height + outward_delta).max(0.0);
                if let Some(scroll_size) = parent.scroll_size.as_mut() {
                    scroll_size.height = (scroll_size.height + outward_delta).max(0.0);
                }
            }

            let Some(grandparent_id) = parent_snapshot.parent else {
                break;
            };
            let Some(grandparent_snapshot) = tree.get(grandparent_id).cloned() else {
                break;
            };
            let grandparent_style = styles
                .get(grandparent_snapshot.node)
                .cloned()
                .unwrap_or_default();
            if !Self::is_simple_block_flow_container(&grandparent_style) {
                break;
            }

            let Some(next_index) = grandparent_snapshot
                .children
                .iter()
                .position(|&id| id == parent_box_id)
            else {
                break;
            };

            parent_box_id = grandparent_id;
            changed_child_index = next_index;
        }
    }

    /// Apply relative positioning offsets.
    ///
    /// For each element with `position: relative`, shift its visual position
    /// by the resolved top/left (or bottom/right) offsets. This does NOT
    /// affect the layout of surrounding elements — only the element's own
    /// coordinates are shifted.
    fn apply_relative_offsets(tree: &mut LayoutTree, styles: &StyleMap, base_font_size: f32) {
        let all_ids: Vec<LayoutBoxId> = (0..tree.boxes.len()).collect();

        for box_id in all_ids {
            let node_id = match tree.get(box_id) {
                Some(b) => b.node,
                None => continue,
            };
            let style = match styles.get(node_id) {
                Some(s) => s.clone(),
                None => continue,
            };
            if style.position != Position::Relative {
                continue;
            }

            let font_size = style.font_size;

            // Use the containing block dimensions for percentage resolution.
            // For relative positioning, percentages resolve against the
            // containing block (approximated here by the parent's content rect).
            let (cb_w, cb_h) = tree
                .get(box_id)
                .and_then(|b| b.parent)
                .and_then(|pid| tree.get(pid))
                .map(|p| (p.content_rect.width, p.content_rect.height))
                .unwrap_or((0.0, 0.0));

            let top = style
                .top
                .resolve_px(cb_h, base_font_size, font_size, cb_w, cb_h);
            let bottom = style
                .bottom
                .resolve_px(cb_h, base_font_size, font_size, cb_w, cb_h);
            let left = style
                .left
                .resolve_px(cb_w, base_font_size, font_size, cb_w, cb_h);
            let right = style
                .right
                .resolve_px(cb_w, base_font_size, font_size, cb_w, cb_h);

            // Compute offsets: top takes precedence over bottom, left over right
            let dy = if let Some(t) = top {
                t
            } else if let Some(b_val) = bottom {
                -b_val
            } else {
                0.0
            };

            let dx = if let Some(l) = left {
                l
            } else if let Some(r) = right {
                -r
            } else {
                0.0
            };

            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                if let Some(b) = tree.get_mut(box_id) {
                    b.content_rect.x += dx;
                    b.content_rect.y += dy;
                    b.padding_rect.x += dx;
                    b.padding_rect.y += dy;
                    b.border_rect.x += dx;
                    b.border_rect.y += dy;
                    b.margin_rect.x += dx;
                    b.margin_rect.y += dy;
                }
            }
        }
    }

    /// Apply scroll-aware sticky positioning.
    ///
    /// For each element with `position: sticky`, we clamp its position so it
    /// stays within the visible scrollport of the nearest scroll ancestor.
    fn apply_sticky_offsets(
        tree: &mut LayoutTree,
        styles: &StyleMap,
        _doc: &Document,
        base_font_size: f32,
    ) {
        // Collect all box IDs first to avoid borrow issues.
        let all_ids: Vec<LayoutBoxId> = (0..tree.boxes.len()).collect();

        for box_id in all_ids {
            let node_id = match tree.get(box_id) {
                Some(b) => b.node,
                None => continue,
            };
            let style = match styles.get(node_id) {
                Some(s) => s.clone(),
                None => continue,
            };
            if style.position != Position::Sticky {
                continue;
            }

            let font_size = style.font_size;

            // Find the nearest scroll-container ancestor in the layout tree.
            let mut scroll_ancestor = tree.get(box_id).and_then(|b| b.parent);
            let mut scroll_offset = (0.0f32, 0.0f32);
            let mut scroll_viewport = (0.0f32, 0.0f32);
            while let Some(ancestor_id) = scroll_ancestor {
                if let Some(ancestor) = tree.get(ancestor_id) {
                    if ancestor.scroll_size.is_some() {
                        scroll_offset = ancestor.scroll_offset;
                        scroll_viewport =
                            (ancestor.content_rect.width, ancestor.content_rect.height);
                        break;
                    }
                    scroll_ancestor = ancestor.parent;
                } else {
                    break;
                }
            }

            // Resolve sticky offsets (top/right/bottom/left)
            let vw = scroll_viewport.0.max(1.0);
            let vh = scroll_viewport.1.max(1.0);
            let top = style.top.resolve_px(vh, base_font_size, font_size, vw, vh);
            let bottom = style
                .bottom
                .resolve_px(vh, base_font_size, font_size, vw, vh);
            let left = style.left.resolve_px(vw, base_font_size, font_size, vw, vh);
            let right = style
                .right
                .resolve_px(vw, base_font_size, font_size, vw, vh);

            // The element's current (normal-flow) position is stored in its border_rect.
            let bx = tree.get(box_id).map(|b| b.border_rect.x).unwrap_or(0.0);
            let by = tree.get(box_id).map(|b| b.border_rect.y).unwrap_or(0.0);
            let bw = tree.get(box_id).map(|b| b.border_rect.width).unwrap_or(0.0);
            let bh = tree
                .get(box_id)
                .map(|b| b.border_rect.height)
                .unwrap_or(0.0);

            // Compute clamped position based on scroll offset.
            // The sticky constraint is: the element must stay within the
            // scrollport bounds offset by the specified edges.
            let mut new_x = bx;
            let mut new_y = by;

            // Vertical sticky clamping
            if let Some(top_val) = top {
                // Element must not go above (scroll_offset.y + top)
                let min_y = scroll_offset.1 + top_val;
                if new_y < min_y {
                    new_y = min_y;
                }
            }
            if let Some(bottom_val) = bottom {
                // Element must not go below (scroll_offset.y + viewport_h - bottom - element_h)
                let max_y = scroll_offset.1 + scroll_viewport.1 - bottom_val - bh;
                if new_y > max_y {
                    new_y = max_y;
                }
            }

            // Horizontal sticky clamping
            if let Some(left_val) = left {
                let min_x = scroll_offset.0 + left_val;
                if new_x < min_x {
                    new_x = min_x;
                }
            }
            if let Some(right_val) = right {
                let max_x = scroll_offset.0 + scroll_viewport.0 - right_val - bw;
                if new_x > max_x {
                    new_x = max_x;
                }
            }

            // Apply offset delta to all rects
            let dx = new_x - bx;
            let dy = new_y - by;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                if let Some(b) = tree.get_mut(box_id) {
                    b.content_rect.x += dx;
                    b.content_rect.y += dy;
                    b.padding_rect.x += dx;
                    b.padding_rect.y += dy;
                    b.border_rect.x += dx;
                    b.border_rect.y += dy;
                    b.margin_rect.x += dx;
                    b.margin_rect.y += dy;
                }
            }
        }
    }

    /// First pass of positioned layout: register all anchor names.
    ///
    /// Walks the DOM and records the border rect of every element that has
    /// `anchor-name` set, so that positioned elements can reference them
    /// via `position-anchor`.
    fn register_anchors(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
    ) {
        let style = styles.get(node_id).cloned().unwrap_or_default();
        if let Some(ref name) = style.anchor_name {
            if !name.is_empty() {
                // Use the absolute border rect so anchors work across nesting.
                if let Some(box_id) = tree.find_box_id_by_node(node_id) {
                    let rect = tree.absolute_border_rect(box_id);
                    tree.anchor_registry.register(name.clone(), rect);
                }
            }
        }
        let children = doc.children(node_id).to_vec();
        for &child_id in &children {
            self.register_anchors(doc, child_id, styles, tree);
        }
    }

    /// Layout positioned elements (absolute/fixed) in a second pass.
    fn layout_positioned_elements<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &TM,
        image_measurer: &IM,
    ) {
        let viewport_rect = Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
        self.layout_positioned_recursive(
            doc,
            node_id,
            styles,
            tree,
            text_measurer,
            image_measurer,
            viewport_rect,
        );
    }

    /// Recursive positioned layout with tracked fixed-position containing block.
    ///
    /// Per CSS Transforms §7.1, an ancestor with transform/perspective/filter/
    /// contain:paint creates a containing block for fixed-position descendants,
    /// overriding the viewport.
    fn layout_positioned_recursive<TM: TextMeasurer + ?Sized, IM: ImageMeasurer + ?Sized>(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &TM,
        image_measurer: &IM,
        fixed_cb: Rect,
    ) {
        let children = doc.children(node_id).to_vec();

        // Find the containing block rect for this node (CSS2.1 §10.1: padding edge)
        let containing_rect = tree
            .find_by_node(node_id)
            .map(|b| b.padding_rect)
            .unwrap_or(Rect::new(
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
            ));

        // Determine the fixed-position containing block for children.
        // If this node establishes one (via transform/filter/etc.), use its
        // padding rect; otherwise inherit from parent.
        let node_style = styles.get(node_id).cloned().unwrap_or_default();
        let child_fixed_cb = if node_style.establishes_fixed_containing_block() {
            containing_rect
        } else {
            fixed_cb
        };

        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();

            if matches!(child_style.position, Position::Absolute | Position::Fixed) {
                // For fixed positioning, use the tracked fixed_cb instead of
                // always using viewport. layout_positioned handles this via
                // the containing_rect parameter — for fixed elements, we pass
                // child_fixed_cb (which is the viewport unless an ancestor
                // with transform/filter/etc. exists).
                let cb = if child_style.position == Position::Fixed {
                    child_fixed_cb
                } else {
                    containing_rect
                };
                if let Some(pos_box) = crate::positioned::layout_positioned(
                    doc,
                    child_id,
                    styles,
                    tree,
                    text_measurer,
                    image_measurer,
                    cb,
                    self.viewport.width,
                    self.viewport.height,
                    self.base_font_size,
                ) {
                    // Add to parent in tree
                    if let Some(parent_box) = tree.find_by_node(node_id).map(|b| b.id) {
                        tree.add_child(parent_box, pos_box);
                    }
                }
            }

            // Recurse
            self.layout_positioned_recursive(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_fixed_cb,
            );
        }
    }

    // ── Cache management ──────────────────────────────────────────────

    /// Clear the layout cache entirely.
    pub fn clear_cache(&mut self) {
        self.cache.invalidate_all();
    }

    /// Invalidate the cached layout result for a specific node.
    pub fn invalidate_node(&mut self, node_id: NodeId) {
        self.cache.invalidate(node_id);
    }

    /// Invalidate a node and all its descendants in the cache.
    pub fn invalidate_subtree(&mut self, node_id: NodeId, doc: &Document) {
        self.cache
            .invalidate_subtree(node_id, |id| doc.children(id).to_vec());
    }

    /// Access the layout cache (read-only).
    pub fn cache(&self) -> &LayoutCache {
        &self.cache
    }

    /// Access the layout cache (mutable).
    pub fn cache_mut(&mut self) -> &mut LayoutCache {
        &mut self.cache
    }

    // ── Dirty tracking ────────────────────────────────────────────────

    /// Access dirty propagation state (read-only).
    pub fn dirty(&self) -> &DirtyPropagation {
        &self.dirty
    }

    /// Access dirty propagation state (mutable).
    pub fn dirty_mut(&mut self) -> &mut DirtyPropagation {
        &mut self.dirty
    }

    /// Mark a node as needing re-layout.
    pub fn mark_dirty(&mut self, node_id: NodeId, flags: LayoutDirtyFlags) {
        self.dirty.mark_dirty(node_id, flags);
    }

    /// Mark a node dirty and propagate `CHILD_NEEDS_LAYOUT` up through ancestors.
    pub fn mark_dirty_and_propagate(
        &mut self,
        doc: &Document,
        node_id: NodeId,
        flags: LayoutDirtyFlags,
    ) {
        self.dirty
            .mark_dirty_and_propagate(node_id, flags, |id| doc.parent(id));
    }

    // ── Debug bypass ──────────────────────────────────────────────────

    /// Whether the layout cache is currently bypassed.
    pub fn bypass_cache(&self) -> bool {
        self.bypass_cache
    }

    /// Enable or disable the cache bypass (skips all cache lookups/stores).
    pub fn set_bypass_cache(&mut self, bypass: bool) {
        self.bypass_cache = bypass;
    }

    // ── Cache population helpers ──────────────────────────────────────

    /// Walk the completed layout tree and store every node's result in the cache.
    ///
    /// Constraints are derived from the parent's content rect (or viewport for root).
    fn populate_cache_from_tree(&mut self, tree: &LayoutTree) {
        let viewport_w = self.viewport.width;
        let viewport_h = self.viewport.height;

        for layout_box in &tree.boxes {
            let (avail_w, avail_h) = if let Some(parent_id) = layout_box.parent {
                tree.get(parent_id)
                    .map(|p| (p.content_rect.width, p.content_rect.height))
                    .unwrap_or((viewport_w, viewport_h))
            } else {
                (viewport_w, viewport_h)
            };

            let constraints = LayoutConstraints::fixed(avail_w, avail_h);
            let result = Self::extract_cached_result(layout_box, tree);
            self.cache.store(layout_box.node, constraints, result);
        }
    }

    /// Extract a [`CachedLayoutResult`] from a completed [`LayoutBox`].
    fn extract_cached_result(layout_box: &LayoutBox, tree: &LayoutTree) -> CachedLayoutResult {
        let br = &layout_box.border_rect;
        let mr = &layout_box.margin_rect;
        let cr = &layout_box.content_rect;

        let margin_top = br.y - mr.y;
        let margin_left = br.x - mr.x;
        let margin_bottom = (mr.y + mr.height) - (br.y + br.height);
        let margin_right = (mr.x + mr.width) - (br.x + br.width);

        let child_offsets: Vec<(f32, f32)> = layout_box
            .children
            .iter()
            .filter_map(|&cid| {
                tree.get(cid)
                    .map(|cb| (cb.margin_rect.x - cr.x, cb.margin_rect.y - cr.y))
            })
            .collect();

        let overflow = layout_box
            .scroll_size
            .map(|s| (s.width, s.height))
            .unwrap_or((cr.width, cr.height));

        CachedLayoutResult {
            size: (br.width, br.height),
            baseline: layout_box.baseline,
            margins: (margin_top, margin_right, margin_bottom, margin_left),
            child_offsets,
            overflow,
            intrinsic_sizes: Default::default(),
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new(Size::new(1920.0, 1080.0), 16.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DefaultImageMeasurer, DefaultTextMeasurer};
    use liquide_dom::Document;
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

    /// Regression (t76-layoutorigin): a full-viewport `position: fixed` element
    /// with `top/left/right/bottom: 0` (the desktop-background / wallpaper box)
    /// must lay out with a border box of EXACTLY the viewport `(0, 0, vw, vh)`.
    ///
    /// This pins the layout half of the wallpaper-left-strip investigation: the
    /// layout box origin is correct here; the x≈50 origin the strip came from
    /// was introduced later, in the painter's background-position handling (see
    /// `liquide-paint` `background_position_does_not_offset_full_bleed_cover`).
    #[test]
    fn fixed_full_viewport_box_is_exactly_the_viewport() {
        let mut doc = Document::new();
        let root = doc.root();
        let bg = doc.create_element("desktop-background");
        doc.append_child(root, bg);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1280.0,
                height: 720.0,
            },
            16.0,
        );
        // Mirror the real desktop-background: fixed, all-zero insets, no width.
        style_engine.add_stylesheet(
            "desktop-background { position: fixed; top: 0; left: 0; right: 0; bottom: 0; }",
        );
        let styles = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1280.0, 720.0), 16.0);
        let tree = layout.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let id = tree.find_box_id_by_node(bg).expect("desktop-background box");
        let r = tree.absolute_border_rect(id);
        assert!(
            r.x.abs() < 0.01 && r.y.abs() < 0.01,
            "fixed inset:0 box must start at (0,0), got ({},{})",
            r.x,
            r.y
        );
        assert!(
            (r.width - 1280.0).abs() < 0.01 && (r.height - 720.0).abs() < 0.01,
            "fixed inset:0 box must span the full viewport (1280x720), got {}x{}",
            r.width,
            r.height
        );
    }

    #[test]
    fn basic_block_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet("div { width: 200px; height: 100px; }");

        let style_map = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let tree = layout.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        assert!(tree.box_count() > 0);
    }

    #[test]
    fn flex_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("dock");
        let item1 = doc.create_element("dock-item");
        let item2 = doc.create_element("dock-item");
        doc.append_child(root, container);
        doc.append_child(container, item1);
        doc.append_child(container, item2);

        let mut style_engine = StyleEngine::default();
        style_engine.add_stylesheet(
            r#"
            dock { display: flex; width: 200px; gap: 8px; }
            dock-item { width: 50px; height: 50px; }
            "#,
        );

        let style_map = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::default();
        let tree = layout.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        // Should have boxes for container + 2 items
        assert!(tree.box_count() >= 3);
    }

    #[test]
    fn relayout_subtree_matches_full_for_simple_block_flow() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("div");
        doc.append_child(root, container);
        let first = doc.create_element("item");
        let second = doc.create_element("item");
        doc.append_child(container, first);
        doc.append_child(container, second);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet(
            r#"
            div { width: 300px; }
            item {
                display: block;
                height: 20px;
                margin: 0;
                padding: 0;
            }
            "#,
        );

        let mut engine = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);

        let styles_before = style_engine.restyle_all(&doc);
        let baseline = engine.layout(
            &doc,
            &styles_before,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let third = doc.create_element("item");
        doc.append_child(container, third);
        let styles_after = style_engine.restyle_all(&doc);
        let input = LayoutInput::new(
            &doc,
            &styles_after,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let incremental = engine.relayout_subtree(&input, container, &baseline);
        let full = engine.layout(
            &doc,
            &styles_after,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let inc_container = incremental
            .find_by_node(container)
            .expect("incremental relayout must keep container mapped");
        let full_container = full
            .find_by_node(container)
            .expect("full relayout must keep container mapped");
        assert!(
            (inc_container.content_rect.height - full_container.content_rect.height).abs() < 0.1
        );

        let inc_third = incremental
            .find_by_node(third)
            .expect("incremental relayout must include appended child");
        let full_third = full
            .find_by_node(third)
            .expect("full relayout must include appended child");
        assert!((inc_third.margin_rect.y - full_third.margin_rect.y).abs() < 0.1);
        assert!((inc_third.margin_rect.height - full_third.margin_rect.height).abs() < 0.1);
    }

    /// Regression for the layout-cache leaf-safety hazard (t49-e3-F2):
    /// a cache hit on a node WITH children must reconstruct the children —
    /// the subtree must NOT vanish.  `relayout_subtree` exercises the
    /// cache-accelerated fast path in `layout_node_in_context`; with the
    /// pre-fix code a clean non-leaf cache hit dropped every child.
    #[test]
    fn cache_hit_on_node_with_children_preserves_subtree() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("div");
        doc.append_child(root, container);
        let first = doc.create_element("item");
        let second = doc.create_element("item");
        doc.append_child(container, first);
        doc.append_child(container, second);
        // An unrelated node we can dirty so dirty tracking is "active"
        // (dirty_count > 0) while the container itself stays clean — this is
        // the exact precondition that makes the cache fast path fire.
        let sibling = doc.create_element("div");
        doc.append_child(root, sibling);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet(
            r#"
            div { width: 300px; }
            item { display: block; height: 20px; margin: 0; padding: 0; }
            "#,
        );

        let styles = style_engine.restyle_all(&doc);
        let mut engine = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);

        // Full layout populates the cache, including a NON-LEAF entry for the
        // container (its stored `child_offsets` is non-empty).
        let baseline = engine.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
        assert_eq!(
            baseline
                .find_by_node(container)
                .expect("container laid out")
                .children
                .len(),
            2,
            "precondition: container has two child boxes in full layout",
        );

        // Activate dirty tracking via an UNRELATED node; leave the container
        // clean so the cache fast path is taken for it.
        engine.mark_dirty(sibling, LayoutDirtyFlags::NEEDS_LAYOUT);
        assert!(engine.dirty().dirty_count() > 0);
        assert!(!engine.dirty().needs_layout(container));
        assert!(!engine.dirty().has_dirty_flags(container));

        let input = LayoutInput::new(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
        let relaid = engine.relayout_subtree(&input, container, &baseline);

        let relaid_container = relaid
            .find_by_node(container)
            .expect("container must remain mapped after relayout");
        assert_eq!(
            relaid_container.children.len(),
            2,
            "cache hit on a node with children must NOT drop the subtree",
        );
        assert!(
            relaid.find_by_node(first).is_some(),
            "first child box must survive the cache fast path",
        );
        assert!(
            relaid.find_by_node(second).is_some(),
            "second child box must survive the cache fast path",
        );
    }

    /// Companion: a true leaf cache hit is still honored (fast path not
    /// over-disabled).  A childless node with a clean cache entry should
    /// reconstruct without re-running layout, and must stay childless.
    #[test]
    fn cache_hit_on_true_leaf_is_still_honored() {
        let mut doc = Document::new();
        let root = doc.root();
        let leaf = doc.create_element("div");
        doc.append_child(root, leaf);
        let sibling = doc.create_element("div");
        doc.append_child(root, sibling);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet("div { width: 120px; height: 40px; }");

        let styles = style_engine.restyle_all(&doc);
        let mut engine = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let baseline = engine.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

        engine.mark_dirty(sibling, LayoutDirtyFlags::NEEDS_LAYOUT);

        let input = LayoutInput::new(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
        let relaid = engine.relayout_subtree(&input, leaf, &baseline);

        let relaid_leaf = relaid
            .find_by_node(leaf)
            .expect("leaf must remain mapped after relayout");
        assert!(
            relaid_leaf.children.is_empty(),
            "a true leaf stays childless",
        );
        assert!((relaid_leaf.border_rect.width - 120.0).abs() < 0.1);
        assert!((relaid_leaf.border_rect.height - 40.0).abs() < 0.1);
    }

    /// Regression (t60 finding #5): `position: absolute; height: 50%` must
    /// resolve against the containing block's HEIGHT, not its width. With a
    /// 400x200 relatively-positioned container, a 50%-tall absolute child is
    /// 100px tall (50% of 200), NOT 200px (50% of 400 width).
    #[test]
    fn absolute_height_percent_resolves_against_cb_height() {
        let mut doc = Document::new();
        let root = doc.root();
        let outer = doc.create_element("outer");
        let inner = doc.create_element("inner");
        doc.append_child(root, outer);
        doc.append_child(outer, inner);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet(
            r#"
            outer { position: relative; width: 400px; height: 200px; }
            inner { position: absolute; top: 0; left: 0; width: 50%; height: 50%; }
            "#,
        );

        let styles = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let tree = layout.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let inner_box = tree.find_box_id_by_node(inner).expect("inner box");
        let r = tree.get(inner_box).unwrap().border_rect;
        // width 50% of 400 = 200; height 50% of 200 = 100 (NOT 200).
        assert!(
            (r.width - 200.0).abs() < 1.0,
            "inner width = {} (expected 200)",
            r.width
        );
        assert!(
            (r.height - 100.0).abs() < 1.0,
            "inner height = {} (expected 100 = 50% of cb HEIGHT, not width)",
            r.height
        );
    }

    /// Regression (t60 finding #8, end-to-end): an absolutely-positioned child
    /// of a *padded* relatively-positioned container must paint at its
    /// containing-block-relative coordinates, NOT shifted by the parent's
    /// padding. The absolute CB is the parent's padding box, so `left: 100`
    /// places the child at absolute x=100 — the parent's `padding-left: 50`
    /// must not be double-counted into the child's absolute rect.
    #[test]
    fn absolute_child_not_shifted_by_parent_padding() {
        let mut doc = Document::new();
        let root = doc.root();
        let outer = doc.create_element("outer");
        let abs = doc.create_element("abs");
        doc.append_child(root, outer);
        doc.append_child(outer, abs);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet(
            r#"
            outer { position: relative; width: 500px; height: 500px; padding-left: 50px; padding-top: 50px; }
            abs { position: absolute; left: 100px; top: 100px; width: 200px; height: 200px; }
            "#,
        );

        let styles = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let tree = layout.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let abs_box = tree.find_box_id_by_node(abs).expect("abs box");
        let r = tree.absolute_border_rect(abs_box);
        // CB = outer's padding box; with no border its left edge is the element
        // origin (0), so left:100 → absolute x=100. NOT 150 (100 + padding 50).
        assert!(
            (r.x - 100.0).abs() < 1.0,
            "abs x = {} (expected 100, parent padding must not double-count)",
            r.x
        );
        assert!(
            (r.y - 100.0).abs() < 1.0,
            "abs y = {} (expected 100)",
            r.y
        );
    }

    /// Regression (t71): a `position: fixed` flex element with NO explicit
    /// `width` (e.g. the right-click context menu — `min-width` only) must
    /// produce EXACTLY ONE layout box in the flat `tree.boxes` list, anchored
    /// at its real position — never a leftover twin box at the local origin.
    ///
    /// Root cause history: `layout_positioned` ran an intrinsic-measurement
    /// `layout_block` pass at local (0,0) to size the element, then allocated a
    /// *second* canonical box and stole the children — but the orphaned
    /// intrinsic box was never removed from `tree.boxes` and stayed mapped to
    /// the same node with a `border_rect` at (0,0). Consumers that iterate the
    /// flat `boxes` Vec (the glass/blur extractor, the layout cache) then saw
    /// two boxes for the node and double-counted it, painting a duplicate
    /// frosted-glass panel at the viewport origin for every fixed/absolute
    /// flex+blur surface. The fix reuses the intrinsic box as the canonical
    /// positioned box, guaranteeing one box per node.
    #[test]
    fn fixed_flex_no_width_has_single_box_at_anchor() {
        let mut doc = Document::new();
        let root = doc.root();
        let menu = doc.create_element("context-menu");
        let item1 = doc.create_element("menu-item");
        let item2 = doc.create_element("menu-item");
        doc.append_child(root, menu);
        doc.append_child(menu, item1);
        doc.append_child(menu, item2);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1280.0,
                height: 720.0,
            },
            16.0,
        );
        // Mirror the real menu: fixed, flex, NO width (only min-width), anchored
        // at the click point, with a blur backdrop. `needs_intrinsic` is true,
        // so the intrinsic-measurement pass runs — the exact trigger condition.
        style_engine.add_stylesheet(
            r#"
            context-menu {
                position: fixed;
                display: flex;
                flex-direction: column;
                left: 640px;
                top: 360px;
                min-width: 180px;
                backdrop-filter: blur(12px);
            }
            menu-item { height: 30px; }
            "#,
        );

        let styles = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1280.0, 720.0), 16.0);
        let tree = layout.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

        // EXACTLY ONE box in the flat list references the menu node.
        let menu_boxes: Vec<&LayoutBox> =
            tree.boxes.iter().filter(|b| b.node == menu).collect();
        assert_eq!(
            menu_boxes.len(),
            1,
            "menu node must have exactly ONE layout box (no leaked intrinsic twin); \
             found {} at rects {:?}",
            menu_boxes.len(),
            menu_boxes
                .iter()
                .map(|b| (b.border_rect.x, b.border_rect.y))
                .collect::<Vec<_>>()
        );

        // The single box is the canonical one and sits at the anchor, not (0,0).
        let canonical = tree.find_box_id_by_node(menu).expect("menu box");
        let r = tree.absolute_border_rect(canonical);
        assert!(
            (r.x - 640.0).abs() < 1.0 && (r.y - 360.0).abs() < 1.0,
            "menu box must be at its anchor (640,360), got ({},{})",
            r.x,
            r.y
        );

        // No box for the menu node anywhere at the origin (the stray twin).
        let origin_twin = tree.boxes.iter().any(|b| {
            b.node == menu && b.border_rect.x.abs() < 0.01 && b.border_rect.y.abs() < 0.01
        });
        assert!(
            !origin_twin,
            "no leftover menu box at local origin (0,0) is allowed"
        );

        // Sizing still works: min-width is honored.
        assert!(
            r.width >= 180.0 - 1.0,
            "menu width must honor min-width:180, got {}",
            r.width
        );
    }
}

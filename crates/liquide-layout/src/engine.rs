//! Layout engine — the main entry point for computing layout.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, Position};

use crate::geometry::{Rect, Size};
use crate::tree::{LayoutBoxId, LayoutTree};
use crate::writing_mode::WritingModeContext;
use crate::{ImageMeasurer, TextMeasurer};

/// The layout engine. Computes geometry for all elements in the document.
pub struct LayoutEngine {
    /// Viewport size.
    pub viewport: Size,
    /// Root font size for `rem` units.
    pub base_font_size: f32,
}

/// Bundled input for layout and relayout APIs.
pub struct LayoutInput<'a> {
    pub doc: &'a Document,
    pub styles: &'a StyleMap,
    pub text_measurer: &'a dyn TextMeasurer,
    pub image_measurer: &'a dyn ImageMeasurer,
}

impl<'a> LayoutInput<'a> {
    pub fn new(
        doc: &'a Document,
        styles: &'a StyleMap,
        text_measurer: &'a dyn TextMeasurer,
        image_measurer: &'a dyn ImageMeasurer,
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
        }
    }

    /// Run layout on the entire document.
    pub fn layout(
        &mut self,
        doc: &Document,
        styles: &StyleMap,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
    ) -> LayoutTree {
        let mut tree = LayoutTree::new();
        let root = doc.root();

        let root_style = styles.get(root).cloned().unwrap_or_default();

        // Read writing-mode and direction from the root element's computed style.
        // These determine the document's inline/block axis mapping.
        let _root_wm = WritingModeContext::with_direction(
            root_style.writing_mode,
            root_style.direction,
        );

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

        // Second pass: layout positioned elements
        self.layout_positioned_elements(
            doc,
            root,
            styles,
            &mut tree,
            text_measurer,
            image_measurer,
        );

        // Third pass: apply relative positioning offsets
        Self::apply_relative_offsets(&mut tree, styles);

        // Fourth pass: adjust sticky-positioned elements based on scroll offsets
        Self::apply_sticky_offsets(&mut tree, styles, doc);

        tree
    }

    /// Run layout using a bundled input object.
    pub fn layout_with_input(&mut self, input: &LayoutInput<'_>) -> LayoutTree {
        self.layout(
            input.doc,
            input.styles,
            input.text_measurer,
            input.image_measurer,
        )
    }

    /// Incremental relayout entrypoint.
    pub fn relayout_subtree(
        &mut self,
        input: &LayoutInput<'_>,
        node_id: NodeId,
        previous_tree: &LayoutTree,
    ) -> LayoutTree {
        if node_id == input.doc.root() || !self.supports_incremental_relayout(input.doc, input.styles, node_id) {
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
        let Some(replace_index) = parent_box.children.iter().position(|&id| id == old_box_id) else {
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
        Self::apply_relative_offsets(&mut relaid_subtree, input.styles);

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
            if replace_index < parent.children.len() && parent.children[replace_index] == old_box_id {
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

        let new_root_id =
            Self::clone_subtree_into(&relaid_subtree, relaid_root, &mut result, Some(parent_box_id));
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

        Self::apply_sticky_offsets(&mut result, input.styles, input.doc);
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

    fn is_simple_block_flow_container(style: &liquide_style_engine::computed::ComputedStyle) -> bool {
        matches!(style.display, Display::Block | Display::FlowRoot | Display::ListItem)
            && !style.is_flex_container()
            && !style.is_grid_container()
            && !style.is_table()
            && !style.is_multicol()
            && !matches!(
                style.position,
                Position::Absolute | Position::Fixed | Position::Sticky
            )
    }

    fn layout_node_in_context(
        &self,
        input: &LayoutInput<'_>,
        node_id: NodeId,
        tree: &mut LayoutTree,
        container_width: f32,
        container_height: f32,
        offset_x: f32,
        offset_y: f32,
    ) -> LayoutBoxId {
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
            return wrapper;
        }

        if style.is_flex_container() {
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
        }
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
        let outward_delta = delta_h;
        while outward_delta.abs() > 0.001 {
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

            let parent_style = styles.get(parent_snapshot.node).cloned().unwrap_or_default();
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
    fn apply_relative_offsets(tree: &mut LayoutTree, styles: &StyleMap) {
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
            let base_font_size = 16.0f32; // TODO: propagate from engine

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
    fn apply_sticky_offsets(tree: &mut LayoutTree, styles: &StyleMap, _doc: &Document) {
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
            let base_font_size = 16.0f32; // TODO: propagate from engine

            // Find the nearest scroll-container ancestor in the layout tree.
            let mut scroll_ancestor = tree.get(box_id).and_then(|b| b.parent);
            let mut scroll_offset = (0.0f32, 0.0f32);
            let mut scroll_viewport = (0.0f32, 0.0f32);
            while let Some(ancestor_id) = scroll_ancestor {
                if let Some(ancestor) = tree.get(ancestor_id) {
                    if ancestor.scroll_size.is_some() {
                        scroll_offset = ancestor.scroll_offset;
                        scroll_viewport = (ancestor.content_rect.width, ancestor.content_rect.height);
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
            let bottom = style.bottom.resolve_px(vh, base_font_size, font_size, vw, vh);
            let left = style.left.resolve_px(vw, base_font_size, font_size, vw, vh);
            let right = style.right.resolve_px(vw, base_font_size, font_size, vw, vh);

            // The element's current (normal-flow) position is stored in its border_rect.
            let bx = tree.get(box_id).map(|b| b.border_rect.x).unwrap_or(0.0);
            let by = tree.get(box_id).map(|b| b.border_rect.y).unwrap_or(0.0);
            let bw = tree.get(box_id).map(|b| b.border_rect.width).unwrap_or(0.0);
            let bh = tree.get(box_id).map(|b| b.border_rect.height).unwrap_or(0.0);

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

    /// Layout positioned elements (absolute/fixed) in a second pass.
    fn layout_positioned_elements(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
    ) {
        let viewport_rect = Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height);
        self.layout_positioned_recursive(
            doc, node_id, styles, tree, text_measurer, image_measurer, viewport_rect,
        );
    }

    /// Recursive positioned layout with tracked fixed-position containing block.
    ///
    /// Per CSS Transforms §7.1, an ancestor with transform/perspective/filter/
    /// contain:paint creates a containing block for fixed-position descendants,
    /// overriding the viewport.
    fn layout_positioned_recursive(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
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
}

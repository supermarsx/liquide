//! Hierarchical window tree with linked-list z-order, hit testing, and
//! parent-child relationships.
//!
//! This crate implements a proper window tree that replaces a flat window list
//! with a real tree structure supporting z-order management, visibility
//! propagation, and efficient hit testing.

mod flags;
mod hit_test;
mod iterators;
mod node;
mod rect;
mod region;
mod tree;

pub use flags::{WindowExStyle, WindowFlags, WindowStyle};
pub use hit_test::{HitArea, HitTestResult, ResizeEdge};
pub use iterators::*;
pub use node::WindowNode;
pub use rect::Rect;
pub use region::Region;
pub use tree::WindowTree;

/// Unique identifier for a window in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    // ===================================================================
    // Window creation & tree structure
    // ===================================================================

    #[test]
    fn create_tree_with_desktop() {
        let tree = WindowTree::new(1920, 1080);
        assert_eq!(tree.len(), 1);
        let desktop = tree.get(tree.desktop_id).unwrap();
        assert_eq!(desktop.bounds, Rect::new(0, 0, 1920, 1080));
        assert_eq!(desktop.title, "Desktop");
    }

    #[test]
    fn create_single_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Test",
        );
        assert_eq!(tree.len(), 2);
        assert!(tree.is_child(w, tree.desktop_id));
        let node = tree.get(w).unwrap();
        assert_eq!(node.title, "Test");
        assert!(node.is_visible());
        assert!(node.is_enabled());
    }

    #[test]
    fn create_child_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let parent = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Parent",
        );
        let child = tree.create_window(
            Some(parent),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(110, 110, 200, 150),
            "Child",
        );
        assert!(tree.is_child(child, parent));
        assert!(tree.is_descendant(child, parent));
        assert!(tree.is_descendant(child, tree.desktop_id));
        assert!(!tree.is_child(child, tree.desktop_id));
    }

    #[test]
    fn create_multiple_children_z_order() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );

        // Most recently created is topmost.
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![c, b, a]);
    }

    #[test]
    fn destroy_window_removes_node() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );
        assert_eq!(tree.len(), 2);
        tree.destroy_window(w);
        assert_eq!(tree.len(), 1);
        assert!(tree.get(w).is_none());
    }

    #[test]
    fn destroy_window_destroys_children() {
        let mut tree = WindowTree::new(1920, 1080);
        let parent = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Parent",
        );
        let child1 = tree.create_window(
            Some(parent),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(110, 110, 200, 150),
            "C1",
        );
        let _grandchild = tree.create_window(
            Some(child1),
            3,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(120, 120, 50, 50),
            "GC",
        );
        let _child2 = tree.create_window(
            Some(parent),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(110, 300, 200, 150),
            "C2",
        );
        assert_eq!(tree.len(), 5);
        tree.destroy_window(parent);
        // Only desktop remains.
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn destroy_middle_sibling() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );

        // Order: C, B, A — destroy B.
        tree.destroy_window(b);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![c, a]);
    }

    #[test]
    fn cannot_destroy_desktop() {
        let mut tree = WindowTree::new(1920, 1080);
        let desktop = tree.desktop_id;
        tree.destroy_window(desktop);
        assert!(tree.get(desktop).is_some());
    }

    // ===================================================================
    // Z-order manipulation
    // ===================================================================

    #[test]
    fn bring_to_top() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A
        tree.bring_to_top(a);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![a, c, b]);
    }

    #[test]
    fn send_to_bottom() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A
        tree.send_to_bottom(c);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![b, a, c]);
    }

    #[test]
    fn insert_after_sibling() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A — move A after C (i.e., between C and B).
        tree.insert_after(a, c);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![c, a, b]);
    }

    #[test]
    fn insert_before_sibling() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A — move A before B.
        tree.insert_before(a, b);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![c, a, b]);
    }

    #[test]
    fn insert_before_first_child() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        // Order: B, A — move A before B (A becomes first).
        tree.insert_before(a, b);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![a, b]);
    }

    // ===================================================================
    // Topmost windows
    // ===================================================================

    #[test]
    fn set_topmost_moves_to_front() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let _b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let _c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A
        tree.set_topmost(a, true);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children[0], a);
        assert!(tree.get(a).unwrap().is_topmost());
    }

    #[test]
    fn topmost_windows_query() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::TOPMOST,
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let _b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::TOPMOST,
            Rect::new(0, 0, 100, 100),
            "C",
        );
        let topmost = tree.topmost_windows();
        assert!(topmost.contains(&a));
        assert!(topmost.contains(&c));
        assert_eq!(topmost.len(), 2);
    }

    #[test]
    fn create_with_ex_topmost() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::TOPMOST,
            Rect::new(0, 0, 100, 100),
            "Topmost",
        );
        assert!(tree.get(w).unwrap().is_topmost());
    }

    // ===================================================================
    // Reparenting
    // ===================================================================

    #[test]
    fn reparent_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let p1 = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 400, 300),
            "Parent1",
        );
        let p2 = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(500, 0, 400, 300),
            "Parent2",
        );
        let child = tree.create_window(
            Some(p1),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(10, 10, 100, 80),
            "Child",
        );
        assert!(tree.is_child(child, p1));

        tree.reparent(child, p2);
        assert!(!tree.is_child(child, p1));
        assert!(tree.is_child(child, p2));
    }

    #[test]
    fn reparent_prevents_cycle() {
        let mut tree = WindowTree::new(1920, 1080);
        let parent = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 400, 300),
            "Parent",
        );
        let child = tree.create_window(
            Some(parent),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(10, 10, 100, 80),
            "Child",
        );
        // Try to make parent a child of its own child — should be rejected.
        tree.reparent(parent, child);
        // Parent should still be a top-level window.
        assert!(tree.is_child(parent, tree.desktop_id));
    }

    // ===================================================================
    // Traversal iterators
    // ===================================================================

    #[test]
    fn children_back_iterator() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Front-to-back: C, B, A
        // Back-to-front: A, B, C
        let back: Vec<_> = tree.children_back(tree.desktop_id).collect();
        assert_eq!(back, vec![a, b, c]);
    }

    #[test]
    fn ancestor_iterator() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 400, 300),
            "W",
        );
        let child = tree.create_window(
            Some(w),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(10, 10, 100, 80),
            "Child",
        );
        let grandchild = tree.create_window(
            Some(child),
            3,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(20, 20, 50, 50),
            "GC",
        );

        let ancestors: Vec<_> = tree.ancestors(grandchild).collect();
        assert_eq!(ancestors, vec![child, w, tree.desktop_id]);
    }

    #[test]
    fn dfs_traversal() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 400, 300),
            "A",
        );
        let a1 = tree.create_window(
            Some(a),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(10, 10, 100, 80),
            "A1",
        );
        let a2 = tree.create_window(
            Some(a),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(10, 100, 100, 80),
            "A2",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(500, 0, 400, 300),
            "B",
        );

        // DFS from desktop: desktop, b (topmost), a, a2 (topmost child), a1
        let dfs: Vec<_> = tree.descendants_dfs(tree.desktop_id).collect();
        assert_eq!(dfs.len(), 5);
        assert_eq!(dfs[0], tree.desktop_id);
        // B is topmost of desktop's children (created last).
        assert_eq!(dfs[1], b);
        assert_eq!(dfs[2], a);
        // A2 is topmost child of A (created last).
        assert_eq!(dfs[3], a2);
        assert_eq!(dfs[4], a1);
    }

    #[test]
    fn sibling_iterator() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );

        let sibs_of_b: Vec<_> = tree.siblings(b).collect();
        assert_eq!(sibs_of_b, vec![c, a]);
    }

    // ===================================================================
    // Visible windows
    // ===================================================================

    #[test]
    fn visible_windows_query() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        // Hide A.
        tree.get_mut(a).unwrap().flags.remove(WindowFlags::VISIBLE);
        let visible = tree.visible_windows();
        assert_eq!(visible, vec![b]);
    }

    // ===================================================================
    // Hit testing
    // ===================================================================

    #[test]
    fn hit_test_client_area() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        let result = tree.hit_test((400, 400)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::Client);
    }

    #[test]
    fn hit_test_caption() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Caption is at top of window, below the border (4px) and above client.
        // Y = 100 + 10 = 110, which is in the caption region (border=4..caption=30+4=34).
        let result = tree.hit_test((400, 110)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::Caption);
    }

    #[test]
    fn hit_test_resize_border() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Left edge: x=100, y in the middle.
        let result = tree.hit_test((101, 400)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::Border(ResizeEdge::Left));
    }

    #[test]
    fn hit_test_corner_resize() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Bottom-right corner.
        let result = tree.hit_test((899, 699)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::Border(ResizeEdge::BottomRight));
    }

    #[test]
    fn hit_test_close_button() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Close button is at the right side of the caption.
        // Window right edge = 900, button width = 46, so button starts at x=854 (local 754).
        // Caption height region: y in [100+4, 100+34).
        let result = tree.hit_test((880, 115)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::CloseButton);
    }

    #[test]
    fn hit_test_child_over_parent() {
        let mut tree = WindowTree::new(1920, 1080);
        let parent = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Parent",
        );
        let child = tree.create_window(
            Some(parent),
            2,
            WindowStyle::CHILD | WindowStyle::BORDER,
            WindowExStyle::empty(),
            Rect::new(200, 200, 200, 150),
            "Child",
        );
        // Point inside child — should hit child, not parent.
        let result = tree.hit_test((300, 280)).unwrap();
        assert_eq!(result.window_id, child);
    }

    #[test]
    fn hit_test_z_order_topmost_wins() {
        let mut tree = WindowTree::new(1920, 1080);
        let _a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(200, 200, 400, 300),
            "B",
        );
        // B is on top (created after A). Point (300, 300) is in both.
        let result = tree.hit_test((300, 300)).unwrap();
        assert_eq!(result.window_id, b);
    }

    #[test]
    fn hit_test_invisible_window_skipped() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "B",
        );
        // Hide B (topmost).
        tree.get_mut(b).unwrap().flags.remove(WindowFlags::VISIBLE);
        let result = tree.hit_test((200, 200)).unwrap();
        assert_eq!(result.window_id, a);
    }

    #[test]
    fn hit_test_transparent_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let _a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::TRANSPARENT,
            Rect::new(100, 100, 400, 300),
            "B",
        );
        // B is transparent — hit test should return B with Transparent area,
        // but the real "interactable" is A (this is up to the caller to handle).
        let result = tree.hit_test((200, 200)).unwrap();
        assert_eq!(result.window_id, b);
        assert_eq!(result.hit_area, HitArea::Transparent);
    }

    #[test]
    fn hit_test_miss() {
        let mut tree = WindowTree::new(1920, 1080);
        let _w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 200, 200),
            "Win",
        );
        // Point outside all windows.
        let result = tree.hit_test((50, 50));
        assert!(result.is_none());
    }

    #[test]
    fn hit_test_with_clip_region() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Clipped",
        );
        // Set a clip region that only covers the left half.
        tree.get_mut(w).unwrap().clip_region = Some(Region::Rect(Rect::new(100, 100, 200, 300)));

        // Point in left half — hit.
        assert!(tree.hit_test((200, 250)).is_some());
        // Point in right half — miss (clipped out).
        assert!(tree.hit_test((400, 250)).is_none());
    }

    #[test]
    fn window_at_point_simplified() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );
        assert_eq!(tree.window_at_point((200, 200)), Some(w));
        assert_eq!(tree.window_at_point((50, 50)), None);
    }

    // ===================================================================
    // Region invalidation
    // ===================================================================

    #[test]
    fn invalidate_and_validate() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );

        assert!(tree.update_region(w).is_none());
        assert!(
            !tree
                .get(w)
                .unwrap()
                .flags
                .contains(WindowFlags::UPDATE_DIRTY)
        );

        tree.invalidate(w, Some(Rect::new(150, 150, 50, 50)));
        assert!(
            tree.get(w)
                .unwrap()
                .flags
                .contains(WindowFlags::UPDATE_DIRTY)
        );
        assert_eq!(tree.update_region(w), Some(Rect::new(150, 150, 50, 50)));

        // Validate the entire region.
        tree.validate(w, None);
        assert!(tree.update_region(w).is_none());
        assert!(
            !tree
                .get(w)
                .unwrap()
                .flags
                .contains(WindowFlags::UPDATE_DIRTY)
        );
    }

    #[test]
    fn invalidate_whole_client() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );
        tree.invalidate(w, None);
        let rgn = tree.update_region(w).unwrap();
        // Should be the client rect.
        assert_eq!(rgn, tree.get(w).unwrap().client_rect);
    }

    #[test]
    fn invalidate_accumulates() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );
        tree.invalidate(w, Some(Rect::new(110, 110, 20, 20)));
        tree.invalidate(w, Some(Rect::new(200, 200, 30, 30)));
        let rgn = tree.update_region(w).unwrap();
        // Union of both rects.
        assert_eq!(rgn, Rect::new(110, 110, 120, 120));
    }

    // ===================================================================
    // Node properties
    // ===================================================================

    #[test]
    fn client_rect_computed_for_captioned_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 400, 300),
            "Win",
        );
        let node = tree.get(w).unwrap();
        // Border = 1, Caption = 30 => client starts at (101, 131), size = (398, 268).
        assert_eq!(node.client_rect, Rect::new(101, 131, 398, 268));
    }

    #[test]
    fn client_rect_no_caption() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::CHILD | WindowStyle::BORDER,
            WindowExStyle::empty(),
            Rect::new(50, 50, 200, 150),
            "Borderless",
        );
        let node = tree.get(w).unwrap();
        // Border = 1, no caption => client starts at (51, 51), size = (198, 148).
        assert_eq!(node.client_rect, Rect::new(51, 51, 198, 148));
    }

    #[test]
    fn window_flags_and_styles() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::POPUP,
            WindowExStyle::TOOL_WINDOW | WindowExStyle::LAYERED,
            Rect::new(100, 100, 200, 200),
            "Popup",
        );
        let node = tree.get(w).unwrap();
        assert!(node.is_popup());
        assert!(!node.is_child());
        assert!(node.ex_style.contains(WindowExStyle::TOOL_WINDOW));
        assert!(node.ex_style.contains(WindowExStyle::LAYERED));
    }

    #[test]
    fn owner_window() {
        let mut tree = WindowTree::new(1920, 1080);
        let main_win = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Main",
        );
        let popup = tree.create_window(
            None,
            1,
            WindowStyle::POPUP,
            WindowExStyle::empty(),
            Rect::new(200, 200, 300, 200),
            "Popup",
        );
        tree.get_mut(popup).unwrap().owner = Some(main_win);
        assert_eq!(tree.get(popup).unwrap().owner, Some(main_win));
    }

    // ===================================================================
    // Edge cases
    // ===================================================================

    #[test]
    fn bring_to_top_already_on_top() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        // B is already on top.
        tree.bring_to_top(b);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![b, a]);
    }

    #[test]
    fn insert_after_last_sibling() {
        let mut tree = WindowTree::new(1920, 1080);
        let a = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "A",
        );
        let b = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "B",
        );
        let c = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "C",
        );
        // Order: C, B, A — insert C after A (move C to bottom).
        tree.insert_after(c, a);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert_eq!(children, vec![b, a, c]);
    }

    #[test]
    fn deeply_nested_hit_test() {
        let mut tree = WindowTree::new(1920, 1080);
        let l1 = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 1000, 800),
            "L1",
        );
        let l2 = tree.create_window(
            Some(l1),
            2,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(50, 50, 900, 700),
            "L2",
        );
        let l3 = tree.create_window(
            Some(l2),
            3,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "L3",
        );
        let l4 = tree.create_window(
            Some(l3),
            4,
            WindowStyle::CHILD,
            WindowExStyle::empty(),
            Rect::new(150, 150, 700, 500),
            "L4",
        );
        // Point inside L4 — deepest wins.
        let result = tree.hit_test((400, 400)).unwrap();
        assert_eq!(result.window_id, l4);
    }

    #[test]
    fn destroy_and_rebuild() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 100, 100),
            "W",
        );
        tree.destroy_window(w);
        let w2 = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(0, 0, 200, 200),
            "W2",
        );
        assert!(tree.get(w2).is_some());
        assert!(tree.get(w).is_none());
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn hit_test_sys_menu() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Sys menu icon area: left side of caption (x < caption_height=30 from left).
        // Window at (100,100), border=4, so caption y: 104..134.
        // Sys menu: lx < 30 => x < 130.
        let result = tree.hit_test((110, 115)).unwrap();
        assert_eq!(result.window_id, w);
        assert_eq!(result.hit_area, HitArea::SysMenu);
    }

    #[test]
    fn hit_test_min_max_buttons() {
        let mut tree = WindowTree::new(1920, 1080);
        let w = tree.create_window(
            None,
            1,
            WindowStyle::OVERLAPPED_WINDOW,
            WindowExStyle::empty(),
            Rect::new(100, 100, 800, 600),
            "Win",
        );
        // Button width = 46, from right edge (900):
        // Close: 854..900 (lx: 754..800)
        // Max:   808..854 (lx: 708..754)
        // Min:   762..808 (lx: 662..708)
        let max_result = tree.hit_test((830, 115)).unwrap();
        assert_eq!(max_result.window_id, w);
        assert_eq!(max_result.hit_area, HitArea::MaxButton);

        let min_result = tree.hit_test((780, 115)).unwrap();
        assert_eq!(min_result.window_id, w);
        assert_eq!(min_result.hit_area, HitArea::MinButton);
    }

    #[test]
    fn desktop_starts_with_no_children() {
        let tree = WindowTree::new(1920, 1080);
        let children: Vec<_> = tree.children(tree.desktop_id).collect();
        assert!(children.is_empty());
    }
}

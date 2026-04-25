use criterion::{Criterion, criterion_group, criterion_main};

use liquide_a11y::focus::FocusManager;
use liquide_a11y::node::{AccessibleNode, Role};
use liquide_a11y::tree::AccessibilityTree;

fn bench_tree_walk_10000_nodes(c: &mut Criterion) {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(0, Role::Window, "Root"));
    for i in 1..10_000u64 {
        let parent = i / 10;
        tree.add_node(parent, AccessibleNode::new(i, Role::Button, "Node"))
            .unwrap();
    }

    c.bench_function("tree_walk_10000_nodes", |b| {
        b.iter(|| {
            let mut count = 0u64;
            tree.walk(|_| count += 1);
            count
        });
    });
}

fn bench_focus_cycle_1000_tabs(c: &mut Criterion) {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(0, Role::Window, "Root"));
    for i in 1..=1000u64 {
        tree.add_node(0, AccessibleNode::new(i, Role::Button, "Btn"))
            .unwrap();
    }
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);

    c.bench_function("focus_cycle_1000_tabs", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = fm.focus_next();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_tree_walk_10000_nodes,
    bench_focus_cycle_1000_tabs
);
criterion_main!(benches);

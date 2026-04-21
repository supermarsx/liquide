use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use liquide_dom::Document;
use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, LayoutInput, Size};
use liquide_layout_cache::LayoutDirtyFlags;
use liquide_style_engine::computed::{ComputedStyle, Display};
use liquide_style_engine::StyleMap;

/// Build a DOM tree of `n` nodes with mixed block/flex/grid containers.
///
/// The tree has the following shape:
///   root
///     ├─ container-0  (flex)
///     │    ├─ child ...
///     │    └─ child ...
///     ├─ container-1  (grid)
///     │    ├─ child ...
///     │    └─ child ...
///     ├─ container-2  (block)
///     │    ├─ child ...
///     │    └─ child ...
///     └─ ...
///
/// Returns the document, a style map, and the `NodeId` of the last leaf.
fn build_tree(n: usize) -> (Document, StyleMap, u64) {
    let mut doc = Document::new();
    let mut styles = StyleMap::new();
    let root = doc.root();

    // Root gets a default block style.
    styles.insert(root, ComputedStyle::default());

    // Distribute nodes across containers of varying display types.
    let container_displays = [Display::Flex, Display::Grid, Display::Block];
    let children_per_container = 10usize;
    // Number of containers needed (at least 1).
    let containers = ((n.saturating_sub(1)) / (children_per_container + 1)).max(1);

    let mut total_created = 0usize;
    let mut last_leaf = root;

    for i in 0..containers {
        if total_created >= n {
            break;
        }
        let container = doc.create_element("div");
        doc.append_child(root, container);

        let mut cs = ComputedStyle::default();
        cs.display = container_displays[i % container_displays.len()];
        styles.insert(container, cs);
        total_created += 1;

        for _j in 0..children_per_container {
            if total_created >= n {
                break;
            }
            let child = doc.create_element("div");
            doc.append_child(container, child);

            let mut child_style = ComputedStyle::default();
            child_style.display = Display::Block;
            styles.insert(child, child_style);
            last_leaf = child;
            total_created += 1;
        }
    }

    (doc, styles, last_leaf)
}

/// Full layout from scratch — cache is bypassed so every node is computed.
fn bench_layout_full(c: &mut Criterion) {
    let text = DefaultTextMeasurer;
    let img = DefaultImageMeasurer;
    let viewport = Size {
        width: 1920.0,
        height: 1080.0,
    };

    let mut group = c.benchmark_group("layout_full");

    for &node_count in &[100, 500, 2000] {
        let (doc, styles, _last_leaf) = build_tree(node_count);

        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    let mut engine = LayoutEngine::new(viewport, 16.0);
                    engine.set_bypass_cache(true);
                    let _tree = engine.layout(&doc, &styles, &text, &img);
                });
            },
        );
    }

    group.finish();
}

/// Incremental layout — modify one leaf node and relayout using the cache.
fn bench_layout_incremental(c: &mut Criterion) {
    let text = DefaultTextMeasurer;
    let img = DefaultImageMeasurer;
    let viewport = Size {
        width: 1920.0,
        height: 1080.0,
    };

    let mut group = c.benchmark_group("layout_incremental");

    for &node_count in &[100, 500, 2000] {
        let (doc, styles, last_leaf) = build_tree(node_count);

        // Perform an initial full layout to populate the cache.
        let mut engine = LayoutEngine::new(viewport, 16.0);
        let initial_tree = engine.layout(&doc, &styles, &text, &img);

        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    // Mark only the last leaf dirty and relayout.
                    engine.mark_dirty_and_propagate(
                        &doc,
                        last_leaf,
                        LayoutDirtyFlags::NEEDS_LAYOUT,
                    );
                    let input = LayoutInput::new(&doc, &styles, &text, &img);
                    let _tree =
                        engine.relayout_subtree(&input, last_leaf, &initial_tree);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_layout_full, bench_layout_incremental);
criterion_main!(benches);

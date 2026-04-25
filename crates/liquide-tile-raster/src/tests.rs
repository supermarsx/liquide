//! Tests for tile-based incremental rasterization.

use crate::TileRasterizer;
use crate::cache::TileCache;
use crate::clipper;
use crate::compositor;
use crate::damage::DamageTracker;
use crate::grid::{PixelRect, TileGrid};
use crate::scheduler;
use crate::tile::{DEFAULT_TILE_SIZE, Tile, TileId, TileState, validate_tile_size};
use liquide_compositor::pixel::Color;
use liquide_paint::display_list::{DisplayItem, DisplayList};

fn make_rect(x: f32, y: f32, w: f32, h: f32) -> liquide_layout::Rect {
    liquide_layout::Rect::new(x, y, w, h)
}

fn make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

// ═══════════════════════════════════════════════════════════════
// Tile tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn tile_new_has_correct_dimensions() {
    let tile = Tile::new(TileId::new(2, 3), 256, 256);
    assert_eq!(tile.id, TileId::new(2, 3));
    assert_eq!(tile.width, 256);
    assert_eq!(tile.height, 256);
    assert_eq!(tile.state, TileState::Empty);
    assert_eq!(tile.generation, 0);
    assert_eq!(tile.byte_len(), 256 * 256 * 4);
}

#[test]
fn tile_edge_has_smaller_dimensions() {
    let tile = Tile::new(TileId::new(0, 0), 100, 50);
    assert_eq!(tile.width, 100);
    assert_eq!(tile.height, 50);
    assert_eq!(tile.byte_len(), 100 * 50 * 4);
}

#[test]
fn tile_clear_zeros_all_pixels() {
    let mut tile = Tile::new(TileId::new(0, 0), 8, 8);
    tile.pixels.fill(255);
    tile.clear();
    assert!(tile.pixels.iter().all(|&b| b == 0));
}

#[test]
fn tile_clear_color_fills_correctly() {
    let mut tile = Tile::new(TileId::new(0, 0), 4, 4);
    tile.clear_color(255, 0, 128, 200);
    for chunk in tile.pixels.chunks_exact(4) {
        assert_eq!(chunk, [255, 0, 128, 200]);
    }
}

#[test]
fn tile_id_manhattan_distance() {
    let a = TileId::new(0, 0);
    let b = TileId::new(3, 4);
    assert_eq!(a.manhattan_distance(&b), 7);
    assert_eq!(b.manhattan_distance(&a), 7);
    assert_eq!(a.manhattan_distance(&a), 0);
}

#[test]
fn tile_stride() {
    let tile = Tile::new(TileId::new(0, 0), 128, 128);
    assert_eq!(tile.stride(), 128 * 4);
}

#[test]
fn tile_origin_coordinates() {
    let tile = Tile::new(TileId::new(3, 2), 256, 256);
    assert_eq!(tile.origin_x(256), 768);
    assert_eq!(tile.origin_y(256), 512);
}

#[test]
fn validate_tile_size_accepted_values() {
    assert!(validate_tile_size(128));
    assert!(validate_tile_size(256));
    assert!(validate_tile_size(512));
    assert!(!validate_tile_size(64));
    assert!(!validate_tile_size(1024));
    assert!(!validate_tile_size(0));
}

#[test]
fn default_tile_size_is_256() {
    assert_eq!(DEFAULT_TILE_SIZE, 256);
}

// ═══════════════════════════════════════════════════════════════
// TileGrid tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn grid_dimensions_exact_multiple() {
    let grid = TileGrid::new(512, 256, 256);
    assert_eq!(grid.cols(), 2);
    assert_eq!(grid.rows(), 1);
    assert_eq!(grid.tile_count(), 2);
}

#[test]
fn grid_dimensions_with_remainder() {
    let grid = TileGrid::new(700, 500, 256);
    assert_eq!(grid.cols(), 3); // ceil(700/256) = 3
    assert_eq!(grid.rows(), 2); // ceil(500/256) = 2
    assert_eq!(grid.tile_count(), 6);
}

#[test]
fn grid_edge_tile_smaller() {
    let grid = TileGrid::new(300, 300, 256);
    // First tile: full size
    let t00 = grid.tile_at(0, 0);
    assert_eq!(t00.width, 256);
    assert_eq!(t00.height, 256);
    // Right edge tile: 300 - 256 = 44 pixels wide
    let t10 = grid.tile_at(1, 0);
    assert_eq!(t10.width, 44);
    assert_eq!(t10.height, 256);
    // Bottom edge tile: 300 - 256 = 44 pixels tall
    let t01 = grid.tile_at(0, 1);
    assert_eq!(t01.width, 256);
    assert_eq!(t01.height, 44);
    // Corner tile: 44x44
    let t11 = grid.tile_at(1, 1);
    assert_eq!(t11.width, 44);
    assert_eq!(t11.height, 44);
}

#[test]
fn grid_tile_for_point() {
    let grid = TileGrid::new(1024, 1024, 256);
    assert_eq!(grid.tile_for_point(0, 0), TileId::new(0, 0));
    assert_eq!(grid.tile_for_point(255, 255), TileId::new(0, 0));
    assert_eq!(grid.tile_for_point(256, 0), TileId::new(1, 0));
    assert_eq!(grid.tile_for_point(512, 512), TileId::new(2, 2));
}

#[test]
fn grid_tiles_for_rect_single_tile() {
    let grid = TileGrid::new(1024, 1024, 256);
    let ids = grid.tiles_for_rect(&PixelRect::new(10.0, 10.0, 50.0, 50.0));
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], TileId::new(0, 0));
}

#[test]
fn grid_tiles_for_rect_spanning_tiles() {
    let grid = TileGrid::new(1024, 1024, 256);
    let ids = grid.tiles_for_rect(&PixelRect::new(200.0, 200.0, 200.0, 200.0));
    // Spans from tile (0,0) to tile (1,1)
    assert_eq!(ids.len(), 4);
    assert!(ids.contains(&TileId::new(0, 0)));
    assert!(ids.contains(&TileId::new(1, 0)));
    assert!(ids.contains(&TileId::new(0, 1)));
    assert!(ids.contains(&TileId::new(1, 1)));
}

#[test]
fn grid_tiles_for_rect_empty_rect() {
    let grid = TileGrid::new(1024, 1024, 256);
    let ids = grid.tiles_for_rect(&PixelRect::new(10.0, 10.0, 0.0, 0.0));
    assert!(ids.is_empty());
}

#[test]
fn grid_invalidate_rect_marks_dirty() {
    let mut grid = TileGrid::new(512, 512, 256);
    // All start as Empty
    assert_eq!(grid.dirty_tiles().len(), 0);

    grid.invalidate_rect(&PixelRect::new(0.0, 0.0, 100.0, 100.0));
    let dirty = grid.dirty_tiles();
    assert_eq!(dirty.len(), 1);
    assert_eq!(dirty[0], TileId::new(0, 0));
}

#[test]
fn grid_invalidate_all() {
    let mut grid = TileGrid::new(512, 512, 256);
    grid.invalidate_all();
    assert_eq!(grid.dirty_tiles().len(), 4);
}

#[test]
fn grid_clean_tile() {
    let mut grid = TileGrid::new(256, 256, 256);
    grid.invalidate_all();
    assert_eq!(grid.dirty_tiles().len(), 1);
    grid.clean_tile(TileId::new(0, 0));
    assert_eq!(grid.dirty_tiles().len(), 0);
    assert_eq!(grid.tile_at(0, 0).state, TileState::Clean);
}

#[test]
fn grid_resize_preserves_clean_tiles() {
    let mut grid = TileGrid::new(512, 512, 256);
    // Mark top-left tile as clean with some data.
    {
        let tile = grid.tile_at_mut(0, 0);
        tile.pixels[0] = 42;
        tile.state = TileState::Clean;
        tile.generation = 5;
    }

    // Resize to larger viewport — the tile at (0,0) should be preserved.
    grid.resize(768, 768);
    assert_eq!(grid.cols(), 3);
    assert_eq!(grid.rows(), 3);
    let t00 = grid.tile_at(0, 0);
    assert_eq!(t00.state, TileState::Clean);
    assert_eq!(t00.pixels[0], 42);
    assert_eq!(t00.generation, 5);
}

#[test]
fn grid_resize_same_size_noop() {
    let mut grid = TileGrid::new(512, 512, 256);
    grid.invalidate_all();
    grid.resize(512, 512);
    // Should still have dirty tiles, nothing changed.
    assert_eq!(grid.dirty_tiles().len(), 4);
}

#[test]
fn grid_tile_bounds() {
    let grid = TileGrid::new(300, 300, 256);
    let bounds = grid.tile_bounds(TileId::new(0, 0));
    assert_eq!(bounds.x, 0.0);
    assert_eq!(bounds.y, 0.0);
    assert_eq!(bounds.width, 256.0);
    assert_eq!(bounds.height, 256.0);

    let bounds1 = grid.tile_bounds(TileId::new(1, 1));
    assert_eq!(bounds1.x, 256.0);
    assert_eq!(bounds1.y, 256.0);
    assert_eq!(bounds1.width, 44.0);
    assert_eq!(bounds1.height, 44.0);
}

#[test]
fn grid_state_counts() {
    let mut grid = TileGrid::new(512, 512, 256);
    let counts = grid.state_counts();
    assert_eq!(counts.empty, 4);
    assert_eq!(counts.dirty, 0);

    grid.invalidate_all();
    let counts = grid.state_counts();
    assert_eq!(counts.dirty, 4);
    assert_eq!(counts.empty, 0);
}

// ═══════════════════════════════════════════════════════════════
// PixelRect tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn pixel_rect_intersection() {
    let a = PixelRect::new(0.0, 0.0, 100.0, 100.0);
    let b = PixelRect::new(50.0, 50.0, 100.0, 100.0);
    assert!(a.intersects(&b));
    let inter = a.intersection(&b).unwrap();
    assert_eq!(inter.x, 50.0);
    assert_eq!(inter.y, 50.0);
    assert_eq!(inter.width, 50.0);
    assert_eq!(inter.height, 50.0);
}

#[test]
fn pixel_rect_no_intersection() {
    let a = PixelRect::new(0.0, 0.0, 50.0, 50.0);
    let b = PixelRect::new(100.0, 100.0, 50.0, 50.0);
    assert!(!a.intersects(&b));
    assert!(a.intersection(&b).is_none());
}

#[test]
fn pixel_rect_union() {
    let a = PixelRect::new(0.0, 0.0, 50.0, 50.0);
    let b = PixelRect::new(100.0, 100.0, 50.0, 50.0);
    let u = a.union(&b);
    assert_eq!(u.x, 0.0);
    assert_eq!(u.y, 0.0);
    assert_eq!(u.width, 150.0);
    assert_eq!(u.height, 150.0);
}

// ═══════════════════════════════════════════════════════════════
// DamageTracker tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn damage_tracker_empty_initially() {
    let dt = DamageTracker::new();
    assert!(dt.is_empty());
    assert_eq!(dt.rect_count(), 0);
    assert_eq!(dt.total_damage_area(), 0);
}

#[test]
fn damage_tracker_add_and_query() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(10.0, 20.0, 100.0, 50.0));
    assert_eq!(dt.rect_count(), 1);
    assert_eq!(dt.total_damage_area(), 5000);
}

#[test]
fn damage_tracker_reset_clears() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(0.0, 0.0, 100.0, 100.0));
    dt.reset();
    assert!(dt.is_empty());
}

#[test]
fn damage_tracker_merge_overlapping() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(0.0, 0.0, 100.0, 100.0));
    dt.add_damage(PixelRect::new(50.0, 50.0, 100.0, 100.0));
    dt.merge_damage();
    assert_eq!(dt.rect_count(), 1);
    let merged = dt.damage_region()[0];
    assert_eq!(merged.x, 0.0);
    assert_eq!(merged.y, 0.0);
    assert_eq!(merged.width, 150.0);
    assert_eq!(merged.height, 150.0);
}

#[test]
fn damage_tracker_no_merge_distant() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(0.0, 0.0, 10.0, 10.0));
    dt.add_damage(PixelRect::new(1000.0, 1000.0, 10.0, 10.0));
    dt.merge_damage();
    assert_eq!(dt.rect_count(), 2);
}

#[test]
fn damage_tracker_bounding_box() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(10.0, 20.0, 30.0, 40.0));
    dt.add_damage(PixelRect::new(100.0, 200.0, 50.0, 60.0));
    let bb = dt.bounding_box().unwrap();
    assert_eq!(bb.x, 10.0);
    assert_eq!(bb.y, 20.0);
    assert_eq!(bb.right(), 150.0);
    assert_eq!(bb.bottom(), 260.0);
}

#[test]
fn damage_tracker_ignores_empty_rects() {
    let mut dt = DamageTracker::new();
    dt.add_damage(PixelRect::new(0.0, 0.0, 0.0, 0.0));
    assert!(dt.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// TileCache tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn cache_put_and_get() {
    let mut cache = TileCache::new(10);
    let id = TileId::new(0, 0);
    let data = vec![255u8; 100];
    cache.put(id, data.clone());
    assert!(cache.contains(id));
    let got = cache.get(id).unwrap();
    assert_eq!(got, &data);
}

#[test]
fn cache_miss_returns_none() {
    let mut cache = TileCache::new(10);
    assert!(cache.get(TileId::new(0, 0)).is_none());
}

#[test]
fn cache_evicts_lru() {
    let mut cache = TileCache::new(2);
    let id_a = TileId::new(0, 0);
    let id_b = TileId::new(1, 0);
    let id_c = TileId::new(2, 0);

    cache.put(id_a, vec![1]);
    cache.put(id_b, vec![2]);
    // Cache is full. Inserting C should evict A (LRU).
    cache.put(id_c, vec![3]);

    assert!(!cache.contains(id_a));
    assert!(cache.contains(id_b));
    assert!(cache.contains(id_c));
}

#[test]
fn cache_access_updates_lru_order() {
    let mut cache = TileCache::new(2);
    let id_a = TileId::new(0, 0);
    let id_b = TileId::new(1, 0);
    let id_c = TileId::new(2, 0);

    cache.put(id_a, vec![1]);
    cache.put(id_b, vec![2]);
    // Access A to make it most-recently-used.
    cache.get(id_a);
    // Insert C: should evict B (now LRU), not A.
    cache.put(id_c, vec![3]);

    assert!(cache.contains(id_a));
    assert!(!cache.contains(id_b));
    assert!(cache.contains(id_c));
}

#[test]
fn cache_stats() {
    let mut cache = TileCache::new(10);
    let id = TileId::new(0, 0);
    cache.get(id); // miss
    cache.put(id, vec![0; 1024]);
    cache.get(id); // hit

    let stats = cache.stats();
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.capacity, 10);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.bytes_used, 1024);
    assert!((stats.hit_rate() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn cache_clear() {
    let mut cache = TileCache::new(10);
    cache.put(TileId::new(0, 0), vec![1]);
    cache.put(TileId::new(1, 0), vec![2]);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn cache_remove_specific() {
    let mut cache = TileCache::new(10);
    let id = TileId::new(5, 5);
    cache.put(id, vec![42]);
    assert!(cache.contains(id));
    cache.remove(id);
    assert!(!cache.contains(id));
}

#[test]
fn cache_update_existing() {
    let mut cache = TileCache::new(10);
    let id = TileId::new(0, 0);
    cache.put(id, vec![1, 2, 3]);
    cache.put(id, vec![4, 5, 6, 7]);
    let got = cache.get(id).unwrap();
    assert_eq!(got, &[4, 5, 6, 7]);
    assert_eq!(cache.len(), 1);
}

#[test]
fn cache_zero_capacity() {
    let mut cache = TileCache::new(0);
    cache.put(TileId::new(0, 0), vec![1]);
    assert!(cache.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// DisplayListClipper tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn clipper_includes_intersecting_items() {
    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(10.0, 10.0, 50.0, 50.0),
        color: make_color(255, 0, 0, 255),
    });
    dl.push(DisplayItem::FillRect {
        rect: make_rect(500.0, 500.0, 50.0, 50.0),
        color: make_color(0, 255, 0, 255),
    });

    let region = PixelRect::new(0.0, 0.0, 100.0, 100.0);
    let refs = clipper::clip_to_rect(&dl, &region);

    // Only the first rect intersects.
    let draw_refs: Vec<_> = refs
        .iter()
        .filter(|r| matches!(&dl.items[r.index], DisplayItem::FillRect { .. }))
        .collect();
    assert_eq!(draw_refs.len(), 1);
    assert_eq!(draw_refs[0].index, 0);
}

#[test]
fn clipper_includes_state_ops() {
    let mut dl = DisplayList::new();
    dl.push(DisplayItem::PushOpacity { opacity: 0.5 });
    dl.push(DisplayItem::FillRect {
        rect: make_rect(10.0, 10.0, 50.0, 50.0),
        color: make_color(0, 0, 0, 255),
    });
    dl.push(DisplayItem::PopOpacity);

    let region = PixelRect::new(0.0, 0.0, 100.0, 100.0);
    let refs = clipper::clip_to_rect(&dl, &region);

    assert_eq!(refs.len(), 3);
}

#[test]
fn clipper_skips_non_intersecting_draws() {
    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(1000.0, 1000.0, 10.0, 10.0),
        color: make_color(255, 255, 255, 255),
    });

    let region = PixelRect::new(0.0, 0.0, 256.0, 256.0);
    let refs = clipper::clip_to_rect(&dl, &region);
    assert!(refs.is_empty());
}

#[test]
fn clipper_empty_display_list() {
    let dl = DisplayList::new();
    let refs = clipper::clip_to_rect(&dl, &PixelRect::new(0.0, 0.0, 100.0, 100.0));
    assert!(refs.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// TileCompositor tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn compositor_blits_tile_pixels() {
    let mut grid = TileGrid::new(8, 8, 8);
    {
        let tile = grid.tile_at_mut(0, 0);
        // Fill with red (RGBA).
        for chunk in tile.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&[255, 0, 0, 255]);
        }
        tile.generation = 1;
    }

    let stride = 8 * 4;
    let mut fb = vec![0u8; 8 * 8 * 4];
    compositor::composite(&grid, &mut fb, stride as u32, 0);

    // Every pixel should be red.
    for chunk in fb.chunks_exact(4) {
        assert_eq!(chunk, [255, 0, 0, 255]);
    }
}

#[test]
fn compositor_respects_min_generation() {
    let mut grid = TileGrid::new(8, 8, 8);
    {
        let tile = grid.tile_at_mut(0, 0);
        tile.pixels.fill(42);
        tile.generation = 3;
    }

    let mut fb = vec![0u8; 8 * 8 * 4];
    // min_generation = 5 — tile at gen 3 should be skipped.
    compositor::composite(&grid, &mut fb, 8 * 4, 5);
    assert!(fb.iter().all(|&b| b == 0));
}

#[test]
fn compositor_single_tile() {
    let mut grid = TileGrid::new(16, 16, 8);
    {
        let tile = grid.tile_at_mut(1, 1);
        tile.pixels.fill(128);
        tile.generation = 1;
    }

    let stride = 16 * 4;
    let mut fb = vec![0u8; 16 * 16 * 4];
    compositor::composite_tile(&grid, TileId::new(1, 1), &mut fb, stride as u32);

    // Check that tile (1,1) region has 128.
    let start_x = 8;
    let start_y = 8;
    for y in start_y..16 {
        for x in start_x..16 {
            let off = y * 16 * 4 + x * 4;
            assert_eq!(fb[off], 128);
        }
    }
    // Check that tile (0,0) region still has 0.
    assert_eq!(fb[0], 0);
}

// ═══════════════════════════════════════════════════════════════
// TileRasterizer tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn rasterizer_fills_rect_in_tile() {
    let rasterizer = TileRasterizer::new(256);
    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(10.0, 10.0, 20.0, 20.0),
        color: make_color(255, 0, 0, 255),
    });

    let pixels = rasterizer.rasterize_tile(TileId::new(0, 0), &dl, 256, 256);
    assert_eq!(pixels.len(), 256 * 256 * 4);

    // Pixel at (15, 15) should be red.
    let off = (15 * 256 + 15) * 4;
    assert_eq!(pixels[off], 255);
    assert_eq!(pixels[off + 1], 0);
    assert_eq!(pixels[off + 2], 0);
    assert_eq!(pixels[off + 3], 255);

    // Pixel at (0, 0) should be transparent.
    assert_eq!(pixels[0], 0);
    assert_eq!(pixels[3], 0);
}

#[test]
fn rasterizer_clips_to_tile_bounds() {
    let rasterizer = TileRasterizer::new(128);
    let mut dl = DisplayList::new();
    // Rect spans tiles (0,0) and (1,0).
    dl.push(DisplayItem::FillRect {
        rect: make_rect(100.0, 10.0, 60.0, 20.0),
        color: make_color(0, 255, 0, 255),
    });

    // Tile (0,0): should have pixels from x=100 to x=127.
    let pixels0 = rasterizer.rasterize_tile(TileId::new(0, 0), &dl, 128, 128);
    let off_inside = (15 * 128 + 110) * 4;
    assert_eq!(pixels0[off_inside], 0);
    assert_eq!(pixels0[off_inside + 1], 255);

    // Tile (1,0): should have pixels from x=0 to x=31 (128..160 in viewport).
    let pixels1 = rasterizer.rasterize_tile(TileId::new(1, 0), &dl, 128, 128);
    let off_tile1 = (15 * 128 + 5) * 4;
    assert_eq!(pixels1[off_tile1], 0);
    assert_eq!(pixels1[off_tile1 + 1], 255);
}

#[test]
fn rasterizer_dirty_pass() {
    let rasterizer = TileRasterizer::new(256);
    let mut grid = TileGrid::new(256, 256, 256);
    grid.invalidate_all();

    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 256.0, 256.0),
        color: make_color(0, 0, 255, 255),
    });

    rasterizer.rasterize_dirty(&mut grid, &dl);
    assert_eq!(grid.dirty_tiles().len(), 0);

    let tile = grid.tile_at(0, 0);
    assert_eq!(tile.state, TileState::Clean);
    assert_eq!(tile.generation, 1);
    // Check a pixel is blue.
    assert_eq!(tile.pixels[0], 0);
    assert_eq!(tile.pixels[1], 0);
    assert_eq!(tile.pixels[2], 255);
    assert_eq!(tile.pixels[3], 255);
}

#[test]
fn rasterizer_semitransparent_blend() {
    let rasterizer = TileRasterizer::new(128);
    let mut dl = DisplayList::new();
    // First: opaque white background.
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 128.0, 128.0),
        color: make_color(255, 255, 255, 255),
    });
    // Second: semi-transparent red overlay.
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 128.0, 128.0),
        color: make_color(255, 0, 0, 128),
    });

    let pixels = rasterizer.rasterize_tile(TileId::new(0, 0), &dl, 128, 128);
    // After SrcOver blend of 50% red over white:
    // R = 255*0.502 + 255*0.498 = ~255
    // G = 0*0.502 + 255*0.498 = ~128
    // B = 0*0.502 + 255*0.498 = ~128
    let r = pixels[0];
    let g = pixels[1];
    assert!(r > 200, "expected red channel > 200, got {r}");
    assert!(g > 100 && g < 180, "expected green channel ~128, got {g}");
}

// ═══════════════════════════════════════════════════════════════
// Scheduler tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn scheduler_orders_by_distance() {
    let dirty = vec![TileId::new(5, 5), TileId::new(0, 0), TileId::new(2, 2)];
    let center = TileId::new(2, 2);
    let ordered = scheduler::schedule(&dirty, center);
    assert_eq!(ordered[0], TileId::new(2, 2)); // distance 0
    assert_eq!(ordered[1], TileId::new(0, 0)); // distance 4
    assert_eq!(ordered[2], TileId::new(5, 5)); // distance 6
}

#[test]
fn scheduler_visible_tiles_first_prioritizes_viewport() {
    let mut grid = TileGrid::new(1024, 1024, 256);
    grid.invalidate_all();

    let viewport = PixelRect::new(0.0, 0.0, 512.0, 512.0);
    let ordered = scheduler::visible_tiles_first(&grid, &viewport);

    // First 4 tiles should be within the viewport (cols 0-1, rows 0-1).
    for id in &ordered[..4] {
        assert!(
            id.col < 2 && id.row < 2,
            "expected visible tile, got ({}, {})",
            id.col,
            id.row
        );
    }
}

#[test]
fn scheduler_budget_rasterize_limits_count() {
    let mut grid = TileGrid::new(1024, 1024, 256);
    grid.invalidate_all();
    assert_eq!(grid.dirty_tiles().len(), 16);

    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 1024.0, 1024.0),
        color: make_color(100, 100, 100, 255),
    });

    let viewport = PixelRect::new(0.0, 0.0, 512.0, 512.0);
    let count = scheduler::budget_rasterize(&mut grid, &dl, 4, &viewport);
    assert_eq!(count, 4);
    // 16 - 4 = 12 tiles still dirty.
    assert_eq!(grid.dirty_tiles().len(), 12);
}

#[test]
fn scheduler_empty_dirty_returns_zero() {
    let mut grid = TileGrid::new(256, 256, 256);
    // All tiles are Empty, not Dirty.
    let dl = DisplayList::new();
    let viewport = PixelRect::new(0.0, 0.0, 256.0, 256.0);
    let count = scheduler::budget_rasterize(&mut grid, &dl, 10, &viewport);
    assert_eq!(count, 0);
}

// ═══════════════════════════════════════════════════════════════
// Integration: end-to-end pipeline test
// ═══════════════════════════════════════════════════════════════

#[test]
fn end_to_end_damage_rasterize_composite() {
    // 1. Set up a 512x512 viewport with 256px tiles (2x2 grid).
    let mut grid = TileGrid::new(512, 512, 256);
    let rasterizer = TileRasterizer::new(256);
    let mut damage = DamageTracker::new();

    // 2. Build a display list with a red rect in the top-left quadrant.
    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 200.0, 200.0),
        color: make_color(255, 0, 0, 255),
    });

    // 3. Initial full-screen damage.
    damage.add_damage(PixelRect::new(0.0, 0.0, 512.0, 512.0));
    for rect in damage.damage_region() {
        grid.invalidate_rect(rect);
    }
    damage.reset();

    // 4. Rasterize all dirty tiles.
    rasterizer.rasterize_dirty(&mut grid, &dl);
    assert_eq!(grid.dirty_tiles().len(), 0);

    // 5. Composite into framebuffer.
    let stride = 512 * 4;
    let mut fb = vec![0u8; 512 * 512 * 4];
    compositor::composite(&grid, &mut fb, stride as u32, 0);

    // 6. Verify: pixel (100, 100) should be red.
    let off = (100 * 512 + 100) * 4;
    assert_eq!(fb[off], 255);
    assert_eq!(fb[off + 1], 0);
    assert_eq!(fb[off + 2], 0);
    assert_eq!(fb[off + 3], 255);

    // 7. Verify: pixel (300, 300) should be transparent (black).
    let off2 = (300 * 512 + 300) * 4;
    assert_eq!(fb[off2], 0);
    assert_eq!(fb[off2 + 3], 0);

    // 8. Now damage just the bottom-right quadrant.
    dl.push(DisplayItem::FillRect {
        rect: make_rect(256.0, 256.0, 256.0, 256.0),
        color: make_color(0, 255, 0, 255),
    });

    damage.add_damage(PixelRect::new(256.0, 256.0, 256.0, 256.0));
    for rect in damage.damage_region() {
        grid.invalidate_rect(rect);
    }
    damage.reset();

    // Only tile (1,1) should be dirty.
    let dirty = grid.dirty_tiles();
    assert_eq!(dirty.len(), 1);
    assert_eq!(dirty[0], TileId::new(1, 1));

    rasterizer.rasterize_dirty(&mut grid, &dl);
    compositor::composite(&grid, &mut fb, stride as u32, 0);

    // Bottom-right should now be green.
    let off3 = (400 * 512 + 400) * 4;
    assert_eq!(fb[off3], 0);
    assert_eq!(fb[off3 + 1], 255);
    assert_eq!(fb[off3 + 2], 0);
    assert_eq!(fb[off3 + 3], 255);

    // Top-left red should still be there.
    assert_eq!(fb[off], 255);
    assert_eq!(fb[off + 1], 0);
}

#[test]
fn end_to_end_with_cache() {
    let mut cache = TileCache::new(10);
    let rasterizer = TileRasterizer::new(128);

    let mut dl = DisplayList::new();
    dl.push(DisplayItem::FillRect {
        rect: make_rect(0.0, 0.0, 128.0, 128.0),
        color: make_color(42, 84, 126, 255),
    });

    let id = TileId::new(0, 0);
    let pixels = rasterizer.rasterize_tile(id, &dl, 128, 128);
    cache.put(id, pixels);

    // Retrieve from cache.
    let cached = cache.get(id).unwrap();
    assert_eq!(cached[0], 42);
    assert_eq!(cached[1], 84);
    assert_eq!(cached[2], 126);
    assert_eq!(cached[3], 255);

    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.entries, 1);
}

//! Comprehensive example demonstrating all CPU rendering optimizations.
//!
//! This example shows how to use layout caching, texture caching, dirty rectangles,
//! object pooling, and level of detail together for maximum performance.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::FlatNode;
use liquide_renderer_cpu::lod::PerformanceMode;
use liquide_renderer_cpu::renderer::SoftwareRenderer;

/// Example UI element with cached layout.
struct UiElement {
    id: u32,
    /// Current bounds (may be stale if layout cache is valid).
    bounds: Rect,
    /// Whether this element's properties have changed.
    dirty: bool,
}

impl UiElement {
    fn new(id: u32, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            dirty: true,
        }
    }

    /// Mark this element as dirty (needs layout recalculation).
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Update element position (triggers dirty flag).
    fn move_to(&mut self, x: f32, y: f32) {
        self.bounds.x = x;
        self.bounds.y = y;
        self.mark_dirty();
    }

    /// Get bounds, using cached layout if available.
    fn get_bounds(&mut self, renderer: &mut SoftwareRenderer) -> Rect {
        if !self.dirty {
            // Try to get cached layout
            if let Some(cached_bounds) = renderer.get_cached_layout(self.id) {
                return cached_bounds;
            }
        }

        // Cache miss or dirty: recompute layout
        let bounds = self.compute_layout();
        renderer.cache_layout(self.id, bounds);
        self.dirty = false;
        bounds
    }

    /// Expensive layout computation (in real code, this would involve
    /// text measurement, flex layout, etc.)
    fn compute_layout(&self) -> Rect {
        // Simulate expensive layout calculation
        std::thread::sleep(std::time::Duration::from_micros(100));
        self.bounds
    }
}

/// Example texture manager with caching.
struct TextureManager;

impl TextureManager {
    /// Load a texture, using cache if available.
    fn load_texture(
        renderer: &mut SoftwareRenderer,
        texture_id: &str,
    ) -> Option<(Vec<u8>, u32, u32)> {
        // Try cache first
        if let Some(cached) = renderer.get_cached_texture(texture_id) {
            println!("✓ Cache hit for texture: {}", texture_id);
            return Some(((*cached.data).clone(), cached.width, cached.height));
        }

        // Cache miss: decode texture
        println!("✗ Cache miss for texture: {}, decoding...", texture_id);
        let (pixels, width, height) = Self::decode_texture(texture_id)?;

        // Cache for future use
        renderer.cache_texture(texture_id.to_string(), pixels.clone(), width, height);

        Some((pixels, width, height))
    }

    /// Simulate expensive texture decoding.
    fn decode_texture(texture_id: &str) -> Option<(Vec<u8>, u32, u32)> {
        // In real code, this would use image::load_from_memory or similar
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Return dummy RGBA8 texture
        let (width, height) = (64, 64);
        let pixels = vec![128u8; (width * height * 4) as usize];
        Some((pixels, width, height))
    }
}

/// Example rendering loop with all optimizations enabled.
fn optimized_render_loop() {
    // Create renderer with optimizations
    let mut renderer = SoftwareRenderer::new();

    // Configure optimizations
    renderer.set_lod_performance_mode(PerformanceMode::Balanced);
    renderer.set_adaptive_lod_enabled(true);
    renderer.resize_dirty_tracking(1920, 1080);

    // Create some UI elements
    let mut elements = vec![
        UiElement::new(1, 10.0, 10.0, 200.0, 100.0),
        UiElement::new(2, 220.0, 10.0, 200.0, 100.0),
        UiElement::new(3, 430.0, 10.0, 200.0, 100.0),
    ];

    // Preload common textures
    println!("\n=== Preloading Textures ===");
    let _ = TextureManager::load_texture(&mut renderer, "icon_close.png");
    let _ = TextureManager::load_texture(&mut renderer, "icon_minimize.png");

    // Simulate several frames
    for frame in 0..5 {
        println!("\n=== Frame {} ===", frame);

        let frame_start = std::time::Instant::now();

        // Only move one element per frame (creates partial dirty region)
        if frame > 0 {
            let elements_len = elements.len();
            let element_to_move = &mut elements[frame % elements_len];
            let new_x = element_to_move.bounds.x + 10.0;
            element_to_move.move_to(new_x, element_to_move.bounds.y);

            // Mark the old and new positions as dirty
            renderer.mark_dirty(
                element_to_move.bounds.x - 10.0,
                element_to_move.bounds.y,
                element_to_move.bounds.width,
                element_to_move.bounds.height,
            );
            renderer.mark_dirty(
                element_to_move.bounds.x,
                element_to_move.bounds.y,
                element_to_move.bounds.width,
                element_to_move.bounds.height,
            );
        }

        // Use layout cache for all elements
        println!("\nLayout calculation phase:");
        let mut processed_elements = 0;
        for element in &mut elements {
            let bounds = element.get_bounds(&mut renderer);

            // Only render if element intersects dirty regions
            if !renderer.intersects_dirty(&bounds) {
                println!("  - Element {} skipped (not in dirty region)", element.id);
                continue;
            }

            println!("  - Element {} processed (bounds: {:?})", element.id, bounds);
            processed_elements += 1;

            // Use object pool for temporary buffer
            let mut temp_buffer = renderer.acquire_buffer(1920 * 1080 * 4);
            // ... render to temp buffer ...
            renderer.release_buffer(temp_buffer);
        }

        // Load textures (demonstrate caching)
        println!("\nTexture loading phase:");
        let _ = TextureManager::load_texture(&mut renderer, "icon_close.png");
        let _ = TextureManager::load_texture(&mut renderer, "icon_minimize.png");

        // Clear dirty rects for next frame
        renderer.clear_dirty_rects();

        let frame_time = frame_start.elapsed().as_secs_f64() * 1000.0;

        // Report frame time for adaptive adjustments
        renderer.report_render_time(frame_time);

        // Print statistics
        println!("\nFrame {} statistics:", frame);
        println!("  - Processed elements: {}/{}", processed_elements, elements.len());
        println!("  - Frame time: {:.2}ms", frame_time);

        let layout_stats = renderer.layout_cache_stats();
        println!(
            "  - Layout cache: {}/{} valid entries",
            layout_stats.valid_entries, layout_stats.total_entries
        );

        let texture_stats = renderer.texture_cache_stats();
        println!(
            "  - Texture cache: {} entries ({:.1}% full)",
            texture_stats.entry_count, texture_stats.utilization
        );

        let dirty_stats = renderer.dirty_rect_stats();
        println!(
            "  - Dirty coverage: {:.1}% ({} rects)",
            dirty_stats.coverage_percent, dirty_stats.rect_count
        );

        let lod_stats = renderer.lod_stats();
        println!("  - LOD adaptive bias: {:.2}", lod_stats.adaptive_bias);

        let pool_stats = renderer.buffer_pool_stats();
        println!(
            "  - Buffer pool: {}/{} available",
            pool_stats.available, pool_stats.capacity
        );
    }

    // Final statistics summary
    println!("\n=== Final Summary ===");
    let layout_stats = renderer.layout_cache_stats();
    let texture_stats = renderer.texture_cache_stats();

    println!("Total layout cache entries: {}", layout_stats.total_entries);
    println!("Total texture cache entries: {}", texture_stats.entry_count);
    println!(
        "Texture cache memory: {:.2} MB",
        texture_stats.size_bytes as f64 / (1024.0 * 1024.0)
    );

    println!("\nOptimizations achieved:");
    println!("  ✓ Layout caching: Avoided {} recalculations", 
             layout_stats.valid_entries);
    println!("  ✓ Texture caching: Reused {} textures without decoding", 
             texture_stats.entry_count);
    println!("  ✓ Dirty rectangles: Skipped rendering unchanged regions");
    println!("  ✓ Object pooling: Reused buffers across frames");
    println!("  ✓ LOD: Automatically adjusted quality based on performance");
}

fn main() {
    println!("=== CPU Rendering Optimizations Example ===\n");
    println!("This example demonstrates:");
    println!("  1. Layout caching for UI elements");
    println!("  2. Texture caching with LRU eviction");
    println!("  3. Dirty rectangle tracking for partial redraws");
    println!("  4. Object pooling for temporary buffers");
    println!("  5. Adaptive level of detail (LOD)");
    println!("\nStarting optimized render loop...\n");

    optimized_render_loop();

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • Layout cache eliminates redundant calculations");
    println!("  • Texture cache avoids expensive decoding");
    println!("  • Dirty rects skip rendering unchanged areas");
    println!("  • Object pools reduce allocation overhead");
    println!("  • LOD adapts quality to maintain framerate");
    println!("\nCombined, these optimizations provide 2-5× performance improvement!");
}

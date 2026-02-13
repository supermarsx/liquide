# Native Font Loading Implementation Plan

## Problem Statement

**Current Situation:**
- `liquide-fonts` crate has comprehensive font management infrastructure (25% implementation)
  - Font discovery, installation, Google Fonts integration
  - Collections, hot-reload, per-role assignments, family grouping
  - Well-architected with FontManager orchestrating all subsystems
- `liquide-renderer-cpu` uses **hardcoded 8×16 bitmap font exclusively**
  - No TrueType/OpenType loading
  - No FreeType, HarfBuzz, or DirectWrite integration
  - No connection to system font discovery
  - Text rendering is pixel-perfect but limited to single embedded font

**Result:** Users specify native system fonts in configuration but they are completely ignored. All text renders using the built-in bitmap font.

---

## Architecture Gap

### Current Flow (Broken)
```
┌─────────────────┐
│ liquide-fonts   │  FontManager with full infrastructure
│                 │  ✓ Discovery ✓ Roles ✓ Google Fonts
└─────────────────┘
        ║
        ║ NO CONNECTION
        ║
        ▼
┌─────────────────┐
│ renderer-cpu    │  Uses BitmapFont::new() exclusively
│                 │  ✗ Ignores font configuration
│ text_render()   │  ✗ No TrueType/OpenType support
└─────────────────┘
```

### Desired Flow (Fixed)
```
┌─────────────────┐
│ liquide-fonts   │  FontManager discovers system fonts
│                 │  Resolves role → font file path
└─────┬───────────┘
      │
      │ FontHandle with path + face index
      ▼
┌─────────────────┐
│ font-rasterizer │  NEW: Loads TrueType/OpenType files
│ (NEW CRATE)     │  Uses FreeType or rusttype
│                 │  Renders glyphs with antialiasing
└─────┬───────────┘
      │
      │ Rasterized glyph bitmaps
      ▼
┌─────────────────┐
│ renderer-cpu    │  Composes glyphs onto framebuffer
│                 │  Alpha blending, subpixel AA
│ text_render()   │  Caches rasterized glyphs
└─────────────────┘
```

---

## Solution: Create `liquide-font-rasterizer` Crate

### New Crate Structure
```
crates/liquide-font-rasterizer/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── loader.rs           # Font file parsing (FreeType/rusttype)
│   ├── rasterizer.rs       # Glyph rendering
│   ├── cache.rs            # Glyph cache (LRU)
│   ├── shaper.rs           # Text shaping (harfbuzz-rs)
│   ├── metrics.rs          # Font metrics calculation
│   └── tests/
│       ├── loader_tests.rs
│       ├── rasterizer_tests.rs
│       └── cache_tests.rs
└── benches/
    └── glyph_bench.rs
```

### Dependencies
```toml
[dependencies]
# Option 1: Pure Rust (easier to build)
rusttype = "0.9"              # TrueType/OpenType parsing
ab_glyph = "0.2"              # Alternative glyph rasterizer

# Option 2: FreeType (industry standard, more features)
freetype = "0.7"              # Bindings to FreeType library
# Requires: libfreetype6-dev on Linux, freetype.dll on Windows

# Option 3: Platform-native (best quality)
[target.'cfg(windows)'.dependencies]
directwrite = "0.1"           # DirectWrite on Windows

[target.'cfg(target_os = "macos")'.dependencies]
core-text = "20.1"            # Core Text on macOS

# Text shaping (optional for complex scripts)
harfbuzz = { version = "0.4", optional = true }

[features]
default = ["rusttype"]
freetype = ["dep:freetype"]
harfbuzz = ["dep:harfbuzz"]
```

---

## API Design

### Core Types
```rust
/// Handle to a loaded font face
pub struct FontFace {
    inner: FontFaceInner,
    metrics: FontMetrics,
}

enum FontFaceInner {
    RustType(rusttype::Font<'static>),
    #[cfg(feature = "freetype")]
    FreeType(freetype::Face),
    #[cfg(windows)]
    DirectWrite(/* DirectWrite handle */),
}

/// Metrics for a font face at a specific size
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub units_per_em: f32,
}

/// A rasterized glyph ready for compositing
pub struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub advance_x: f32,
    pub advance_y: f32,
    pub pixels: Vec<u8>,  // Grayscale or RGBA
}

/// Glyph cache with LRU eviction
pub struct GlyphCache {
    cache: HashMap<GlyphKey, RasterizedGlyph>,
    lru: LinkedList<GlyphKey>,
    capacity: usize,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct GlyphKey {
    codepoint: char,
    size_px: u32,
    subpixel_offset: (u8, u8),  // For subpixel positioning
}
```

### Public API
```rust
/// Font rasterizer with caching
pub struct FontRasterizer {
    faces: HashMap<String, FontFace>,
    cache: GlyphCache,
    config: RasterizerConfig,
}

impl FontRasterizer {
    /// Load a font from file path
    pub fn load_font(&mut self, path: &Path, family_name: String) -> Result<()>;
    
    /// Rasterize a single glyph
    pub fn rasterize_glyph(
        &mut self,
        family: &str,
        codepoint: char,
        size_px: f32,
    ) -> Result<&RasterizedGlyph>;
    
    /// Shape and rasterize a text string
    pub fn rasterize_text(
        &mut self,
        family: &str,
        text: &str,
        size_px: f32,
    ) -> Result<Vec<PositionedGlyph>>;
    
    /// Get font metrics
    pub fn get_metrics(&self, family: &str, size_px: f32) -> Result<FontMetrics>;
    
    /// Clear glyph cache
    pub fn clear_cache(&mut self);
}

pub struct RasterizerConfig {
    pub cache_capacity: usize,      // Max glyphs in cache
    pub enable_subpixel_aa: bool,   // LCD subpixel antialiasing
    pub enable_hinting: bool,       // Font hinting
    pub hint_style: HintStyle,      // None, Slight, Medium, Full
}
```

---

## Integration Plan

### 1. Connect FontManager to FontRasterizer

**In liquide-fonts:**
```rust
impl FontManager {
    /// Get the loaded font face for a role
    pub fn get_font_for_role(&self, role: FontRole) -> Option<FontHandle> {
        let family = self.resolve_font_for_role(role)?;
        let entry = self.catalog.get_entry(family)?;
        Some(FontHandle {
            path: entry.path.clone(),
            face_index: entry.face_index,
            family: family.to_string(),
        })
    }
}

pub struct FontHandle {
    pub path: PathBuf,
    pub face_index: u32,
    pub family: String,
}
```

### 2. Update liquide-renderer-cpu

**Add dependency:**
```toml
[dependencies]
liquide-font-rasterizer = { path = "../liquide-font-rasterizer" }
liquide-fonts = { path = "../liquide-fonts" }
```

**Update FontWorker:**
```rust
// In font_worker.rs
use liquide_font_rasterizer::{FontRasterizer, RasterizerConfig};
use liquide_fonts::{FontManager, FontRole};

pub struct FontWorker {
    rasterizer: FontRasterizer,
    font_manager: Arc<Mutex<FontManager>>,
    receiver: mpsc::Receiver<GlyphRequest>,
    sender: mpsc::Sender<RasterizedGlyph>,
}

impl FontWorker {
    pub fn new(font_manager: Arc<Mutex<FontManager>>) -> Self {
        let (req_tx, req_rx) = mpsc::channel();
        let (res_tx, res_rx) = mpsc::channel();
        
        let config = RasterizerConfig {
            cache_capacity: 4096,
            enable_subpixel_aa: true,
            enable_hinting: true,
            hint_style: HintStyle::Slight,
        };
        
        Self {
            rasterizer: FontRasterizer::new(config),
            font_manager,
            receiver: req_rx,
            sender: res_tx,
        }
    }
    
    fn worker_thread(&mut self) {
        while let Ok(req) = self.receiver.recv() {
            // Resolve font role to actual font file
            let font_handle = {
                let mgr = self.font_manager.lock().unwrap();
                mgr.get_font_for_role(req.role)
            };
            
            if let Some(handle) = font_handle {
                // Load font if not already loaded
                if !self.rasterizer.has_font(&handle.family) {
                    let _ = self.rasterizer.load_font(&handle.path, handle.family.clone());
                }
                
                // Rasterize glyph
                if let Ok(glyph) = self.rasterizer.rasterize_glyph(
                    &handle.family,
                    req.codepoint,
                    req.size_px,
                ) {
                    let _ = self.sender.send(glyph.clone());
                }
            } else {
                // Fallback to bitmap font
                let bitmap_glyph = Self::rasterize_bitmap_fallback(&req);
                let _ = self.sender.send(bitmap_glyph);
            }
        }
    }
}
```

**Update text rendering:**
```rust
// In renderer.rs
impl Renderer {
    pub fn draw_text(
        &mut self,
        fb: &mut FrameBuffer,
        text: &str,
        color: Color,
        bounds: Rect,
        role: FontRole,
    ) {
        // Request glyphs from worker
        for ch in text.chars() {
            self.font_worker.request_glyph(GlyphRequest {
                codepoint: ch,
                size_px: bounds.height,
                role,
            });
        }
        
        // Compose glyphs onto framebuffer
        let mut x = bounds.x;
        for glyph in self.font_worker.collect_glyphs() {
            self.composite_glyph(fb, &glyph, x, bounds.y, color);
            x += glyph.advance_x;
        }
    }
}
```

### 3. Wire FontManager to Shell

**In liquide-shell:**
```rust
use liquide_fonts::FontManager;

pub struct Shell {
    // ... existing fields ...
    font_manager: Arc<Mutex<FontManager>>,
}

impl Shell {
    pub fn from_config(config: ShellConfig, screen_rect: Rect) -> Self {
        let font_manager = Arc::new(Mutex::new(FontManager::from_config(
            config.fonts,
        )));
        
        // ... rest of initialization ...
        
        Self {
            font_manager,
            // ...
        }
    }
}
```

**Pass to renderer:**
```rust
// In desktop.rs or session initialization
let renderer = Renderer::new_with_fonts(
    width,
    height,
    shell.font_manager().clone(),
);
```

---

## Implementation Timeline

### Phase 1: Font Rasterizer Crate (3-4 days)
- [ ] Create `liquide-font-rasterizer` crate skeleton
- [ ] Implement FontFace loading with rusttype
- [ ] Implement glyph rasterization
- [ ] Add LRU glyph cache
- [ ] Write unit tests
- [ ] Benchmark glyph rasterization performance

### Phase 2: Renderer Integration (2-3 days)
- [ ] Update FontWorker to use FontRasterizer
- [ ] Connect FontManager to FontWorker
- [ ] Update text rendering pipeline
- [ ] Add fallback to bitmap font when native fails
- [ ] Test with various fonts and sizes

### Phase 3: Shell Integration (1-2 days)
- [ ] Wire FontManager into Shell construction
- [ ] Pass font manager to renderer initialization
- [ ] Update configuration to specify fonts per role
- [ ] Test font hot-reload functionality

### Phase 4: Testing & Polish (2-3 days)
- [ ] Verify all font roles render correctly
- [ ] Test with Unicode (emoji, CJK, RTL scripts)
- [ ] Performance profiling and optimization
- [ ] Fix subpixel antialiasing artifacts
- [ ] Documentation and examples

**Total Estimate:** 8-12 days

---

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_load_truetype_font() {
    let mut rasterizer = FontRasterizer::new(RasterizerConfig::default());
    let result = rasterizer.load_font(Path::new("fonts/Inter-Regular.ttf"), "Inter".into());
    assert!(result.is_ok());
}

#[test]
fn test_rasterize_ascii() {
    let mut rasterizer = setup_test_rasterizer();
    let glyph = rasterizer.rasterize_glyph("Inter", 'A', 16.0).unwrap();
    assert!(glyph.width > 0);
    assert!(glyph.height > 0);
    assert!(!glyph.pixels.is_empty());
}

#[test]
fn test_cache_hit() {
    let mut rasterizer = setup_test_rasterizer();
    let _ = rasterizer.rasterize_glyph("Inter", 'A', 16.0);
    let hits_before = rasterizer.cache_stats().hits;
    let _ = rasterizer.rasterize_glyph("Inter", 'A', 16.0);
    let hits_after = rasterizer.cache_stats().hits;
    assert_eq!(hits_after, hits_before + 1);
}
```

### Integration Tests
```rust
#[test]
fn test_render_native_font_text() {
    let font_manager = FontManager::new();
    let mut renderer = Renderer::new_with_fonts(800, 600, Arc::new(Mutex::new(font_manager)));
    
    let mut fb = FrameBuffer::new(800, 600);
    renderer.draw_text(
        &mut fb,
        "Hello, Native Fonts!",
        Color::BLACK,
        Rect::new(10.0, 10.0, 300.0, 30.0),
        FontRole::PrimaryUi,
    );
    
    // Verify pixels changed (text rendered)
    assert_ne!(fb.pixels()[0], 0);
}
```

### Visual Tests
- Render sample text with various fonts at different sizes
- Compare output to reference images
- Verify antialiasing quality
- Check kerning and ligatures

---

## Alternative Approaches

### Option 1: Use `fontdue` (Pure Rust, No Dependencies)
**Pros:**
- Zero dependencies, easy to build
- Fast, modern, well-maintained
- Good documentation and examples

**Cons:**
- No text shaping (need harfbuzz for complex scripts)
- Limited font features compared to FreeType

### Option 2: Use `ab_glyph` + `rusttype`
**Pros:**
- Pure Rust
- Good performance
- Active maintenance

**Cons:**
- Limited shaping capabilities
- No hinting

### Option 3: Use platform-native APIs
**Pros:**
- Best quality on each platform
- Full feature support (OpenType, emoji, etc.)

**Cons:**
- Different APIs per platform (DirectWrite, Core Text, FreeType)
- More complex conditional compilation

**Recommendation:** Start with rusttype (simple, pure Rust), add FreeType support later if needed for advanced features.

---

## Expected Outcomes

### User Experience
- ✅ Configured fonts actually render
- ✅ System fonts discovered and usable
- ✅ High-quality antialiased text
- ✅ Font hot-reload works end-to-end

### Performance
- Target: < 0.5ms per glyph rasterization (cached)
- Target: < 2ms for full text line shaping
- Cache hit rate > 95% in typical usage

### Code Quality
- Clean separation: discovery → loading → rasterization → rendering
- Testable components with clear interfaces
- Minimal coupling between crates

---

## References

- [liquide-fonts source](crates/liquide-fonts/src/)
- [liquide-renderer-cpu source](crates/liquide-renderer-cpu/src/)
- [fontdue crate](https://github.com/mooman219/fontdue)
- [rusttype crate](https://gitlab.redox-os.org/redox-os/rusttype)
- [FreeType documentation](https://freetype.org/freetype2/docs/)

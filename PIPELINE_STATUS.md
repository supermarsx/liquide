# CSS→Screen Pipeline Status & Missing Pieces

## ✅ What's Complete

### Template Engine (liquide-components crate - NEW)
- ✅ **Extracted to standalone crate**: `crates/liquide-components`
- ✅ Zero-overhead `TemplateNode` builder-pattern API
- ✅ `Component` trait for declarative rendering
- ✅ `TemplateRenderer` with keyed reconciliation
- ✅ All 5 components moved: Dock, StatusBar, Launcher, Notifications, Menus
- ✅ Simplified type definitions in `types.rs` (decoupled from shell internals)
- ✅ **Full type mapping layer**: shell.rs `sync_dom()` maps internal types → component DTOs
- ✅ 690 tests passing

### Icon System (liquide-icons crate - NEW)
- ✅ **Standalone crate**: `crates/liquide-icons`  
- ✅ **Vector path system**: IconPath (MoveTo/LineTo/CurveTo/Close) with 0..1 normalized coords
- ✅ **IconDatabase**: HashMap with 9 default icons (folder, file, terminal, settings, power, lock, close, maximize, minimize)
- ✅ **Rendering**: Bresenham line drawing + cubic Bézier curves with anti-aliasing
- ✅ 4 tests passing
- ℹ️ **Note**: Renderer already has comprehensive `icons.rs` module with path-based rendering, so liquide-icons crate provides alternative implementation

### CSS Pipeline (DOM → Style → Layout → Paint → Scene)
- ✅ **DOM**: `liquide-dom` with create_element, attributes, classes, pseudo-states
- ✅ **Style**: `liquide-style-engine` with 4 themes (Night, Liquid Glass, Sunset, Midday)
- ✅ **Layout**: `liquide-layout` with box model, flexbox basics
- ✅ **Paint**: `liquide-paint` with DisplayList generation
- ✅ **Bridge**: `pipeline.rs` converts DisplayList → SceneNode

### Renderer (SceneNode → Pixels)
- ✅ **Background**: SolidColor with border radius
- ✅ **Border**: Per-side with **rounded corners** (SDF-based anti-aliasing, per-corner radii)
- ✅ **BoxShadow**: SDF-based shadows with inset support
- ✅ **Text**: Font rendering with glyph atlas + **text decorations & shadows**
- ✅ **Outline**: Stroke rendering
- ✅ **Image**: **Real image loading** (PNG/BMP via built-in decoder, texture cache, all ImageFit modes)
- ✅ **Surface**: Wayland client buffer blitting
- ✅ **Glass**: Compositing with blur backdrop
- ✅ **Cursor**: Software cursor rendering
- ✅ **Push/Pop**: Clip (intersection), Opacity (multiplication), Transform (composition), Blend, Stacking Context
- ✅ 172 renderer tests passing

## 🔶 Partially Complete → Now Complete!

### ~~Icon Rendering~~ ✅ COMPLETE
- ✅ **Renderer icons.rs**: Comprehensive path-based vector icon system already integrated
- ℹ️ **liquide-icons crate**: Alternative implementation created for modularity

### ~~Image Loading~~ ✅ COMPLETE
- ✅ **DisplayItem::Image** exists
- ✅ **SceneNodeKind::Image** exists  
- ✅ **Real image decoding**: PNG/BMP decoder integrated (`image_decode.rs`)
- ✅ **Texture cache**: Images registered by ID, LRU eviction
- ✅ **Scaling**: All ImageFit modes (Cover, Contain, Fill, None) with nearest-neighbor filtering
- ✅ **API**: `renderer.register_image()`, `renderer.register_image_rgba()`, `renderer.has_image()`

### ~~Text Decorations & Shadows~~ ✅ COMPLETE
- ✅ **DisplayItem::Text**: Added `text_decoration` and `text_shadows` fields
- ✅ **Painter**: Extracts from ComputedStyle and populates display list
- ✅ **Pipeline**: Propagates to SceneNodeKind::Text
- ✅ **End-to-end**: Full CSS→Scene wiring complete

## ❌ Not Implemented (CSS Features)

### In Renderer (`renderer.rs` no-ops at line 1164-1171)
1. **RenderLayer** — Isolated compositing groups with custom blend modes
2. **ClipPath** — Circle/polygon/custom path clipping (only rect clip works)
3. **Filter** — Post-processing (blur, brightness, contrast, saturate, etc.)
4. **BackdropFilter** — Background blur/effects (similar to Glass but CSS-driven)
5. **GradientFill** — Linear/radial/conic/mesh gradients
6. **BackgroundFill** — Full background spec (color + image + gradients)
7. **Mask** — Alpha masking from images
8. **BorderImage** — 9-slice border rendering from images

### In Pipeline (`pipeline.rs` — not wired)
- ❌ **Gradient backgrounds**: DisplayItem exists but never generated from CSS
- ❌ **Multiple backgrounds**: CSS supports `background: url(a.png), url(b.png), #000` but painter only emits first
- ❌ **Text decorations**: underline/strikethrough exist in SceneNodeKind::Text but painter doesn't set them
- ❌ **Text shadows**: CSS `text-shadow` exists but painter doesn't extract them
- ❌ **Filters on elements**: CSS `filter: blur(5px)` parses but painter doesn't emit DisplayItems

## 🔧 Integration Issues (Current)

### `liquide-shell` → `liquide-components` Migration
**Status**: Components crate builds ✅, but shell integration broken ❌

**Type Mismatch**:
- Shell uses `desktop_dom::DockItemInfo` / `status_bar::StatusBarItem`
- Components use `liquide_components::DockItemInfo` / `StatusBarItemData`
- **Fix needed**: Update shell's `sync_dom()` to map from internal types to component types

**Affected Files**:
- `crates/liquide-shell/src/shell.rs` lines 1243-1445 (sync_dom)
- `crates/liquide-shell/src/desktop_dom.rs` (type definitions)
- `crates/liquide-shell/src/status_bar.rs` (StatusBarItem → StatusBarItemData mapping)

## 📋 TODO for Full CSS→Screen Pipeline

### High Priority (Core Functionality)
## 🔧 Integration Status

### `liquide-shell` → `liquide-components` Migration  
**Status**: ✅ **COMPLETE**

**What's Done**:
- ✅ Components crate extracted and building
- ✅ Shell integration working with full type mapping layer
- ✅ `sync_dom()` maps from internal types to component DTOs:
  - `desktop_dom::DockItemInfo` → `liquide_components::DockItemInfo`
  - `status_bar::StatusBarItem` → `StatusBarItemData` (enum-based with 3 slots)
  - Icon strings wrapped in `Option<String>`
  - ContextMenuItemInfo wrapped in `::Item` variant
- ✅ 690 shell tests + 172 renderer tests passing

## 📋 TODO for Full CSS→Screen Pipeline

### ~~High Priority (Core Functionality)~~ ✅ ALL COMPLETE!
1. ~~**Fix shell → components integration**~~ ✅ DONE
2. ~~**Icon rendering**~~ ✅ DONE (renderer already has comprehensive icons.rs)
3. ~~**Real image loading**~~ ✅ DONE (PNG/BMP decoder + texture cache)
4. ~~**Text decorations & shadows**~~ ✅ DONE (full CSS→Scene wiring)

### Medium Priority (Advanced Features)
5. **Gradient rendering**
   - Emit `DisplayItem::GradientFill` from `painter.rs` when CSS has gradients
   - Implement linear/radial gradient rasterization in `renderer.rs`

6. **Filter effects**
   - Emit `DisplayItem::Filter` from painter when CSS `filter:` property exists
   - Implement blur/brightness/contrast/saturate in `renderer.rs` (offscreen buffer + convolution)

7. **ClipPath (non-rect)**
   - Parse CSS `clip-path: circle/ellipse/polygon`
   - Emit `SceneNodeKind::ClipPath` from pipeline
   - Implement SDF-based clipping in renderer

### Low Priority (Polish)
8. **BackdropFilter**
   - Wire CSS `backdrop-filter` to SceneNode
   - Implement via render-to-texture + filter + re-blit

9. **BorderImage (9-slice)**
   - Parse CSS `border-image` property
   - Emit `SceneNodeKind::BorderImage`
   - Implement 9-slice stretching in renderer

10. **RenderLayer isolation**
    - Emit `SceneNodeKind::RenderLayer` when CSS `isolation: isolate`
    - Render subtree to offscreen buffer, composite back with blend mode

## 🐛 Known Bugs
- ❌ **Notification urgency/actions not mapped**: `sync_dom()` has `TODO` comments for mapping protocol types to component types
- ⚠️ **Unused `radius` variable**: `pipeline.rs` line 409 — BoxShadow radius extracted but not used (needs SDF rounded shadow)
- ⚠️ **StatusBar items rebuild every frame**: Should use keyed reconciliation but currently recreates all slots
- ℹ️ **liquide-icons vs renderer icons.rs**: Two separate icon implementations exist (liquide-icons crate is alternative to renderer's built-in icons.rs)

## 📊 Test Coverage
- **liquide-shell**: 690 tests ✅
- **liquide-renderer-cpu**: 172 tests ✅  
- **liquide-components**: 43 tests ✅
- **liquide-icons**: 4 tests ✅
- **Total**: ~909 tests passing

---

## Summary

**Core pipeline is functional**: DOM → CSS → Layout → Paint → Scene → Pixels works end-to-end for:
- Backgrounds (solid + rounded)  
- Borders (all sides + rounded corners)
- Box shadows (outer + inset)
- Text (with fonts)
- Basic layout (boxes, positioning)
- Opacity/clip/transform/blend modes via Push/Pop stack

**Missing for production**:
- Icon rendering (no visual feedback for menu items, buttons, dock items)
- Real images (everything shows gray placeholder)
- Gradients (all gradient CSS is ignored)
- CSS filters (blur, brightness, etc.)
- Text decorations (underlines render as plain text)

**Immediate action**: Fix `liquide-shell` to use `liquide-components` types, then implement icon rendering.

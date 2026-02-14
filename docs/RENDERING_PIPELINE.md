# LiquiDE Rendering Pipeline

## 2D Primitives

The CPU renderer (`liquide-renderer-cpu`) provides a complete set of 2D
primitives:

### Shapes
- **Rectangle** (`fill_rect`) — fast memcpy path for opaque fills
- **Rounded rectangle** (`fill_rounded_rect`) — per-corner radii, SDF-based AA
- **Circle** (`fill_circle`) — anti-aliased circle fill
- **Ellipse** (`fill_ellipse`) — axis-aligned ellipses
- **Line** (`draw_line`) — anti-aliased line with configurable width
- **Arc** — circular arc segments
- **Polygon** — arbitrary convex/concave polygons via path rasterizer

### Path Rasterizer
- **Segments**: MoveTo, LineTo, QuadTo (quadratic Bézier), CubicTo (cubic Bézier), Close
- **Fill**: even-odd rule, winding rule, 4× vertical supersampling AA
- **Stroke**: variable width, round/square/butt caps, miter/bevel/round joins
- **Bézier flattening**: adaptive subdivision to configurable tolerance

### Gradients
- **Linear** — two-point gradient with arbitrary color stops
- **Radial** — center + radius with color stops
- **Conic** — sweep around a center point
- **Mesh** — bilinear interpolation across a quad patch grid

### Images
- **Blit** — direct pixel copy with clipping
- **Scaled blit** — bilinear interpolation for arbitrary scaling
- **Nine-patch** — stretchable border images for UI chrome
- **Pattern fill** — tiled/repeated image fills
- **Image decode** — PNG, JPEG, BMP, ICO, WebP format loading

### Blur & Effects
- **Gaussian blur** (`blur_region`) — separable two-pass, σ from kernel radius
- **Fast blur** (`blur_fast`) — downsampled Gaussian for large radii (≥8px)
- **Backdrop blur** — blur region of existing framebuffer, then tint overlay
- **Box shadow** — SDF-based with configurable spread, blur radius, offset, color
- **Inner glow** — inward-facing soft glow effect
- **Drop shadow** — below-surface shadow with offset

### Color
- **sRGB ↔ linear** — lookup table for correct gamma-aware blending
- **Premultiplied alpha** — all blending in premultiplied space
- **Blend modes**: SrcOver (Porter-Duff), Src (direct copy)

## Scene Graph Nodes

The compositor scene graph (`liquide-compositor`) supports these node kinds:

| Node Kind         | Description |
|-------------------|-------------|
| `Root`            | Scene graph root |
| `Background`      | Solid color background |
| `BlurCache`       | Cached blur result |
| `Workspace`       | Virtual desktop container |
| `Surface`         | Application surface with pixel buffer |
| `ChildSurface`    | Nested surface (popups, tooltips) |
| `Shadow`          | Drop shadow effect |
| `Decoration`      | Window title bar and buttons |
| `Overlay`         | Transparent overlay layer |
| `Glass`           | Liquid Glass translucency effect |
| `BlurBackdrop`    | Backdrop blur region |
| `Tint`            | Color tint overlay |
| `Content`         | Arbitrary content region |
| `ShellLayer`      | Shell UI layer (dock, status bar) |
| `Cursor`          | Hardware/software cursor |
| `Text`            | Text rendering node |
| `Icon`            | Vector/raster icon |
| `RenderLayer`     | Isolated render layer with blend mode |
| `ClipPath`        | Arbitrary clip shape |
| `Filter`          | Post-processing filter chain |
| `Image`           | Decoded image content |
| `LockScreen`      | Lock screen overlay |
| `CrashScreen`     | Error recovery screen |

## Damage Tracking

Each frame, the `DamageTracker` computes which tiles changed:

1. Scene graph is flattened to `FlatNode[]` (z-sorted)
2. Frame buffer is divided into tiles (e.g., 64×64 or 128×128)
3. Each tile is hashed with CRC-32C
4. Hashes are compared against previous frame
5. Changed tiles form a `DamageSet` with `DamageClass` priority

### Damage Classes (priority order)
1. `TextGlyph` — text changed (highest priority)
2. `UiPrimitive` — UI element changed
3. `BitmapRegion` — image/surface content changed
4. `CursorOnly` — only cursor moved (lowest priority)

## Effect Budget

The `DegradationController` tracks rendering time and automatically degrades
effects when the frame budget is exceeded:

| Level | Action |
|-------|--------|
| `Full` | All effects enabled |
| `ReduceBlur` | Reduce blur radius by 50% |
| `DisableGlass` | Disable glass transparency |
| `DisableShadows` | Disable drop shadows |
| `MinimalEffects` | Only solid fills and text |

## Glyph Atlas

The `GlyphAtlas` manages a texture atlas for rendered glyphs:

- **Row packing** — glyphs packed in horizontal rows, new row on overflow
- **Subpixel modes**: Grayscale, RGB, BGR, VRGB, VBGR
- **Cache key**: (font_id, glyph_id, size, subpixel_offset)
- **Eviction**: LRU when atlas is full

# LiquiDE — Software Rendering Pipeline & Compositor Contract

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Design Language](spec-design.md) · [Performance](spec-performance.md) · [Client](spec-client.md) · [Protocol](spec-protocol-formal.md)

---

## 1) Purpose

This document specifies the **software rendering pipeline** used by the LiquiDE compositor when no GPU is available (the default and primary path). It defines the scene graph model, rendering primitives, damage tracking granularity, text rasterization behavior, Liquid Glass effect implementation, per-effect budgets, and the deterministic degradation ladder.

The software renderer is the **reference implementation** — when a GPU path exists, it MUST produce visually equivalent output (within perceptual thresholds).

---

## 2) Scene Graph Model

The compositor maintains a scene graph representing the current visual state of the desktop. The scene graph is rebuilt each frame from Wayland surface state + shell UI state.

### 2.1 Node Types

```
SceneRoot
├── BackgroundNode          (wallpaper / solid color / gradient)
│   └── BlurCacheNode       (pre-blurred wallpaper for glass surfaces)
├── WorkspaceNode[0..N]     (one per workspace, only active is visible)
│   ├── SurfaceNode         (Wayland client surface: toplevel, popup, subsurface)
│   │   ├── ShadowNode      (drop shadow behind the surface)
│   │   ├── DecorationNode  (server-side title bar, if applicable)
│   │   └── ChildSurfaceNode[0..N] (subsurfaces, popups)
│   ├── OverlayNode         (transient: tooltips, menus, DnD feedback)
│   └── GlassNode           (glass panel: dock, status bar, notification)
│       ├── BlurBackdropNode (blurred region of what's behind the glass)
│       ├── TintNode         (color tint overlay)
│       └── ContentNode      (text, icons, widgets rendered on the glass)
├── ShellLayer              (zwlr_layer_shell surfaces: dock, bar, launcher)
├── CursorNode              (hardware-style cursor, separate from scene for cursor channel)
├── LockScreenNode          (composited on top of everything when locked)
└── CrashScreenNode         (emergency overlay, software-rendered)
```

### 2.2 Node Properties

Every scene graph node carries:

| Property | Type | Description |
|----------|------|-------------|
| `bounds` | Rect (x, y, w, h) | Position and size in compositor-space pixels |
| `opacity` | f32 (0.0–1.0) | Node opacity (pre-multiplied alpha compositing) |
| `transform` | Affine2D | Translation, rotation, scale |
| `clip` | Option\<Rect\> | Optional clip rectangle |
| `visible` | bool | Visibility flag (invisible nodes skip render and damage) |
| `z_order` | u32 | Stacking order within parent |

### 2.3 Scene Graph Update Cycle

Each frame:

1. **Wayland commit processing** — process pending `wl_surface.commit()` from clients. Update SurfaceNode geometry, buffer, damage.
2. **Shell state update** — apply shell animations (dock hide/show, launcher fade, notification slide).
3. **Effect invalidation** — mark BlurCacheNodes whose backing content changed.
4. **Scene graph flatten** — walk the tree in z-order, compute final bounds and transforms, clip to viewport(s).
5. **Damage computation** — compare with previous frame's flattened scene to identify changed tiles/regions.
6. **Render pass** — composite only damaged regions (see §4).
7. **Encode handoff** — pass damaged regions to the encoding pipeline for Mode A/B/C decisions.

---

## 3) Rendering Primitives

The software rasterizer implements the following primitives:

### 3.1 Primitive Table

| Primitive | SIMD Path | Fallback | Notes |
|-----------|-----------|----------|-------|
| Rect fill (solid) | AVX2: 8 px/cycle, NEON: 4 px/cycle | Scalar: 1 px/cycle | Aligned to cache lines (64B) when possible |
| Rect fill (gradient) | AVX2: 4 px/cycle | Scalar | Linear, radial, conic. Pre-interpolated color ramp. |
| Rounded rect | AVX2: per-scanline mask, blend 8 px | Scalar per-pixel distance | Corner radius: 0 to min(w,h)/2 |
| Circle / ellipse | Same as rounded rect | | |
| Anti-aliased edge | 4× supersampling horizontally | Area coverage | Used for rounded corners, circles, path edges |
| Image blit (opaque) | AVX2: memcpy-optimized, 64B aligned | memcpy | No blending, fastest path |
| Image blit (alpha) | AVX2: premul alpha blend 8 px/cycle | Scalar blend | Porter-Duff src-over |
| Image blit (scaled) | AVX2: bilinear interpolation | Scalar bilinear | Upscale/downscale with filtering |
| Path fill | Scanline rasterizer, AVX2 coverage | Scalar scanline | Used for arbitrary shapes (SVG icons) |
| Path stroke | Offset path → fill | | Uniform/variable width |
| Text glyph blit | AVX2: alpha-only blit (luminance mask) | Scalar | Glyph atlases pre-rasterized by FreeType |
| Box shadow | Cached downsample + blur, blit result | | See §5.2 |
| Backdrop blur | Downsample → separable Gaussian → upsample | | See §5.1 |

### 3.2 Compositing Operations

All compositing uses **premultiplied alpha** (Porter-Duff src-over as default).

Supported blend modes:
- `src-over` — standard alpha compositing (default everywhere)
- `src` — replace destination (used for opaque surface blit)
- `multiply` — used for color tint on glass surfaces
- `screen` — used for specular highlights
- `src-atop` — used for clip-to-shape effects

### 3.3 Color Space & Deep Color Pipeline

The compositor supports three color pipeline modes. The active mode is determined during session startup by negotiating the client's display capabilities (`color.supported_modes` in `ClientHello`) against the server's configuration (`[display.color]`). The default mode is **SDR-sRGB** for backward compatibility; wide gamut and HDR are opt-in.

#### Pipeline Mode Summary

| Pipeline Mode | Internal Precision | Linearization | Compositing Gamut | Output Bit Depth | Output Transfer | Tile Pixel Format |
|--------------|-------------------|---------------|-------------------|------------------|-----------------|-------------------|
| **SDR-sRGB** (default) | 8-bit per channel | 256-entry LUT | sRGB / BT.709 | 8 bpc | sRGB gamma | `rgb888`, `rgba8888` |
| **WCG-SDR** | 16-bit or float32 | 1024-entry LUT or analytical | Display-P3 or Rec.2020 | 10 bpc | sRGB gamma | `rgb101010`, `rgba1010102` |
| **HDR** | float32 | Analytical (exact) | Rec.2020 | 10 or 16 bpc | PQ (ST 2084) or HLG | `rgb101010`, `rgba1010102`, `rgba16161616` |

#### SDR-sRGB Mode (Default)

- Internal compositing operates in **linear sRGB** for correct alpha blending.
- Input surfaces are assumed sRGB (gamma ≈ 2.2). Conversion: sRGB → linear on surface upload, linear → sRGB on output.
- The sRGB linearization uses a **256-entry lookup table** (LUT) for performance — the standard sRGB piecewise transfer function is precomputed at startup.
- Output framebuffer is sRGB, 8 bits per channel. The encode pipeline receives sRGB pixel data.
- This is the lowest-cost path and the only mode guaranteed on all hardware.

#### WCG-SDR Mode (Wide Color Gamut, SDR Output)

- Activated when the client advertises `color.supported_modes` containing `"wcg-sdr"` and the server has `display.color.pipeline_mode = "wcg-sdr"`.
- Internal compositing operates in **linear Display-P3 or Rec.2020** (configurable via `display.color.compositing_gamut`). Default is Display-P3.
- sRGB input surfaces are converted to the wider gamut via a **3×3 matrix transform** (sRGB → linear → gamut matrix → compositing space). sRGB is a strict subset of Display-P3 and Rec.2020, so this mapping is lossless.
- Surfaces tagged with `wp_color_management_v1` ICC profiles or color space descriptors are converted to the compositing gamut using the appropriate 3×3 chromatic adaptation matrix.
- Linearization uses a **1024-entry LUT** for the sRGB transfer function (higher precision to preserve 10-bit output fidelity) or an **analytical piecewise function** (configurable — analytical is slower but exact).
- Output framebuffer is 10 bits per channel (`rgb101010` or `rgba1010102`). The encode pipeline receives 10-bit pixel data; codecs must use 10-bit profiles (H.265 Main 10, AV1 10-bit, VP9 Profile 2).
- Output transfer function is sRGB gamma — the wider gamut is used for color accuracy, not luminance extension.

#### HDR Mode (High Dynamic Range)

- Activated when the client advertises `color.supported_modes` containing `"hdr"`, the server has `display.color.pipeline_mode = "hdr"`, and the client display supports HDR output (`color.display_hdr = true`).
- Internal compositing operates in **linear Rec.2020** at **float32 precision** to avoid banding in the extended luminance range.
- Linearization is always **analytical** (exact inverse PQ or HLG) — LUT approximation is insufficient for the non-linear PQ curve.
- HDR content surfaces provide scene-referred linear light values. SDR content surfaces are inverse-tone-mapped to the HDR luminance range using a configurable lift (default: SDR white = 203 nits, per ITU-R BT.2408).
- Output transfer function is **PQ (Perceptual Quantizer, SMPTE ST 2084)** or **HLG (Hybrid Log-Gamma, ARIB STD-B67)**, configurable via `display.color.hdr_transfer_function`.
- Output bit depth is 10 or 16 bits per channel (configurable via `display.color.compositing_bit_depth`; default 10 for PQ, 16 available for mastering workflows).
- **HDR metadata passthrough**: the compositor attaches per-frame `HDRMetadata` to `FrameHeader` messages (see [spec-protocol-formal.md](spec-protocol-formal.md) §8.4). HDR10 static metadata (SMPTE ST 2086 mastering display primaries + MaxCLL/MaxFALL) is sent once at stream start and on change. HDR10+ dynamic metadata is passed through per-frame when available.

#### Gamut Mapping

- **Narrower → wider** (sRGB surface in P3/Rec.2020 compositing space): lossless — sRGB is an exact colorimetric subset.
- **Wider → narrower** (P3 surface in sRGB compositing space, or HDR → SDR fallback): requires gamut compression. The compositor uses **relative colorimetric intent** with **soft-knee gamut compression** on the chroma axis (preserves hue, compresses saturation for out-of-gamut colors). No clipping — out-of-gamut colors are smoothly compressed.

#### Tone Mapping (HDR → SDR Fallback)

When a session runs in HDR mode but must produce SDR output (e.g., for a client that doesn't support HDR, or for recording/screenshots), the compositor applies a tone mapping operator (TMO):

| Operator | ID | Description | Use Case |
|----------|-----|-------------|----------|
| **Reinhard** (default) | `reinhard` | Simple global TMO: `L_out = L_in / (1 + L_in)`. Fast, preserves overall luminance relationships. | General fallback, low CPU cost |
| **BT.2390 EETF** | `bt2390` | ITU-R BT.2390 Electrical-Electrical Transfer Function. Broadcast-standard knee function. | Broadcast content, standards compliance |
| **Hable (Filmic)** | `hable` | John Hable's filmic curve (Uncharted 2). S-shaped, good highlight rolloff. | Cinematic content, photo editing |
| **ACES** | `aces` | Academy Color Encoding System RRT+ODT. Industry-standard, full gamut mapping included. | Professional color grading, mastering |

The TMO is configurable via `display.color.tone_map_operator`. Default is `reinhard` for its low CPU cost. TMO selection does NOT affect the encoding pipeline when HDR output is active — it only applies when HDR content must be displayed on an SDR output path.

#### Color Pipeline Diagram

```
Surface Upload                     Compositing                           Output
─────────────                     ───────────                           ──────
                                                                    ┌─ SDR-sRGB ──── 8-bit sRGB ──── rgb888
                    linearize        blend/effects      output TF   │
[sRGB surface] ──── (LUT/analytical) ──► linear RGB ──► composite ──┤─ WCG-SDR ──── 10-bit sRGB ──── rgb101010
                                         (gamut)                    │  (P3/2020)      gamma
[WCG surface] ──── gamut matrix ────────►                           │
                                                                    └─ HDR ────────── 10/16-bit PQ ── rgb101010
[HDR surface] ──── inverse PQ ──────────►                              (Rec.2020)     or HLG           rgba16161616
```

---

## 4) Damage Tracking

Damage tracking is the single most important optimization for remote desktop rendering. The compositor tracks damage at three granularities:

### 4.1 Surface-Level Damage

- Each Wayland client reports damage via `wl_surface.damage_buffer()`.
- The compositor converts client-reported damage from buffer-space to compositor-space.
- Surface damage is **unioned** across all surfaces that overlap a given region.
- Surfaces that did not commit are assumed undamaged.

### 4.2 Tile-Level Damage

- The compositor output framebuffer is divided into tiles (default 64×64 px, matching Mode B tile size).
- After compositing, each tile is compared to its previous-frame counterpart:
  - **CRC-32C hash comparison** (fast, 4 bytes per tile).
  - If hash matches → tile is undamaged → skip encode.
  - If hash differs → tile is damaged → encode and send.
- Tile damage is the **authoritative** damage signal for the encode pipeline.

### 4.3 Pixel-Level Damage (XOR Delta)

- For tiles that are damaged, the Mode B encoder performs pixel-level XOR to determine the optimal encoding strategy (see spec.md §8 Mode B).
- This is not a separate damage pass — it is integrated into the tile encoder.

### 4.4 Damage Optimization Rules

| Rule | Description |
|------|-------------|
| **Coalescing** | Multiple small damage rects in the same tile are coalesced (the entire tile is re-rendered). Below tile granularity, there is no benefit to tracking smaller damage. |
| **Expansion** | Damage caused by window movement includes both the old and new positions (the vacated area shows the background, which needs re-rendering). |
| **Glass propagation** | If a surface behind a glass panel changes, the glass panel's blur cache is invalidated → the glass tile is marked damaged too. |
| **Cursor exclusion** | Cursor movement does not generate tile damage (cursor is a separate channel). The cursor is composited into the cursor channel, not the scene framebuffer. |
| **Animation batching** | Shell animations (fade, slide) generate damage every frame for their duration. The compositor batches these into a single damage rect per animation. |
| **Overdraw prevention** | When multiple overlapping surfaces are damaged, the compositor composites from back to front but only within the damaged region. Undamaged pixels are never touched. |

---

## 5) Liquid Glass Effect Implementation

### 5.1 Backdrop Blur

The signature visual effect of Liquid Glass. Implementation for software rendering:

#### Algorithm: Dual-pass Separable Gaussian Blur

```
1. Extract backdrop region (the pixels behind the glass surface)
2. Downsample to 1/DS resolution (DS = downsample_factor, default: 4)
   - Method: area-average downsampling via SIMD
3. Horizontal Gaussian blur pass
   - Kernel radius = ceil(blur_radius / DS)
   - SIMD: AVX2 processes 8 pixels per iteration (symmetric kernel, half-kernel optimization)
4. Vertical Gaussian blur pass
   - Same kernel radius
   - Memory access is column-strided → cache-unfriendly
   - Optimization: process in 8×8 blocks, transpose, horizontal pass, transpose back
5. Upsample to original resolution (bilinear interpolation)
6. Composite blurred backdrop behind glass surface content
```

#### Performance Characteristics

| Resolution | Blur Radius | Downsample | CPU Time (AVX2, single core) | CPU Time (NEON, single core) |
|-----------|-------------|------------|------|------|
| 400×400 (panel) | 20px | 4× | ~0.3ms | ~0.5ms |
| 400×400 (panel) | 40px | 8× | ~0.2ms | ~0.3ms |
| 1920×40 (status bar) | 20px | 4× | ~0.4ms | ~0.6ms |
| 1920×1080 (full-screen, e.g., login) | 40px | 16× | ~1.5ms | ~2.5ms |
| 3840×2160 (4K full-screen) | 40px | 16× | ~4.0ms | ~6.0ms |

#### Blur Quality Levels

| Level | Downsample | Kernel | Visual Quality | CPU Cost |
|-------|-----------|--------|---------------|----------|
| `quality` | 2× | Full Gaussian | Near-perfect glass, subtle bokeh | High |
| `balanced` | 4× | Full Gaussian | Good glass, minimal artifacts | Medium |
| `performance` | 8× | Full Gaussian | Visible softness, acceptable | Low |
| `minimal` | 16× | Box blur (3-tap) | Noticeable banding, still glass-like | Very low |
| `disabled` | N/A | No blur | Solid tinted panel | Zero |

### 5.2 Box Shadows

Drop shadows behind windows, panels, and elevated surfaces.

#### Algorithm

```
1. Generate shadow shape: expand surface bounds by shadow spread
2. Apply corner radius to shadow shape (rounded-rect SDF)
3. Downsample shadow shape to 1/4 resolution
4. Apply Gaussian blur (same separable pass as backdrop blur)
5. Upsample and multiply by shadow color + opacity
6. Cache the result (shadow shape + blur) keyed by (width, height, radius, spread, blur_radius)
```

#### Shadow Caching

- Shadows are **geometry-dependent only** — they do not depend on background content.
- A shadow is recomputed only when the surface geometry changes (resize, corner radius change).
- Identical shadow shapes (same dimensions + radius + parameters) share a single cached texture.
- Cache eviction: LRU, max 64 shadow textures per session (configurable).

### 5.3 Inner Glow / Specular Highlights

- Thin bright border on glass surfaces simulating light reflection.
- Implementation: 1–2px inset stroke with gradient opacity (brightest at top, fading at bottom).
- Uses `screen` blend mode for additive light effect.
- CPU cost: negligible (single scanline stroke).

### 5.4 Translucency / Tint

- Glass surfaces have a base tint color applied via `multiply` blend.
- Tint is applied after backdrop blur, before content compositing.
- CSS-controlled: `--liquid-surface` defines the tint RGBA.
- No additional CPU cost beyond a single blended rect fill.

### 5.5 Depth / Parallax

- Subtle parallax effect: glass panels shift slightly when windows move behind them.
- Implementation: offset the blur sample region by a fraction of the background movement vector.
- This piggybacks on the blur re-computation that already happens when background changes.
- Parallax is **disabled by default** on software rendering. Enabled only with `effects.parallax = true`.

---

## 6) Per-Effect Budget Contract

Every visual effect has a defined maximum time budget. The compositor enforces these budgets and degrades effects that exceed them.

### 6.1 Effect Budget Table

| Effect | Max Budget (per frame) | Max Instances | Max Radius/Size | Caching | Fallback |
|--------|----------------------|---------------|-----------------|---------|----------|
| Backdrop blur | 4ms | 8 simultaneous | 60px (pre-downsample) | Yes: per-surface, invalidated on background change | Solid tint panel |
| Box shadow | 1ms | 32 surfaces | spread: 30px, blur: 30px | Yes: per-geometry, LRU 64 | No shadow (flat surface) |
| Inner glow | 0.2ms | 16 surfaces | width: 2px | No (trivial cost) | No glow |
| Animation (shell transition) | 2ms | 4 concurrent | 300ms duration | No | Instant transition (no animation) |
| Rounded corners (AA) | 0.5ms | 32 surfaces | radius: 999px | Mask cached per geometry | Square corners |
| Text rendering | 3ms | — | — | Glyph atlas cached | — (text is never degraded) |
| Wallpaper blur | 5ms (one-shot, cached) | 1 | 60px | Yes: recomputed on change only | Solid color desktop |
| Gradient fill | 0.3ms | 16 | — | Color ramp LUT cached | Solid average color |

### 6.2 Total Frame Budget

| Profile | Total Effects Budget | Total Frame Budget (composite + effects + handoff) | Target FPS |
|---------|---------------------|------------------------------------------------------|------------|
| `quality` | 10ms | 16ms (60 fps) | 60 |
| `balanced` | 6ms | 12ms (target 60 fps, may reduce to 30 under load) | 30–60 |
| `performance` | 3ms | 8ms (60 fps if CPU allows, else 30 fps) | 30–60 |
| `minimal` | 1ms | 5ms | 30 |
| `bandwidth_saver` | 0.5ms | 5ms | 15–30 |

### 6.3 Budget Enforcement

The compositor measures actual time spent per effect per frame using `CLOCK_MONOTONIC` timestamps.

1. **Pre-frame planning**: before rendering, the compositor estimates the total effect cost based on visible glass surfaces, shadow counts, and animation state.
2. **Fast path**: if estimated cost < 50% of budget, render all effects at full quality.
3. **Budget pressure**: if estimated or measured cost approaches the budget, effects are degraded per the degradation ladder (§7).
4. **Hard cutoff**: if any single effect exceeds 2× its per-effect budget, it is disabled for the remainder of the frame and a metric is emitted.
5. **Frame skip**: if the total frame takes > 2× the frame budget (e.g., >32ms at 60fps), the next frame is skipped (FPS drop) rather than falling further behind.

---

## 7) Deterministic Degradation Ladder

When the compositor cannot maintain the target frame rate at full visual quality, effects are degraded in a **fixed, deterministic order**. This ensures predictable behavior — the same hardware always produces the same visual degradation.

### 7.1 Degradation Steps

The steps are applied in order from top to bottom. Each step is triggered when the previous step is insufficient to meet the frame budget.

| Step | Trigger | Action | Visual Impact | Bandwidth Impact |
|------|---------|--------|---------------|-----------------|
| **L0** | Nominal | All effects at configured quality level | Full Liquid Glass | Baseline |
| **L1** | Frame time > budget × 1.1 for 3 consecutive frames | Increase blur downsample by 1 level (e.g., 4× → 8×) | Slightly softer blur | -10% (fewer blur pixels) |
| **L2** | Still over budget | Disable parallax effect | No depth shift on glass | Negligible |
| **L3** | Still over budget | Reduce shadow blur radius by 50% | Sharper, smaller shadows | -5% |
| **L4** | Still over budget | Disable inner glow on non-focused surfaces | Focused window retains glow, others flat | Negligible |
| **L5** | Still over budget | Reduce max concurrent backdrop blurs to 4 (background panels lose blur) | Some panels become solid tint | -15% |
| **L6** | Still over budget | Increase blur downsample by 1 more level (e.g., 8× → 16×) | Visibly soft blur | -10% |
| **L7** | Still over budget | Disable all animations (instant transitions) | No motion, instant state changes | -5% (no animation frames) |
| **L8** | Still over budget | Replace Gaussian blur with box blur (3-tap) | Visible banding in blur | -5% |
| **L9** | Still over budget | Disable all backdrop blur (solid tint panels) | No glass transparency — flat colored panels | -20% |
| **L10** | Still over budget | Disable all shadows | Flat appearance, no depth | -10% |
| **L11** | Still over budget | Reduce target FPS to 30 (from 60) | Choppy motion | -50% |
| **L12** | Still over budget at 30fps | Reduce target FPS to 15 | Very choppy | -50% |
| **L13** | Still over budget at 15fps | Emergency: compositor renders at best-effort rate, no effects | Raw surface compositing only | Minimal |

### 7.2 Recovery (Ascending)

When the frame budget has headroom (frame time < budget × 0.7 for 10 consecutive frames), the compositor ascends one step. Recovery is **slower than degradation** to avoid oscillation.

### 7.3 Degradation State Reporting

- The current degradation level is exposed via:
  - `liquidctl session status` → shows `degradation_level: L3`
  - Stream analysis overlay (client-side) → shows current level
  - Metrics: `compositor_degradation_level` gauge (see spec-observability.md)
  - Notification: when descending past L5, a toast notification is shown: "Visual effects reduced to maintain performance"
- Policy can enforce a **minimum degradation level** (e.g., `rendering.max_quality = "L3"` forces at least L3 degradations on low-resource servers).
- Policy can enforce a **maximum degradation level** (e.g., `rendering.min_quality = "L7"` prevents degradation past L7 — if the system can't maintain L7, FPS drops instead).

### 7.4 Degradation Hysteresis

To prevent rapid oscillation between levels:

| Direction | Condition | Delay |
|-----------|-----------|-------|
| Descend (degrade) | Frame time > threshold for 3 consecutive frames | Immediate after 3-frame confirmation |
| Ascend (restore) | Frame time < budget × 0.7 for 10 consecutive frames | 1 step per 10-frame window |

---

## 8) Text Rasterization Contract

Text rendering has special requirements for remote desktop — text must be sharp at all scales and survive encoding.

### 8.1 Glyph Rasterization

- **FreeType** is used with **light autohinting** by default (best balance of sharpness and fidelity).
- Hinting modes available: `none`, `light`, `medium`, `full`. Configurable per-session.
- Glyph bitmaps are cached in a **texture atlas** (1024×1024 px minimum, grows as needed).
- Atlas is organized by (font_face, size_px, hinting_mode, subpixel_mode). Same glyph at different sizes gets separate atlas entries.

### 8.2 Subpixel Rendering

| Mode | Description | When Used |
|------|-------------|-----------|
| `none` (grayscale AA) | Standard anti-aliasing, no subpixel | Default for remote (safest through codecs) |
| `rgb` | RGB subpixel rendering | When client reports LCD-RGB and Mode B/C is used |
| `bgr` | BGR subpixel rendering | When client reports LCD-BGR |
| `vrgb` / `vbgr` | Vertical subpixel | Rare, for rotated displays |

- **Default**: `none` (grayscale) for video-encoded content (Mode A). Subpixel rendering produces colored fringing when passed through lossy video codecs.
- **Exception**: when Mode B (tile/bitmap, lossless) or Mode C (client-side render) is active, subpixel rendering MAY be enabled if the client reports its subpixel layout.

### 8.3 Font Metrics Contract

- DPI-aware rendering: compositor queries the virtual monitor's configured DPI.
- Font size in CSS `px` units maps to physical pixels via: `physical_px = css_px × (dpi / 96.0)`.
- Fractional scaling is handled by rendering at the scaled resolution (e.g., 150% scale → render at 1.5× and downsample or render natively).
- **Text is never degraded** by the degradation ladder — text rendering budget is protected even at L13.

---

## 9) Frame Scheduling

### 9.1 Frame Pacing Model

```
  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
  │ vsync 0 │    │ vsync 1 │    │ vsync 2 │    │ vsync 3 │
  └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘
       │              │              │              │
  ┌────▼────┐    ┌────▼────┐    ┌────▼────┐         │
  │  Commit │    │  Commit │    │  Commit │    (no damage
  │ Process │    │ Process │    │ Process │     → no frame)
  │ + Render│    │ + Render│    │ + Render│
  │ + Encode│    │ + Encode│    │ + Encode│
  └────┬────┘    └────┬────┘    └────┬────┘
       │              │              │
  ┌────▼────┐    ┌────▼────┐    ┌────▼────┐
  │  Send   │    │  Send   │    │  Send   │
  └─────────┘    └─────────┘    └─────────┘
```

### 9.2 Scheduling Rules

| Rule | Description |
|------|-------------|
| **No damage, no frame** | If no surface committed since last frame, no render/encode cycle runs. CPU usage drops to near zero. |
| **Single writer** | Only one render thread writes to the output framebuffer at a time (no concurrent writes to the same tile region). |
| **Pipeline overlap** | While frame N is being encoded, frame N+1 can begin compositing into a second framebuffer (double-buffered). |
| **Deadline-driven** | If compositing exceeds the frame deadline, the compositor finishes the current frame (no tearing) but skips the next frame. |
| **Input boost** | On input event arrival, if the compositor is idle, a frame is scheduled immediately (input-to-photon fast path). |
| **Batch commits** | Multiple `wl_surface.commit()` calls are batched within a 1ms window before triggering a frame. |

### 9.3 Adaptive Frame Rate

| Condition | Target FPS |
|-----------|-----------|
| Active interaction (input events) | 60 fps (or max configured) |
| Animation playing (shell transition) | 60 fps for animation duration |
| Static content, cursor moving | 30–60 fps (cursor channel carries cursor, video may be lower) |
| Idle (no damage) | 0 fps (nothing sent) |
| Video playback detected | Match source frame rate (24/25/30/60) |

---

## 10) Software Renderer Configuration

```toml
[rendering]
# GPU usage: "auto" (use GPU if available), "cpu" (force CPU), "gpu" (require GPU, fail if unavailable)
gpu_mode = "auto"

# Effect quality profile: "quality", "balanced", "performance", "minimal", "bandwidth_saver"
profile = "balanced"

# Per-effect overrides (override the profile defaults)
[rendering.effects]
blur_enabled = true
blur_quality = "balanced"          # quality, balanced, performance, minimal, disabled
blur_max_radius = 40               # px, pre-downsample
shadow_enabled = true
shadow_max_blur = 20               # px
shadow_max_spread = 20             # px
animation_enabled = true
animation_max_duration_ms = 300
parallax_enabled = false           # disabled by default in software mode
inner_glow_enabled = true
rounded_corners = true

# Frame scheduling
[rendering.frame]
target_fps = 60                    # maximum target FPS
min_fps = 15                       # FPS floor (below this, compositor logs a warning)
double_buffer = true               # double-buffered compositing
input_boost = true                 # immediate frame on input event

# Degradation policy
[rendering.degradation]
enabled = true                     # enable automatic degradation ladder
max_quality = "L0"                 # highest quality allowed (e.g., "L3" to start degraded)
min_quality = "L13"                # lowest degradation allowed (e.g., "L7" to prevent full degradation)
descend_threshold_frames = 3       # consecutive over-budget frames before descending
ascend_threshold_frames = 10       # consecutive under-budget frames before ascending
```

---

## 11) Test Plan

### Rendering Correctness
- Software renderer output matches reference images (pixel-level comparison with ±1 tolerance per channel for AA).
- All blend modes produce correct output against known test patterns.
- Damage tracking: verify that undamaged tiles are never re-sent after surface operations.
- Glass propagation: verify that moving a window behind a glass panel triggers glass re-render.
- Cursor exclusion: verify cursor movement does not trigger tile damage.
- Text at all hinting modes and sizes matches FreeType reference output.
- Subpixel rendering produces correct RGB/BGR fringe pattern.
- sRGB linearization round-trip: verify (sRGB → linear → compositing → sRGB) produces no drift.

### Degradation Ladder
- Force L0–L13 via config; verify visual output matches expected degradation at each level.
- Simulate CPU overload; verify automatic descent follows the deterministic order.
- Simulate load relief; verify ascent is slower than descent (hysteresis).
- Verify notification at L5+ degradation.
- Verify `max_quality` and `min_quality` policy limits are respected.
- Verify `liquidctl session status` reports correct degradation level.

### Effect Budgets
- Measure each effect against its budget on reference hardware (8-core, AVX2).
- Verify hard cutoff disables effects that exceed 2× budget.
- Verify frame skip on 2× total frame budget overrun.
- Verify text rendering is never degraded even at L13.

### Performance
- Verify idle CPU < 1% when no damage.
- Verify input-to-photon latency < 16ms on reference hardware (single monitor, 1080p).
- Verify 4K rendering meets frame budget at `balanced` profile on 8-core machine.
- Benchmark blur performance matches the table in §5.1 (±25% tolerance).

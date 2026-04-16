# `liquide-renderer-wgpu` — Implementation Plan

**Crate**: `liquide-renderer-wgpu` (3,230 LOC, 0 tests, 8 TODOs, 71 unsafe blocks)  
**Reference**: `liquide-renderer-cpu` (16,881 LOC, 172 tests)  
**See also**: [Production readiness overview](production-readiness.md#liquide-renderer-wgpu)

---

## Table of Contents

1. [Current State](#1-current-state)
2. [Implementation Phases](#2-implementation-phases)
   - [Phase 1: Background / Tint / Surface](#phase-1-background--tint--surface-dispatch)
   - [Phase 2: Shadow / Gradient](#phase-2-shadow--gradient-dispatch)
   - [Phase 3: Filter / Blur / Blend](#phase-3-filter--blur--blend-dispatch)
   - [Phase 4: Tests](#phase-4-test-infrastructure)
   - [Phase 5: Cleanup](#phase-5-cleanup--polish)
3. [Summary](#3-summary)
4. [Risk Assessment](#4-risk-assessment)
5. [Fallback Strategy](#5-fallback-strategy)

---

## 1. Current State

### What Works

- **Device initialization** (`device.rs`): Full `WgpuDevice` with backend selection, adapter enumeration.
- **Pipeline compilation** (`pipeline.rs`): All 7 pipelines compile — `rect`, `blur`, `blend` (compute), `gradient`, `shadow`, `text`, `image`.
- **Shader code** (`shader.rs`): WGSL shaders for all pipeline types. 10/16 CSS blend modes implemented; SoftLight and non-separable modes fall back to SrcOver.
- **Text rendering** (`renderer.rs:656-812`): Full glyph atlas + per-glyph textured quad pipeline.
- **Image rendering** (`renderer.rs:814-981`): Full image texture cache + UV fit modes (Fill/Contain/Cover/None).
- **Texture management** (`texture.rs`): `GpuTexture` creation, upload, readback.
- **Vulkan DMA-BUF export** (`vulkan_export.rs`): 71 unsafe blocks, all justified FFI with SAFETY comments. **Do not touch.**
- **Readback** (`renderer.rs:983-1020`): GPU→CPU readback via staging buffer.
- **Render-to-framebuffer** (`renderer.rs:1023-1041`): Bridges GPU render to CPU `FrameBuffer`.

### Broken: 6 Stub Node Types

| # | Node Type(s) | Line | Pipeline Available | Status |
|---|-------------|------|-------------------|--------|
| 1 | `Background`, `Tint`, `Glass` | 574-578 | `rect_pipeline` | `draw_calls += 1` only |
| 2 | `Shadow`, `BoxShadows` | 579-582 | `shadow_pipeline` | `draw_calls += 1` only |
| 3 | `GradientFill` | 583-586 | `gradient_pipeline` | `draw_calls += 1` only |
| 4 | `Filter`, `BackdropFilter` | 587-590 | `blur_pipeline` + compute | `draw_calls += 1` only |
| 5 | `RenderLayer` | 591-594 | `blend_pipeline` (compute) | `draw_calls += 1` only |
| 6 | `Surface`, `ChildSurface` | 595-601 | `image_pipeline` (reuse) | `draw_calls += 1` only |

### Structural Issue: Missing Bind Group Layouts

`PipelineCache::new()` creates bind group layouts for `rect`, `blur`, `blend`, `gradient`, `shadow` as **local variables** that are dropped after pipeline creation. Only `text_bind_group_layout`, `quad_bind_group_layout`, and `image_bind_group_layout` are retained.

**Fix**: Store all bind group layouts on `PipelineCache`.

### `render_frame_filtered()` Also Stubbed

`renderer.rs:1071-1112`: The damage-aware path just does `draw_calls += 1` without dispatch.

---

## 2. Implementation Phases

### Phase 1: Background / Tint / Surface Dispatch

**Goal**: Render the most common visual elements — solid color backgrounds, color tints, and client surface blits.

#### 1A. Store bind group layouts on `PipelineCache`

Add fields:
```rust
pub rect_bind_group_layout: wgpu::BindGroupLayout,
pub shadow_bind_group_layout: wgpu::BindGroupLayout,
pub gradient_bind_group_layout: wgpu::BindGroupLayout,
pub gradient_stops_bind_group_layout: wgpu::BindGroupLayout,
pub blur_bind_group_layout: wgpu::BindGroupLayout,
pub blend_bind_group_layout: wgpu::BindGroupLayout,
```

Change local `let` bindings to struct field assignments. ~20 LOC, zero risk.

#### 1B. `render_background_node()`

```rust
fn render_background_node(
    &self, encoder: &mut wgpu::CommandEncoder,
    output: &GpuTexture, node: &FlatNode, color: &Color,
) -> u32
```

Steps:
1. Convert `Color` to premultiplied `[f32; 4]` RGBA, apply `node.opacity`.
2. Create `RectUniforms` buffer: `{ color, bounds, corner_radius, opacity }`.
3. Create bind group with `rect_bind_group_layout`.
4. Render pass with `LoadOp::Load`, set `rect_pipeline`, scissor to `node.absolute_bounds`.

**Decision**: Reuse `TEXTURED_QUAD_VERT` for all per-node rendering (same as text/image). This makes the architecture uniform.

Uniform struct:
```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct RectUniforms {
    color: [f32; 4],
    bounds: [f32; 4],      // x, y, w, h in pixels
    corner_radius: f32,
    opacity: f32,
    _pad: [f32; 2],
}
```

~100 LOC.

#### 1C. `render_tint_node()`

Same as background but with Multiply blend state:
```rust
blend: Some(wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Dst,
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::OVER,
})
```

Add a `tint_pipeline` to `PipelineCache`. ~40 LOC.

#### 1D. `render_glass_node()` (partial)

Phase 1: Tint-only (skip backdrop blur). Phase 2 adds real blur. ~15 LOC.

#### 1E. `render_surface_node()`

Upload `SurfaceBuffer.pixels` to GPU texture (keyed by `surface_id`), render via `image_pipeline`. Re-upload every frame; optimize later. ~60 LOC.

**Phase 1 total**: ~235 LOC. Risk: Low.

---

### Phase 2: Shadow / Gradient Dispatch

#### 2A. `render_shadow_node()`

The `BOX_SHADOW_FRAG` shader already implements SDF rounded rect + spread + blur falloff. Implementation:
1. Expand bounds by `blur_radius + spread`.
2. Create `ShadowUniforms`: bounds, color, offset, blur, spread, radius, inset flag.
3. Use `shadow_pipeline` with quad vert + scissor.

~60 LOC.

#### 2B. `render_box_shadows_node()`

Iterate `Vec<BoxShadowSpec>`, dispatch shadow per entry. ~40 LOC.

#### 2C. `render_gradient_node()`

The `GRADIENT_FRAG` shader handles linear/radial/conic. Implementation:
1. Map `GradientSpec` to `GradientUniforms` (kind + parameters).
2. Upload color stops to a GPU storage buffer.
3. Create bind group (uniform at binding 0, storage at binding 1).

Add SDF corner radius mask to gradient shader (~10 lines WGSL).

**Alignment note**: `GradientStop { position: f32, _pad: [f32; 3], color: [f32; 4] }` = 32 bytes for 16-byte alignment.

~120 LOC. Risk: Medium (storage buffer alignment).

**Phase 2 total**: ~220 LOC.

---

### Phase 3: Filter / Blur / Blend Dispatch

#### 3A. Filter Pipeline (Color Transforms)

New `FILTER_COMPUTE` shader for per-pixel color matrix operations (brightness, contrast, saturate, hue-rotate, grayscale, sepia, invert, opacity).

All represented as 4x4 color matrices:

| Filter | Matrix |
|--------|--------|
| Brightness(b) | `diag(b, b, b, 1)` |
| Contrast(c) | `diag(c, c, c, 1)` + translate `(0.5*(1-c))` |
| Grayscale | Saturate(0) |
| Sepia | Fixed sepia matrix |

**Challenge**: Compute shaders cannot read and write the same texture simultaneously. Pattern: copy output → temp, apply filter: read temp → write output.

~200 LOC.

#### 3B. BackdropFilter

Same as Filter but operates on already-rendered backdrop pixels. Shares code with 3A. ~80 LOC.

#### 3C. Blur (Multi-Pass)

Existing `blur_pipeline` + `BLUR_FRAG` shader, dispatched as two passes:
1. Horizontal: read output → blur `direction=(1,0)` → write temp.
2. Vertical: read temp → blur `direction=(0,1)` → write output.

Requires one scratch texture (allocated once, reused). ~80 LOC.

#### 3D. RenderLayer (Blend Mode Compositing)

Track current blend mode. For non-SrcOver modes, use compute `blend_pipeline`. ~100 LOC.

#### 3E. Complete Blend Mode Shaders

**SoftLight** (case 11):
```wgsl
fn blend_soft_light(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return select(
        d - (1.0 - 2.0 * s) * d * (1.0 - d),
        d + (2.0 * s - 1.0) * (sqrt(d) - d),
        s > vec3<f32>(0.5)
    );
}
```

**Non-separable modes** (Hue, Saturation, Color, Luminosity): HSL conversion helpers. ~80 LOC WGSL.

**Phase 3 total**: ~540 LOC. Risk: High.

---

### Phase 4: Test Infrastructure

#### 4A. Headless GPU Device

```rust
async fn test_device() -> WgpuDevice {
    WgpuDevice::new(None).await
        .expect("test requires wgpu-compatible device")
}

async fn render_nodes(nodes: &[FlatNode], width: u32, height: u32) -> Vec<u8> {
    let gpu = test_device().await;
    let mut renderer = WgpuRenderer::new(gpu, width, height).unwrap();
    renderer.render_frame(nodes).unwrap();
    renderer.read_back().unwrap()
}
```

~60 LOC.

#### 4B. Unit Tests (22)

| Test | Validates |
|------|-----------|
| `test_clear_to_black` | Render loop baseline |
| `test_background_solid` | Rect pipeline dispatch |
| `test_background_with_radius` | SDF shader |
| `test_background_opacity` | Alpha blending |
| `test_tint_multiply` | Multiply blend |
| `test_surface_blit` | Surface dispatch |
| `test_shadow_basic` | Shadow pipeline |
| `test_shadow_inset` | Inset flag |
| `test_gradient_linear` | Gradient pipeline |
| `test_gradient_radial` | Radial falloff |
| `test_gradient_conic` | Angular sweep |
| `test_filter_brightness` | Filter compute |
| `test_filter_blur` | Blur pipeline |
| `test_filter_grayscale` | Color matrix |
| `test_blend_multiply` | Blend compute |
| `test_blend_screen` | Blend compute |
| `test_render_layer_isolate` | RenderLayer |
| `test_image_fit_cover` | UV computation |
| `test_text_glyph_atlas` | Text pipeline |
| `test_resize` | Texture recreation |
| `test_damage_filtering` | Damage-aware path |
| `test_read_back` | Readback correctness |

~400 LOC.

#### 4C. Visual Parity Tests (5)

Compare wgpu vs CPU renderer output with tolerance (±2 per channel, max 5% failing pixels):

| Test | Scene |
|------|-------|
| `parity_solid_background` | Full-viewport solid red |
| `parity_rounded_rect` | 200x100 rect with 10px corners |
| `parity_gradient_linear` | Linear gradient red→blue |
| `parity_shadow` | Shadow with 10px blur |
| `parity_opacity_composite` | Two overlapping rects at 50% opacity |

~180 LOC.

#### 4D. Render-to-Framebuffer Round-Trip

~30 LOC.

**Phase 4 total**: ~670 LOC (27 tests). Risk: Low.

---

### Phase 5: Cleanup / Polish

#### 5A. Wire `render_frame_filtered()`

Extract node dispatch into a shared helper:
```rust
fn dispatch_node(&self, encoder: &mut wgpu::CommandEncoder, output: &GpuTexture, node: &FlatNode) -> u32
```

Both `render_frame()` and `render_frame_filtered()` call it. ~30 LOC.

#### 5B. Remaining `SceneNodeKind` Variants

- `Decoration` → rect + text pipelines
- `BlurBackdrop` / `BlurCache` → blur pipeline
- `Content` / `Overlay` / `ShellLayer` → filter pass (Opacity)
- `Cursor` → image pipeline
- `LockScreen` → blur + rect
- `CrashScreen` → red rect
- `SvgPath` → deferred (requires path tessellation)

~200 LOC.

**Phase 5 total**: ~230 LOC.

---

## 3. Summary

| Phase | Scope | LOC | Risk |
|-------|-------|-----|------|
| 1 | Background / Tint / Surface | ~235 | Low |
| 2 | Shadow / Gradient | ~220 | Medium |
| 3 | Filter / Blur / Blend / RenderLayer | ~540 | **High** |
| 4 | Tests (22 unit + 5 parity) | ~670 | Low |
| 5 | Cleanup / remaining node types | ~230 | Low |
| **Total** | | **~1,895** | |

---

## 4. Risk Assessment

### High Risk

1. **Filter compute read/write conflicts** (Phase 3A): Cannot read and write the same texture in a single dispatch. Requires scratch textures and copy commands.
   - *Mitigation*: Single scratch texture allocated at startup. Copy→filter→copy is well-established.

2. **Gradient stop buffer alignment** (Phase 2C): WGSL `array<GradientStop>` requires 16-byte alignment.
   - *Mitigation*: Explicit padding in Rust struct. Use `bytemuck`.

3. **Visual parity** (Phase 4C): GPU antialiasing, sRGB conversions, and shader precision differ from CPU integer arithmetic.
   - *Mitigation*: Tolerance-based comparison. Goal is "visually indistinguishable at desktop distance."

4. **Pipeline layout restructuring** (Phase 1A): Changing rect/shadow/gradient from fullscreen vert to quad vert changes bind group layout structure.
   - *Mitigation*: All pipeline changes in a single commit. Text/image serve as reference.

### Medium Risk

5. **sRGB color space**: Textures use `Bgra8UnormSrgb`, shader math is linear. CPU renderer works in sRGB directly.
6. **Backend differences**: D3D12 vs Vulkan vs Metal may differ on edge cases.

---

## 5. Fallback Strategy

1. **Per-feature fallback**: Each node dispatch can individually fall back to CPU — read back GPU output, render node with CPU, re-upload. Expensive but allows incremental adoption.

2. **Full CPU fallback**: Existing `render_to_framebuffer()` reads back to CPU. If wgpu fails, fall back to `liquide-renderer-cpu` entirely (current production path).

3. **Feature flags**: Each phase can be gated:
   ```toml
   [features]
   gpu-background = []
   gpu-shadow = []
   gpu-gradient = []
   gpu-filter = []
   ```

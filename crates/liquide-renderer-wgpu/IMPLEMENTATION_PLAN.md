# `liquide-renderer-wgpu` — Production Readiness Implementation Plan

**Date**: 2026-04-15
**Crate**: `liquide-renderer-wgpu` (2914 LOC, 0 tests, 8 TODOs, 71 unsafe blocks)
**Reference**: `liquide-renderer-cpu` (15853 LOC, 172 tests)

---

## 1. Current State Assessment

### What Works
- **Device initialization** (`device.rs`): Full `WgpuDevice` with backend selection, adapter enumeration — complete.
- **Pipeline compilation** (`pipeline.rs`): All 7 pipelines compile at startup — `rect`, `blur`, `blend` (compute), `gradient`, `shadow`, `text`, `image`. Complete.
- **Shader code** (`shader.rs`): WGSL shaders exist for all pipeline types. 10/16 CSS blend modes implemented; SoftLight and non-separable modes (Color/Saturation/Hue/Luminosity) fall back to SrcOver.
- **Text rendering** (`renderer.rs:656-812`): Full glyph atlas + per-glyph textured quad pipeline. Working.
- **Image rendering** (`renderer.rs:814-981`): Full image texture cache + UV fit modes (Fill/Contain/Cover/None). Working.
- **Texture management** (`texture.rs`): `GpuTexture` creation, upload, readback. Complete.
- **Vulkan DMA-BUF export** (`vulkan_export.rs`): 71 unsafe blocks, all justified FFI with SAFETY comments. Complete. Do not touch.
- **Readback** (`renderer.rs:983-1020`): GPU→CPU readback via staging buffer. Working.
- **Render-to-framebuffer** (`renderer.rs:1023-1041`): Bridges GPU render to CPU `FrameBuffer`. Working path.

### What's Broken (6 TODO Stubs)

| # | Node Type(s) | Renderer Line | Pipeline Available | Status |
|---|-------------|---------------|-------------------|--------|
| 1 | `Background`, `Tint`, `Glass` | 574-578 | `rect_pipeline` | Stub: `draw_calls += 1` only |
| 2 | `Shadow`, `BoxShadows` | 579-582 | `shadow_pipeline` | Stub: `draw_calls += 1` only |
| 3 | `GradientFill` | 583-586 | `gradient_pipeline` | Stub: `draw_calls += 1` only |
| 4 | `Filter`, `BackdropFilter` | 587-590 | `blur_pipeline` + compute filter | Stub: `draw_calls += 1` only |
| 5 | `RenderLayer` | 591-594 | `blend_pipeline` (compute) | Stub: `draw_calls += 1` only |
| 6 | `Surface`, `ChildSurface` | 595-601 | `image_pipeline` (reuse) | Stub: `draw_calls += 1` only |

### Structural Issue: Missing Bind Group Layouts

`PipelineCache::new()` creates bind group layouts for `rect`, `blur`, `blend`, `gradient`, `shadow` as **local variables** that are dropped after pipeline creation. They are NOT stored on the struct. Only `text_bind_group_layout`, `quad_bind_group_layout`, and `image_bind_group_layout` are retained.

**Fix required**: Store all bind group layouts on `PipelineCache` so `render_frame()` can create bind groups at runtime.

### `render_frame_filtered()` Is Also a Stub

`renderer.rs:1071-1112`: The damage-aware path (`render_frame_filtered`) just does `draw_calls += 1` per node without any actual dispatch. Must be wired to use the same dispatch logic as `render_frame()`.

---

## 2. Detailed Implementation Plan

### Phase 1: Background / Tint / Surface Dispatch (Lowest Risk)

**Goal**: Render the most common visual elements — solid color backgrounds, color tints, and client surface blits.

#### 1A. Store bind group layouts on `PipelineCache`

**File**: `crates/liquide-renderer-wgpu/src/pipeline.rs`

Add fields to `PipelineCache`:
```rust
pub rect_bind_group_layout: wgpu::BindGroupLayout,
pub shadow_bind_group_layout: wgpu::BindGroupLayout,
pub gradient_bind_group_layout: wgpu::BindGroupLayout,
pub gradient_stops_bind_group_layout: wgpu::BindGroupLayout,  // storage buffer BGL
pub blur_bind_group_layout: wgpu::BindGroupLayout,
pub blend_bind_group_layout: wgpu::BindGroupLayout,
```

Change local `let rect_bind_group_layout = ...` to assign to the struct field. Same for all other BGLs. **No API change** — just retaining what was already computed.

**Estimated LOC**: ~20 lines (struct fields + assignment changes)
**Risk**: Zero — these layouts are already created; we just stop dropping them.

#### 1B. Implement `render_background_node()`

**File**: `crates/liquide-renderer-wgpu/src/renderer.rs`

New method on `WgpuRenderer`:
```rust
fn render_background_node(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    output: &GpuTexture,
    node: &FlatNode,
    color: &Color,
) -> u32
```

**What it does** (matching CPU renderer `mod.rs:630-645`):
1. Convert `Color` to `[f32; 4]` premultiplied RGBA.
2. Apply `node.opacity` to alpha.
3. Create `RectUniforms` buffer: `{ color, bounds: [x,y,w,h], corner_radius, opacity }`.
   - Use `node.corner_radius` — max of all 4 corner values for the SDF (current shader uses single radius; upgrade shader later if per-corner needed).
4. Create bind group with `rect_bind_group_layout`.
5. Begin render pass with `LoadOp::Load` on output texture.
6. Set `rect_pipeline`, set bind group, draw 0..3 (fullscreen triangle, scissored).
7. Use wgpu scissor rect matching `node.absolute_bounds` to clip the fullscreen triangle to the node area.

**CPU renderer parity**:
- CPU does: `fill_rounded_rect_per_corner()` for radius > 0, `fill_rect()` for no radius.
- GPU does: SDF rounded rect in `RECT_FILL_FRAG` — unified path handles both cases (SDF evaluates to rectangular when `corner_radius = 0`).

**New uniform struct** (add to `renderer.rs`):
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

**Shader change needed**: The current `RECT_FILL_FRAG` uses UV-space coordinates and fullscreen triangle. To render a specific rect region, we need one of:
- **Option A**: Use scissor rect + compute UV from `gl_FragCoord` relative to bounds (simplest).
- **Option B**: Use the textured quad vertex shader (`TEXTURED_QUAD_VERT`) with the rect fragment shader.

**Recommended**: Option A using scissor rect. The fullscreen fragment shader already has UV, and the `bounds` uniform provides the pixel dimensions needed for SDF. We configure a scissor rect so only the node's pixels are shaded.

**However**, the current fullscreen vert shader outputs UV in [0,1] over the entire viewport. For per-node rendering, we need to remap to the node's bounds. **Better approach**: Add a `viewport` uniform to RectUniforms and convert `gl_FragCoord` to local pixel coordinates in the shader.

**Updated RECT_FILL_FRAG** (modify `shader.rs`):
```wgsl
struct Uniforms {
    color: vec4<f32>,
    bounds: vec4<f32>,     // x, y, w, h in pixels
    viewport: vec2<f32>,   // viewport width, height
    corner_radius: f32,
    opacity: f32,
};

@fragment
fn fs_rect(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Local pixel relative to node bounds
    let local = frag_coord.xy - u.bounds.xy;
    let size = u.bounds.zw;
    let half = size * 0.5;
    let r = u.corner_radius;

    // SDF rounded rect
    let p = abs(local - half) - half + vec2<f32>(r, r);
    let d = length(max(p, vec2<f32>(0.0, 0.0))) - r;
    let alpha = 1.0 - smoothstep(-0.5, 0.5, d);

    return u.color * alpha * u.opacity;
}
```

Wait — the fullscreen triangle approach requires `@builtin(position)` in the fragment, but the current vert output doesn't pass UV in a way that works per-node. **Best approach**: Use the `TEXTURED_QUAD_VERT` for all per-node rendering (rect, shadow, gradient), not the fullscreen triangle. The quad vert already converts pixel rects to NDC and outputs UV [0,1] per quad. The fragment shaders then work on normalized UV within the quad.

**Decision**: Reuse `TEXTURED_QUAD_VERT` + `quad_bind_group_layout` for rect, shadow, and gradient pipelines (same pattern as text/image). This requires modifying their pipeline layouts to include `quad_bind_group_layout` at group(0) and their specific BGL at group(1).

This is a pipeline rebuild, but it makes the rendering architecture uniform: all per-node draw calls use quad vert → fragment shader.

**Estimated LOC**: ~100 lines (method + uniform struct + shader update + pipeline layout change)
**Risk**: Low — same pattern as working text/image pipelines.

#### 1C. Implement `render_tint_node()`

Same as background but uses `BlendMode::Multiply`. The rect pipeline uses `wgpu::BlendState::ALPHA_BLENDING` which is `SrcOver`. For `Multiply` blend, we'd need a different blend state.

**Options**:
1. Create a second rect pipeline with multiply blend state.
2. Use the compute blend pipeline (`blend_pipeline`) to apply multiply.
3. Accept SrcOver for tint (visual approximation — CPU does Multiply).

**Recommended**: Create a second rect pipeline variant with custom blend state for Multiply. Or better: make a tint-specific pipeline with:
```rust
blend: Some(wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Dst,    // result = src*dst + 0
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::OVER,
})
```

Actually, for a tint: CPU does `fill_rect(fb, bounds, color, BlendMode::Multiply)` which is `result.rgb = src.rgb * dst.rgb`. In wgpu fixed-function:
- `src_factor = Dst`, `dst_factor = Zero`, `operation = Add` → `result = src * dst`

But we also need to handle the alpha component correctly. For a simple tint overlay this is sufficient.

**Decision**: Add a `tint_pipeline` to `PipelineCache` — same rect fill shader but with Multiply blend state. This costs one additional pipeline compilation at startup (~0ms, shader is cached).

**Estimated LOC**: ~40 lines (pipeline creation + dispatch method)

#### 1D. Implement `render_glass_node()`

**CPU renderer behavior** (`renderer/effects.rs:16-52`):
1. If blur enabled: backdrop blur at `params.blur_radius`.
2. Fill tint color overlay with SrcOver.
3. If `inner_glow`: render inner glow border.

**GPU approach**:
1. **Backdrop blur**: Requires reading existing framebuffer content in the region → blur it → write back. This is a **Phase 4** item due to complexity (multi-pass copy-blur-composite). For Phase 1, skip the backdrop blur (render the tint only, which is the visible part).
2. Tint: Use rect pipeline with the glass `tint_color`.
3. Inner glow: Skip for now (subtle visual detail, Phase 2+).

**Decision**: Phase 1 implements glass as tint-only. Phase 2 adds backdrop blur. Inner glow is Phase 3+.

**Estimated LOC**: ~15 lines (delegate to background rect render with glass tint color)

#### 1E. Implement `render_surface_node()`

**CPU renderer behavior** (`mod.rs:647-674`):
1. If `opacity >= 1.0` and BGRA8 format: `blit_opaque_stride()`.
2. Else: `blit_alpha_stride()`.

**GPU approach**: This is identical to an Image node — upload the surface buffer pixels as a texture and blit using the image pipeline.

1. Upload `SurfaceBuffer.pixels` to GPU texture (use `GpuTextureCache` with `surface_id` as key).
2. Render using `image_pipeline` (same as `render_image_node()`).
3. UV rect is always `[0,0,1,1]` (full surface).
4. Handle stride correctly during upload (`bytes_per_row = stride`, not `width * 4`).

**Cache consideration**: Surface buffers change each frame. Options:
- Re-upload every frame (simple, correct).
- Track generation counter to skip redundant uploads.

For Phase 1: re-upload on every render. Optimize in Phase 2+ if profiling shows upload is a bottleneck.

**Estimated LOC**: ~60 lines (surface upload + dispatch reusing image pipeline)
**Risk**: Low — same pipeline as working image rendering.

#### Phase 1 Total Estimated LOC: ~235
#### Phase 1 Affected Files:
- `pipeline.rs` — store BGLs, add tint pipeline, restructure rect/shadow/gradient pipeline layouts to use quad vert
- `renderer.rs` — add `render_background_node()`, `render_tint_node()`, `render_glass_node()` (partial), `render_surface_node()`
- `shader.rs` — update `RECT_FILL_FRAG` uniforms to work with quad vert UV

---

### Phase 2: Shadow / Gradient Dispatch

#### 2A. Implement `render_shadow_node()`

**CPU renderer behavior** (`renderer/effects.rs:54-125`):
1. Generate shadow mask (SDF rounding + blur).
2. Cache shadow masks keyed by `node.id`.
3. Composite mask onto framebuffer.

**GPU approach** — the shader `BOX_SHADOW_FRAG` already implements all of this:
- SDF rounded rect distance function
- Spread expansion
- Blur via `smoothstep` falloff
- Inset/outset support
- Color + alpha output

**Implementation**:
```rust
fn render_shadow_node(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    output: &GpuTexture,
    node: &FlatNode,
    spread: f32,
    blur_radius: f32,
    color: &Color,
    corner_radius: f32,
) -> u32
```

1. Compute expanded bounds: `bounds.expand(blur_radius + spread)` for the draw area.
2. Create `ShadowUniforms`: bounds, color (with opacity), offset `(0,0)`, blur, spread, radius, `inset=0`.
3. Use `shadow_pipeline` with quad vert + scissor to expanded bounds.
4. Alpha blending composites the shadow under existing content.

**Note**: The CPU renderer draws shadows with generation+blur in software. The GPU shader uses analytic SDF blur (smoothstep), which is a visual approximation. For exact parity, we'd do a true Gaussian blur on a mask — but the SDF approach is the standard GPU technique and is visually close enough.

**Estimated LOC**: ~60 lines

#### 2B. Implement `render_box_shadows_node()`

Same as 2A but iterates over `Vec<BoxShadowSpec>`:
```rust
for shadow in shadows {
    render one shadow with shadow.offset_x/y, shadow.blur_radius, shadow.spread_radius,
    shadow.color, shadow.inset
}
```

**Estimated LOC**: ~40 lines (loop + parameter mapping)

#### 2C. Implement `render_gradient_node()`

**CPU renderer behavior** (`renderer/gradients.rs:18-100`):
1. Per-pixel evaluation of gradient function (linear/radial/conic).
2. Color stop interpolation.
3. SDF rounded rect mask for corner radius.

**GPU approach** — the shader `GRADIENT_FRAG` already implements:
- Linear, radial, conic gradient evaluation
- Color stop interpolation via `sample_gradient(t)`
- Fully functional in WGSL

**Implementation**:
```rust
fn render_gradient_node(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    output: &GpuTexture,
    node: &FlatNode,
    gradient: &GradientSpec,
) -> u32
```

1. Map `GradientSpec` to `GradientUniforms`:
   - `Linear { start_x, start_y, end_x, end_y }` → `kind=0`, compute angle from endpoints.
   - `Radial { center_x, center_y, radius }` → `kind=1`, center + radius.
   - `Conic { center_x, center_y, start_angle }` → `kind=2`, center + angle.
2. Upload color stops to a GPU storage buffer (`GradientStop` array).
3. Create bind group with gradient BGL (uniform at binding 0, storage at binding 1).
4. Use quad vert for positioning, gradient fragment for evaluation.

**Shader change**: The gradient shader currently uses the fullscreen vert's UV. With quad vert, the UV range is [0,1] within the quad — which is exactly what the gradient shader expects (normalized coordinates).

**Corner radius masking**: The current gradient shader doesn't apply corner radius. Options:
- Add `corner_radius` to `GradientUniforms` and apply SDF mask in shader.
- Rely on a ClipPath node in the scene graph.

**Decision**: Add SDF mask to the gradient shader (matching CPU renderer behavior). Adds ~10 lines of WGSL.

**Estimated LOC**: ~120 lines (gradient parameter mapping + stop buffer upload + dispatch + shader SDF addition)
**Risk**: Medium — storage buffer for gradient stops requires correct alignment and size. The `GradientStop` struct in WGSL has `position: f32` + `color: vec4<f32>` = 20 bytes, which needs 16-byte alignment padding.

#### Phase 2 Total Estimated LOC: ~220
#### Phase 2 Affected Files:
- `renderer.rs` — add `render_shadow_node()`, `render_box_shadows_node()`, `render_gradient_node()`
- `shader.rs` — add SDF mask to gradient shader, possibly add `viewport` to shadow uniforms

---

### Phase 3: Filter / Blend / Backdrop Dispatch (Highest Complexity)

#### 3A. Implement Filter Pipeline (Color Transforms)

**CPU renderer behavior** (`renderer/effects.rs:298-394`):
Per-filter dispatch: Blur, Brightness, Contrast, Saturate, HueRotate, Grayscale, Sepia, Invert, Opacity, DropShadow.

**GPU approach**: Most CSS filters are per-pixel color matrix operations. Best implemented as a **compute shader** that reads the output texture, transforms colors, and writes back.

**New shader**: `FILTER_COMPUTE` — post-processing compute shader:
```wgsl
struct FilterUniforms {
    kind: u32,           // 0=blur, 1=brightness, 2=contrast, 3=saturate, 4=hue_rotate, ...
    value: f32,
    bounds: vec4<f32>,   // clip rect (x, y, w, h)
    matrix: mat4x4<f32>, // generic color matrix (for sepia, hue-rotate etc.)
};
```

For color matrix filters (brightness, contrast, saturate, hue-rotate, grayscale, sepia, invert, opacity), a 4x4 color matrix applied per pixel handles all of them:

| Filter | Matrix |
|--------|--------|
| Brightness(b) | `diag(b, b, b, 1)` |
| Contrast(c) | `diag(c, c, c, 1)` + translate `(0.5*(1-c))` |
| Saturate(s) | Standard saturation matrix |
| HueRotate(θ) | Rotation in YIQ/YCbCr space |
| Grayscale | Saturate(0) |
| Sepia | Fixed sepia matrix |
| Invert(i) | `diag(1-2i, 1-2i, 1-2i, 1)` + translate `(i)` |
| Opacity(o) | `diag(1, 1, 1, o)` |

**New pipeline**: `filter_pipeline` (compute) — reads from output texture, writes modified pixels back.

**Implementation**:
```rust
fn render_filter_node(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    output_view: &wgpu::TextureView,
    node: &FlatNode,
    filters: &[FilterSpec],
) -> u32
```

1. For each filter in the chain:
   - Compute the 4x4 color matrix.
   - If `Blur`: dispatch blur pipeline (same as backdrop blur — 2-pass separable Gaussian).
   - If color matrix: dispatch filter compute shader.
   - If `DropShadow`: dispatch shadow pipeline first, then continue.

**Challenge**: Compute shaders cannot read and write the same texture simultaneously. Need a temp texture:
1. Copy output → temp within bounds.
2. Apply filter: read temp → write output.

This adds a copy command per filter application.

**Estimated LOC**: ~200 lines (new compute shader + pipeline + filter matrix math + dispatch)

#### 3B. Implement BackdropFilter

**CPU renderer behavior** (`renderer/effects.rs:227-296`): Same as Filter but operates on the backdrop (already-rendered pixels behind the element).

**GPU approach**: Identical to Filter — reads current output texture content, applies filter, writes back. The only difference from Filter is that BackdropFilter operates on pixels already in the framebuffer (the "backdrop"), while Filter operates on the element's own rendered content.

Since in our render loop both read/modify the same output texture, the implementation is the same:
1. Copy region from output → temp.
2. Apply filter chain to temp.
3. Composite temp back to output.

For blur specifically, use the existing blur pipeline (2-pass separable).

**Estimated LOC**: ~80 lines (largely shares code with Filter)

#### 3C. Implement Blur Pipeline (Multi-Pass)

The `blur_pipeline` and `BLUR_FRAG` shader exist but need a two-pass dispatch:

1. **Horizontal pass**: Read output → blur with `direction = (1, 0)` → write to temp texture.
2. **Vertical pass**: Read temp → blur with `direction = (0, 1)` → write to output (or another temp).

**Implementation needs**:
- A scratch texture the same size as the output (allocated once, reused).
- Two render passes per blur operation.

```rust
fn apply_blur(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::TextureView,
    dest: &wgpu::TextureView,
    scratch: &wgpu::TextureView,
    radius: f32,
    bounds: Rect,
) -> u32
```

**Estimated LOC**: ~80 lines

#### 3D. Implement RenderLayer (Blend Mode Compositing)

**CPU renderer behavior**: `RenderLayer { blend_mode, isolate }` creates a compositing group. Children are rendered into an isolated layer, then the layer is composited onto the parent with the specified blend mode.

**GPU approach**:
1. If `isolate`: Create temp texture, redirect child rendering to temp.
2. After children: Dispatch `blend_pipeline` (compute) — reads temp (src) + output (dst) → writes output.
3. `blend_mode` maps to the `mode` uniform index (0-13).

**Challenge**: This requires modifying the render loop structure to support "push/pop" of render targets. Currently the loop is flat — it iterates `FlatNode`s linearly. RenderLayer requires:
1. On `RenderLayer` start: push a new render target.
2. Subsequent nodes render to the new target.
3. On "end" of RenderLayer scope: blend temp onto previous target.

But `FlatNode` is a flat list — there's no nesting information. The scene graph is already flattened. **Question**: How does the CPU renderer handle this?

Looking at the CPU renderer: it processes `FlatNode`s linearly and `RenderLayer` nodes just change `self.active_blend_mode`. There's no explicit push/pop — the blend mode applies to subsequent sibling nodes. This is simpler than full CSS isolation.

**GPU equivalent**: Track current blend mode. When encountering `RenderLayer`:
- If `blend_mode != SrcOver` and needs compute compositing: use the compute blend pipeline.
- For hardware-accelerable blend modes, change the render pass blend state.

**Simplification for Phase 3**: Support the most common blend modes via fixed-function blending (Multiply, Screen, Overlay can be done with clever blend state settings — actually, most CSS blend modes **cannot** be done with fixed-function blending). Use the compute blend pipeline for all non-SrcOver modes.

**Estimated LOC**: ~100 lines

#### 3E. Complete SoftLight and Non-Separable Blend Modes in Shader

Address the two remaining TODOs in `shader.rs`:

**SoftLight** (case 11 in `BLEND_COMPUTE`):
```wgsl
fn blend_soft_light(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return select(
        d - (1.0 - 2.0 * s) * d * (1.0 - d),
        d + (2.0 * s - 1.0) * (sqrt(d) - d),
        s > vec3<f32>(0.5)
    );
}
```

**Non-separable modes** (Color, Saturation, Hue, Luminosity — cases 14-17):
These require HSL conversion in the shader. Add helper functions:
```wgsl
fn luminosity(c: vec3<f32>) -> f32 { return dot(c, vec3(0.299, 0.587, 0.114)); }
fn set_luminosity(c: vec3<f32>, l: f32) -> vec3<f32> { ... }
fn saturation(c: vec3<f32>) -> f32 { return max3(c) - min3(c); }
fn set_saturation(c: vec3<f32>, s: f32) -> vec3<f32> { ... }
```

**Estimated LOC**: ~80 lines of WGSL

#### Phase 3 Total Estimated LOC: ~540
#### Phase 3 Affected Files:
- `shader.rs` — new `FILTER_COMPUTE` shader, complete blend modes, SoftLight + non-separable
- `pipeline.rs` — add `filter_pipeline`, store additional BGLs
- `renderer.rs` — add `render_filter_node()`, `render_backdrop_filter_node()`, `apply_blur()`, `render_layer_node()`, scratch texture management

---

### Phase 4: Test Infrastructure

#### 4A. Test Harness: Headless GPU Device

**File**: `crates/liquide-renderer-wgpu/src/tests/mod.rs` (new)

```rust
/// Create a headless wgpu device for testing.
/// Uses wgpu's "cpu" backend (software rasterizer) if no GPU available.
async fn test_device() -> WgpuDevice {
    WgpuDevice::new(None).await
        .expect("test requires wgpu-compatible device")
}

/// Render a list of FlatNodes and return the pixel buffer.
async fn render_nodes(nodes: &[FlatNode], width: u32, height: u32) -> Vec<u8> {
    let gpu = test_device().await;
    let mut renderer = WgpuRenderer::new(gpu, width, height).unwrap();
    renderer.render_frame(nodes).unwrap();
    renderer.read_back().unwrap()
}
```

**Estimated LOC**: ~60 lines

#### 4B. Unit Tests per Node Type

| Test | Description | Validates |
|------|-------------|-----------|
| `test_clear_to_black` | Empty scene → all black pixels | Render loop baseline |
| `test_background_solid` | Single `Background { red }` → region is red | Rect pipeline dispatch |
| `test_background_with_radius` | Rounded rect background → corner pixels transparent | SDF shader |
| `test_background_opacity` | 50% opacity background → blended with black | Alpha blending |
| `test_tint_multiply` | Tint on white background → color = tint * white | Multiply blend |
| `test_surface_blit` | Surface buffer → pixels match uploaded data | Surface dispatch |
| `test_shadow_basic` | Shadow node → non-zero alpha around element | Shadow pipeline |
| `test_shadow_inset` | Inset shadow → alpha inside element | Inset flag |
| `test_gradient_linear` | Linear gradient → pixels interpolate between stops | Gradient pipeline |
| `test_gradient_radial` | Radial gradient → circular falloff | Gradient math |
| `test_gradient_conic` | Conic gradient → angular sweep | Conic gradient |
| `test_filter_brightness` | Brightness(2.0) → pixels doubled | Filter compute |
| `test_filter_blur` | Blur(5) → pixel variance reduces | Blur pipeline |
| `test_filter_grayscale` | Grayscale(1.0) → R==G==B per pixel | Color matrix |
| `test_blend_multiply` | Multiply compositing → result = src * dst | Blend compute |
| `test_blend_screen` | Screen compositing → result = s + d - s*d | Blend compute |
| `test_render_layer_isolate` | Isolated blend group → correct compositing | RenderLayer |
| `test_image_fit_cover` | Image with Cover fit → cropped UV | UV computation |
| `test_text_glyph_atlas` | Upload + render glyphs → non-zero pixels | Text pipeline |
| `test_resize` | Resize output → no crash, correct dimensions | Texture recreation |
| `test_damage_filtering` | Only damaged tiles rendered | Damage-aware path |
| `test_read_back` | Read back buffer → size matches w*h*4 | Readback correctness |

**Estimated LOC**: ~400 lines (22 tests, ~18 lines each)

#### 4C. Visual Parity Tests Against CPU Renderer

**File**: `crates/liquide-renderer-wgpu/src/tests/parity.rs` (new)

These tests render the same scene through both CPU and wgpu renderers, then compare output:

```rust
/// Compare wgpu vs CPU renderer output pixel-by-pixel.
/// Allows a tolerance per channel (GPU rounding, sRGB conversions).
fn assert_visual_parity(
    wgpu_pixels: &[u8],
    cpu_pixels: &[u8],
    width: u32,
    height: u32,
    max_channel_diff: u8,     // e.g., 2 for rounding tolerance
    max_failing_pixels: f64,  // e.g., 0.01 = 1% of pixels
)
```

Parity tests:
| Test | Scene |
|------|-------|
| `parity_solid_background` | Full-viewport solid red |
| `parity_rounded_rect` | 200x100 rect with 10px corners |
| `parity_gradient_linear` | Linear gradient red→blue |
| `parity_shadow` | Shadow with 10px blur |
| `parity_opacity_composite` | Two overlapping rects at 50% opacity |

**Note**: Perfect pixel-for-pixel match is unlikely due to GPU sRGB handling, shader precision, and SDF antialiasing differences. Tolerance of ±2 per channel with max 5% failing pixels is reasonable.

**Estimated LOC**: ~180 lines

#### 4D. Regression Test: Render-to-Framebuffer Round-Trip

Validates the `render_to_framebuffer()` path end-to-end:
```rust
#[test]
fn test_render_to_framebuffer_roundtrip() {
    // Render via wgpu, read back to CPU FrameBuffer, verify pixels match expectations.
}
```

**Estimated LOC**: ~30 lines

#### Phase 4 Total Estimated LOC: ~670
#### Phase 4 New Files:
- `crates/liquide-renderer-wgpu/src/tests/mod.rs`
- `crates/liquide-renderer-wgpu/src/tests/parity.rs`

---

### Phase 5: Cleanup & Polish

#### 5A. Wire `render_frame_filtered()`

The damage-aware rendering path at `renderer.rs:1071-1112` just loops nodes without dispatch. Extract the match arms into a shared helper function called by both `render_frame()` and `render_frame_filtered()`.

```rust
fn dispatch_node(
    &self,
    encoder: &mut wgpu::CommandEncoder,
    output: &GpuTexture,
    node: &FlatNode,
) -> u32
```

Both `render_frame()` and `render_frame_filtered()` call `dispatch_node()` per node.

**Estimated LOC**: ~30 lines (refactor, no new logic)

#### 5B. Handle Remaining `SceneNodeKind` Variants

Node types not covered in Phases 1-3 that the wgpu renderer should at least stub correctly:
- `Decoration` — title bar rendering (text + rect + border). Delegate to rect + text pipelines.
- `BlurBackdrop` / `BlurCache` — backdrop blur regions. Wire to blur pipeline.
- `Content` / `Overlay` / `ShellLayer` — layer opacity modulation. Wire to filter pass (Opacity).
- `Cursor` — software cursor. Wire to image pipeline.
- `LockScreen` — backdrop blur + dark overlay. Wire to blur + rect.
- `CrashScreen` — red overlay. Wire to rect.
- `SvgPath` — Phase 3+ (requires path tessellation).
- `BackgroundFill`, `Border`, `BorderImage`, `ClipPath`, `Mask`, `Outline`, `Icon`, `TextCaret`, `SelectionOverlay` — incremental additions.

**Estimated LOC**: ~200 lines

#### 5C. Remove All TODO Comments

Replace each TODO stub with the actual implementation reference.

---

## 3. Summary Table

| Phase | Scope | New LOC | Files Modified | Risk |
|-------|-------|---------|----------------|------|
| **1** | Background / Tint / Surface | ~235 | pipeline.rs, renderer.rs, shader.rs | **Low** |
| **2** | Shadow / Gradient | ~220 | renderer.rs, shader.rs | **Medium** |
| **3** | Filter / Blur / Blend / RenderLayer | ~540 | shader.rs, pipeline.rs, renderer.rs | **High** |
| **4** | Tests (22 unit + 5 parity) | ~670 | tests/mod.rs, tests/parity.rs (new) | **Low** |
| **5** | Cleanup / remaining node types | ~230 | renderer.rs, pipeline.rs | **Low** |
| **Total** | | **~1895** | | |

---

## 4. Risk Assessment

### High Risk Items

1. **Filter compute shader read/write conflicts** (Phase 3A): Cannot read and write the same texture in a single compute dispatch. Requires temp textures and copy commands. If mishandled, causes undefined behavior.
   - **Mitigation**: Allocate one scratch texture at startup (same size as output). Copy→filter→copy pattern is well-established.

2. **Gradient stop storage buffer alignment** (Phase 2C): WGSL `array<GradientStop>` with `GradientStop { position: f32, color: vec4<f32> }` requires 16-byte alignment. The Rust-side struct must match.
   - **Mitigation**: Add explicit padding: `struct GradientStop { position: f32, _pad: [f32; 3], color: [f32; 4] }` = 32 bytes. Use `bytemuck`.

3. **Visual parity** (Phase 4C): GPU antialiasing (SDF smoothstep), sRGB conversions, and shader precision will differ from CPU's integer arithmetic.
   - **Mitigation**: Tolerance-based comparison. Document known differences. The goal is "visually indistinguishable at desktop distance," not bit-exact.

4. **Pipeline layout restructuring** (Phase 1A): Changing rect/shadow/gradient pipelines from fullscreen vert to quad vert changes their bind group layout structure. Must update both pipeline creation AND dispatch simultaneously.
   - **Mitigation**: Do all pipeline changes in a single commit. Test with existing text/image as reference for the pattern.

### Medium Risk Items

5. **sRGB color space**: Textures use `Bgra8UnormSrgb` (sRGB). Shader arithmetic is in linear space. The CPU renderer works in sRGB directly. This means color blending will differ slightly.
   - **Mitigation**: Accept differences, document in parity tests. Optionally add a `Bgra8Unorm` (non-sRGB) output mode.

6. **wgpu backend differences**: D3D12 vs Vulkan vs Metal may produce slightly different results for edge cases (smoothstep precision, rounding).
   - **Mitigation**: Run parity tests on CI with multiple backends if available.

### Low Risk Items

7. All uniform struct alignment (already done correctly for text/image — same pattern).
8. Bind group creation per draw call (performance concern, not correctness — can batch later).
9. Glyph atlas full → rebuild (already handled with `clear_glyph_atlas()`).

---

## 5. Fallback Strategy

If GPU rendering proves unreliable or visually inconsistent:

1. **Per-feature fallback**: Each node type dispatch can individually fall back to the CPU renderer by:
   - Reading back the current GPU output texture.
   - Rendering the node with the CPU renderer.
   - Re-uploading modified pixels.
   - This is expensive per-node but allows incremental GPU adoption.

2. **Full CPU fallback**: The existing `render_to_framebuffer()` method already reads back to CPU memory. If wgpu rendering fails, the system falls back to `liquide-renderer-cpu` entirely (already the production path today).

3. **Feature flags**: Each phase can be gated behind a feature flag:
   ```toml
   [features]
   gpu-background = []
   gpu-shadow = []
   gpu-gradient = []
   gpu-filter = []
   ```
   Allowing selective enablement during rollout.

---

## 6. Dependency Changes

No new crate dependencies needed. All required crates are already in `Cargo.toml`:
- `wgpu = "24"` — GPU abstraction
- `bytemuck = "1"` — uniform struct casting
- `liquide-compositor` — scene types, damage, framebuffer

For testing, may need:
```toml
[dev-dependencies]
pollster = "0.4"    # block_on() for async device creation in tests
liquide-renderer-cpu = { path = "../liquide-renderer-cpu" }  # for parity tests
```

---

## 7. Implementation Order & Suggested Commits

1. `feat(renderer-wgpu): store all bind group layouts on PipelineCache`
2. `feat(renderer-wgpu): restructure rect/shadow/gradient pipelines to use quad vert`
3. `feat(renderer-wgpu): implement Background node dispatch via rect pipeline`
4. `feat(renderer-wgpu): implement Tint node dispatch with multiply blend`
5. `feat(renderer-wgpu): implement Surface/ChildSurface node dispatch via image pipeline`
6. `test(renderer-wgpu): add test harness and Phase 1 unit tests`
7. `feat(renderer-wgpu): implement Shadow/BoxShadows node dispatch`
8. `feat(renderer-wgpu): implement GradientFill node dispatch`
9. `test(renderer-wgpu): add Phase 2 unit tests`
10. `feat(renderer-wgpu): add scratch texture for multi-pass operations`
11. `feat(renderer-wgpu): implement Filter/BackdropFilter dispatch with color matrix compute`
12. `feat(renderer-wgpu): implement two-pass blur`
13. `feat(renderer-wgpu): implement RenderLayer blend dispatch`
14. `feat(renderer-wgpu): complete SoftLight and non-separable blend modes in shader`
15. `test(renderer-wgpu): add Phase 3 unit tests and visual parity tests`
16. `refactor(renderer-wgpu): extract dispatch_node() and wire render_frame_filtered()`
17. `feat(renderer-wgpu): stub remaining node types (Decoration, Cursor, LockScreen, etc.)`

//! WGSL shader sources for the wgpu renderer.

/// Fullscreen quad vertex shader — emits a screen-covering triangle.
pub const FULLSCREEN_VERT: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generates a fullscreen triangle
    let x = f32(i32(vi & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vi >> 1u)) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}
"#;

/// Rect fill fragment shader — solid color + opacity.
pub const RECT_FILL_FRAG: &str = r#"
struct Uniforms {
    color: vec4<f32>,
    bounds: vec4<f32>,  // x, y, w, h
    corner_radius: f32,
    opacity: f32,
    _pad: vec2<f32>,
};

@group(1) @binding(0) var<uniform> u: Uniforms;

@fragment
fn fs_rect(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let pixel = uv * vec2<f32>(u.bounds.z, u.bounds.w);
    let half = vec2<f32>(u.bounds.z, u.bounds.w) * 0.5;
    let r = u.corner_radius;

    // SDF rounded rect
    let p = abs(pixel - half) - half + vec2<f32>(r, r);
    let d = length(max(p, vec2<f32>(0.0, 0.0))) - r;
    let alpha = 1.0 - smoothstep(-0.5, 0.5, d);

    return u.color * alpha * u.opacity;
}
"#;

/// Gaussian blur fragment shader (single-pass separable).
pub const BLUR_FRAG: &str = r#"
struct BlurUniforms {
    direction: vec2<f32>,  // (1,0) for horizontal, (0,1) for vertical
    radius: f32,
    _pad: f32,
};

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> u: BlurUniforms;

@fragment
fn fs_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_input));
    let texel = 1.0 / dims;

    var color = vec4<f32>(0.0);
    var weight_sum = 0.0;

    let r = i32(u.radius);
    let sigma = u.radius * 0.5;

    for (var i = -r; i <= r; i++) {
        let offset = vec2<f32>(f32(i)) * u.direction * texel;
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        color += textureSample(t_input, s_input, uv + offset) * w;
        weight_sum += w;
    }

    return color / weight_sum;
}
"#;

/// CSS blend mode compute shader — applies all 16 CSS blend modes.
pub const BLEND_COMPUTE: &str = r#"
struct BlendUniforms {
    mode: u32,
    _pad: vec3<u32>,
};

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var t_dst: texture_2d<f32>;
@group(0) @binding(2) var t_out: texture_storage_2d<bgra8unorm, write>;
@group(0) @binding(3) var<uniform> u: BlendUniforms;

fn blend_multiply(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return s * d; }
fn blend_screen(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return s + d - s * d; }
fn blend_overlay(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return select(
        1.0 - 2.0 * (1.0 - s) * (1.0 - d),
        2.0 * s * d,
        d < vec3<f32>(0.5)
    );
}
fn blend_darken(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return min(s, d); }
fn blend_lighten(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return max(s, d); }
fn blend_color_dodge(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return select(d / (1.0 - s), vec3<f32>(1.0), s >= vec3<f32>(1.0));
}
fn blend_color_burn(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return select(1.0 - (1.0 - d) / s, vec3<f32>(0.0), s <= vec3<f32>(0.0));
}
fn blend_hard_light(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return blend_overlay(d, s);
}
fn blend_difference(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return abs(s - d); }
fn blend_exclusion(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> { return s + d - 2.0 * s * d; }

// SoftLight per CSS Compositing spec (W3C formula)
fn blend_soft_light(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let lo = 2.0 * s * d + d * d * (1.0 - 2.0 * s);
    let hi = 2.0 * d * (1.0 - s) + sqrt(d) * (2.0 * s - 1.0);
    return select(hi, lo, s <= vec3<f32>(0.5));
}

// ── Non-separable blend mode helpers (CSS Compositing §8.4) ──────────

fn lum(c: vec3<f32>) -> f32 {
    return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;
}

fn clip_color(c_in: vec3<f32>) -> vec3<f32> {
    var c = c_in;
    let l = lum(c);
    let n = min(c.r, min(c.g, c.b));
    let x = max(c.r, max(c.g, c.b));
    if (n < 0.0) {
        c = l + (c - l) * l / (l - n);
    }
    if (x > 1.0) {
        c = l + (c - l) * (1.0 - l) / (x - l);
    }
    return c;
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    let d = l - lum(c);
    return clip_color(c + vec3<f32>(d));
}

fn sat(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

// SetSat: rearranges the color so that the channel ordering is preserved
// and the saturation (max - min) equals `s`.
fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let cmin = min(c.r, min(c.g, c.b));
    let cmax = max(c.r, max(c.g, c.b));
    if (cmax - cmin > 0.0001) {
        return (c - cmin) * s / (cmax - cmin);
    }
    return vec3<f32>(0.0);
}

fn blend_hue(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return set_lum(set_sat(s, sat(d)), lum(d));
}

fn blend_saturation(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return set_lum(set_sat(d, sat(s)), lum(d));
}

fn blend_color(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return set_lum(s, lum(d));
}

fn blend_luminosity(s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return set_lum(d, lum(s));
}

@compute @workgroup_size(8, 8)
fn cs_blend(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(t_src);
    if (gid.x >= dims.x || gid.y >= dims.y) { return; }

    let coord = vec2<i32>(gid.xy);
    let src = textureLoad(t_src, coord, 0);
    let dst = textureLoad(t_dst, coord, 0);

    var blended: vec3<f32>;
    switch (u.mode) {
        case 0u: { blended = src.rgb; }                           // SrcOver (simplified)
        case 1u: { blended = src.rgb; }                           // Src
        case 2u: { blended = src.rgb; }                           // SrcAtop
        case 3u: { blended = blend_multiply(src.rgb, dst.rgb); }
        case 4u: { blended = blend_screen(src.rgb, dst.rgb); }
        case 5u: { blended = blend_overlay(src.rgb, dst.rgb); }
        case 6u: { blended = blend_darken(src.rgb, dst.rgb); }
        case 7u: { blended = blend_lighten(src.rgb, dst.rgb); }
        case 8u: { blended = blend_color_dodge(src.rgb, dst.rgb); }
        case 9u: { blended = blend_color_burn(src.rgb, dst.rgb); }
        case 10u: { blended = blend_hard_light(src.rgb, dst.rgb); }
        case 11u: { blended = blend_soft_light(src.rgb, dst.rgb); }
        case 12u: { blended = blend_difference(src.rgb, dst.rgb); }
        case 13u: { blended = blend_exclusion(src.rgb, dst.rgb); }
        case 14u: { blended = blend_hue(src.rgb, dst.rgb); }
        case 15u: { blended = blend_saturation(src.rgb, dst.rgb); }
        case 16u: { blended = blend_color(src.rgb, dst.rgb); }
        case 17u: { blended = blend_luminosity(src.rgb, dst.rgb); }
        default: { blended = src.rgb; }
    }

    // Compositing: result * src.a + dst * (1 - src.a)
    let out_rgb = blended * src.a + dst.rgb * (1.0 - src.a);
    let out_a = src.a + dst.a * (1.0 - src.a);

    textureStore(t_out, coord, vec4<f32>(out_rgb, out_a));
}
"#;

/// Gradient fragment shader.
pub const GRADIENT_FRAG: &str = r#"
struct GradientUniforms {
    kind: u32,           // 0 = linear, 1 = radial, 2 = conic
    angle: f32,          // linear: angle in radians; conic: start angle
    center: vec2<f32>,   // radial/conic center (normalized 0..1)
    radius: f32,         // radial radius
    stop_count: u32,
    _pad: vec2<f32>,
};

struct GradientStop {
    position: f32,
    color: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: GradientUniforms;
@group(1) @binding(1) var<storage, read> stops: array<GradientStop>;

fn lerp_color(a: vec4<f32>, b: vec4<f32>, t: f32) -> vec4<f32> {
    return mix(a, b, vec4<f32>(t));
}

fn sample_gradient(t_raw: f32) -> vec4<f32> {
    let t = clamp(t_raw, 0.0, 1.0);
    if (u.stop_count == 0u) { return vec4<f32>(0.0); }
    if (u.stop_count == 1u) { return stops[0].color; }

    for (var i = 0u; i < u.stop_count - 1u; i++) {
        if (t >= stops[i].position && t <= stops[i + 1u].position) {
            let range = stops[i + 1u].position - stops[i].position;
            let local_t = select((t - stops[i].position) / range, 0.0, range < 0.001);
            return lerp_color(stops[i].color, stops[i + 1u].color, local_t);
        }
    }
    return stops[u.stop_count - 1u].color;
}

@fragment
fn fs_gradient(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    var t: f32;

    switch (u.kind) {
        case 0u: {
            // Linear gradient
            let dir = vec2<f32>(cos(u.angle), sin(u.angle));
            t = dot(uv - vec2<f32>(0.5), dir) + 0.5;
        }
        case 1u: {
            // Radial gradient
            t = length(uv - u.center) / u.radius;
        }
        case 2u: {
            // Conic gradient
            let d = uv - u.center;
            t = (atan2(d.y, d.x) - u.angle) / (2.0 * 3.14159265) + 0.5;
            t = fract(t);
        }
        default: {
            t = 0.0;
        }
    }

    return sample_gradient(t);
}
"#;

/// Textured quad vertex shader — transforms a unit quad to screen-space pixel rect.
///
/// Uses `dst_rect` (x, y, w, h in pixels) and `viewport` (width, height) uniforms
/// to position a quad. Outputs UV coordinates for texture sampling.
pub const TEXTURED_QUAD_VERT: &str = r#"
struct QuadUniforms {
    dst_rect: vec4<f32>,   // x, y, w, h in pixels
    viewport: vec2<f32>,   // viewport width, height
    _pad: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<uniform> qu: QuadUniforms;

@vertex
fn vs_quad(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Two-triangle quad: 0,1,2, 2,1,3
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let indices = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let idx = indices[vi];
    let pos = positions[idx];

    // Convert pixel rect to NDC
    let px = qu.dst_rect.x + pos.x * qu.dst_rect.z;
    let py = qu.dst_rect.y + pos.y * qu.dst_rect.w;
    let ndc_x = (px / qu.viewport.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (py / qu.viewport.y) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = pos;
    return out;
}
"#;

/// Text glyph fragment shader — samples alpha from glyph atlas, applies text color.
///
/// The atlas is an R8 texture (alpha-only). The shader multiplies the sampled
/// alpha by the text color and opacity to produce the final premultiplied output.
pub const TEXT_FRAG: &str = r#"
@group(1) @binding(0) var t_atlas: texture_2d<f32>;
@group(1) @binding(1) var s_atlas: sampler;
@group(1) @binding(2) var<uniform> tu: TextUniforms;

struct TextUniforms {
    color: vec4<f32>,
    src_rect: vec4<f32>,  // uv_min.x, uv_min.y, uv_max.x, uv_max.y
    opacity: f32,
    _pad: vec3<f32>,
};

@fragment
fn fs_text(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let atlas_uv = mix(tu.src_rect.xy, tu.src_rect.zw, uv);
    let alpha = textureSample(t_atlas, s_atlas, atlas_uv).r;
    return vec4<f32>(tu.color.rgb * alpha * tu.opacity, alpha * tu.opacity * tu.color.a);
}
"#;

/// Image blit fragment shader — samples from an image texture with UV mapping.
///
/// Supports image fitting via `src_rect` which defines the UV sub-region
/// of the source texture to sample from (computed on CPU based on ImageFit).
pub const IMAGE_FRAG: &str = r#"
@group(1) @binding(0) var t_image: texture_2d<f32>;
@group(1) @binding(1) var s_image: sampler;
@group(1) @binding(2) var<uniform> iu: ImageUniforms;

struct ImageUniforms {
    src_rect: vec4<f32>,  // uv_min.x, uv_min.y, uv_max.x, uv_max.y
    opacity: f32,
    _pad: vec3<f32>,
};

@fragment
fn fs_image(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let tex_uv = mix(iu.src_rect.xy, iu.src_rect.zw, uv);
    let color = textureSample(t_image, s_image, tex_uv);
    return vec4<f32>(color.rgb * iu.opacity, color.a * iu.opacity);
}
"#;

/// Box-shadow fragment shader using SDF.
pub const BOX_SHADOW_FRAG: &str = r#"
struct ShadowUniforms {
    bounds: vec4<f32>,   // x, y, w, h of the casting box
    color: vec4<f32>,
    offset: vec2<f32>,
    blur: f32,
    spread: f32,
    radius: f32,
    inset: u32,
    _pad: vec2<f32>,
};

@group(1) @binding(0) var<uniform> u: ShadowUniforms;

fn sdf_rounded_rect(p: vec2<f32>, half_ext: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_ext + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) - r;
}

@fragment
fn fs_shadow(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let screen_pos = uv * vec2<f32>(u.bounds.z, u.bounds.w);
    let box_center = vec2<f32>(u.bounds.z, u.bounds.w) * 0.5 + u.offset;
    let half_ext = vec2<f32>(u.bounds.z, u.bounds.w) * 0.5 + vec2<f32>(u.spread);

    let d = sdf_rounded_rect(screen_pos - box_center, half_ext, u.radius);

    var alpha: f32;
    if (u.inset == 1u) {
        // Inset: shadow inside the box
        alpha = smoothstep(0.0, u.blur, d);
    } else {
        // Outset: shadow outside the box
        alpha = 1.0 - smoothstep(-u.blur, 0.0, d);
    }

    return u.color * alpha;
}
"#;

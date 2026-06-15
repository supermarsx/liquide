use crate::glyph::{GlyphKey, GlyphMetrics};
use crate::renderer::*;
use liquide_compositor::damage::DamageClass;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::{FrameMemoryKind, RendererBackendKind, RendererRejectReason};

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::scene::{FlatNode, SceneNodeKind};
use liquide_font_rasterizer::database::FontDatabase;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn text_node(text: &str, font_family: &str) -> FlatNode {
    FlatNode {
        id: 1_000,
        kind: SceneNodeKind::Text {
            text: text.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: font_family.to_string(),
            font_size: 16.0,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 0.0,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            white_space: 0,
            word_break: liquide_compositor::scene::WordBreak::Normal,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 32.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn render_text_once(renderer: &mut SoftwareRenderer, node: FlatNode) {
    let mut fb = FrameBuffer::new(128, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });
    renderer.render(&[node], &mut fb, &damage).unwrap();
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "liquide-renderer-cpu-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn fixture_font_bytes() -> Option<Vec<u8>> {
    let candidates = [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\calibri.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    candidates.iter().find_map(|path| {
        let data = std::fs::read(path).ok()?;
        let mut db = FontDatabase::new();
        db.load_bytes(data.clone(), "Probe", 400, false).ok()?;
        Some(data)
    })
}

fn write_fixture_font(label: &str) -> Option<(PathBuf, PathBuf)> {
    let data = fixture_font_bytes()?;
    let dir = unique_temp_dir(label);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("fixture.ttf");
    std::fs::write(&path, data).ok()?;
    Some((dir, path))
}

#[test]
fn renderer_creates() {
    let r = SoftwareRenderer::new();
    assert!(r.glyph_atlas().is_empty());
}

/// `compute_font_id` must map distinct (family, weight, italic) tuples to
/// distinct ids — otherwise two different fonts share a glyph-atlas key and the
/// cache returns the WRONG glyph (the text-garbling regression). This pins the
/// collision-free property the renderer relies on, including the weight bits
/// that the old `(weight & 0xFF) << 16` packing truncated and aliased.
#[test]
fn font_id_is_collision_free_across_family_weight_italic() {
    use std::collections::HashSet;

    let families = ["", "Inter", "Segoe UI", "Inter Display", "Arial"];
    let weights: [u16; 9] = [100, 200, 300, 400, 500, 600, 700, 800, 900];
    let mut ids = HashSet::new();
    for fam in families {
        for w in weights {
            for italic in [false, true] {
                let id = compute_font_id(fam, w, italic);
                assert!(
                    ids.insert(id),
                    "font_id collision: ({fam:?}, {w}, italic={italic}) aliased to an \
                     existing id {id:#010x} — distinct fonts would share atlas keys and \
                     return wrong glyphs"
                );
            }
        }
    }

    // The id is a pure function of its inputs (stable run-to-run).
    assert_eq!(
        compute_font_id("Inter", 700, true),
        compute_font_id("Inter", 700, true),
    );
    // Italic and weight each flip the id (no aliasing into the upright/other-weight key).
    assert_ne!(
        compute_font_id("Inter", 400, false),
        compute_font_id("Inter", 400, true),
    );
    assert_ne!(
        compute_font_id("Inter", 400, false),
        compute_font_id("Inter", 700, false),
    );
}

/// DETERMINISM REGRESSION (t65): rendering the SAME text scene through the full
/// async font-worker path must produce a BYTE-IDENTICAL framebuffer every time.
///
/// The regression this guards: glyphs rasterise on a background thread, so a
/// non-blocking poll committed only whatever happened to have arrived, and text
/// layout reads glyph advances out of the atlas — a partially-populated atlas
/// reflowed/garbled text differently each run. Driving each render to quiescence
/// (`has_pending_glyphs` clear) must now land on the same pixels regardless of
/// worker thread timing.
#[test]
fn identical_text_scene_renders_byte_identically_across_renderers() {
    // Render a fresh renderer to quiescence and return the final framebuffer.
    fn render_to_quiescence(text: &str, family: &str) -> Vec<u8> {
        let mut renderer = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(256, 64, PixelFormat::Bgra8);
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::TextGlyph,
        });
        // Drive render passes until no glyphs remain pending (mirrors the
        // capture path's reflush). Bounded so a missing system font can't hang.
        for _ in 0..8 {
            renderer
                .render(&[text_node(text, family)], &mut fb, &damage)
                .unwrap();
            if !renderer.has_pending_glyphs() {
                break;
            }
        }
        fb.pixels().to_vec()
    }

    // Use a real system font when present (exercises the async rasteriser); the
    // bitmap fallback is fine too — both must be deterministic.
    let family = if fixture_font_bytes().is_some() {
        "Arial"
    } else {
        ""
    };
    let text = "Open Terminal File Manager Settings";

    let a = render_to_quiescence(text, family);
    let b = render_to_quiescence(text, family);
    let c = render_to_quiescence(text, family);

    assert_eq!(
        a, b,
        "identical text scene rendered to two byte-different framebuffers — \
         glyph render path is nondeterministic"
    );
    assert_eq!(a, c, "third render diverged — glyph render path is nondeterministic");
    // Sanity: the text actually painted something (not a vacuously-equal blank).
    assert!(a.iter().any(|&p| p != 0), "scene produced an all-zero framebuffer");
}

#[test]
fn renderer_options_disable_common_glyph_prewarm() {
    let mut renderer = SoftwareRenderer::with_options(SoftwareRendererOptions {
        glyph_prewarm: GlyphPrewarmMode::Disabled,
    });

    render_text_once(&mut renderer, text_node("A", "Inter"));

    assert_eq!(renderer.prewarmed_font_count(), 0);
    assert_eq!(renderer.pending_glyph_request_count(), 1);
}

#[test]
fn renderer_default_options_prewarm_common_glyphs() {
    let mut renderer = SoftwareRenderer::new();

    render_text_once(&mut renderer, text_node("A", "Inter"));

    assert_eq!(renderer.prewarmed_font_count(), 1);
    assert!(
        renderer.pending_glyph_request_count() > 1,
        "default prewarm should enqueue common glyphs in addition to visible text"
    );
}

#[test]
fn stale_font_invalidation_returns_faces_and_clears_cpu_glyph_state() {
    let Some((dir, path)) = write_fixture_font("stale-font") else {
        return;
    };
    let mut db = FontDatabase::new();
    let face_id = db.load_file(&path, "Fixture", 400, false).unwrap();
    let mut renderer = SoftwareRenderer::with_font_db(db);

    render_text_once(&mut renderer, text_node("A", "Fixture"));
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: 7,
                glyph_id: 'A' as u32,
                size_px: 16,
                subpixel: false,
            },
            &[255],
            &GlyphMetrics {
                width: 1,
                height: 1,
                bearing_x: 0,
                bearing_y: 1,
                advance: 1.0,
            },
        )
        .unwrap();
    assert!(!renderer.glyph_atlas().is_empty());
    assert_eq!(renderer.prewarmed_font_count(), 1);
    assert!(renderer.pending_glyph_request_count() > 0);
    assert!(renderer.invalidate_stale_fonts().is_empty());

    let mut data = std::fs::read(&path).unwrap();
    data.extend_from_slice(b"stale");
    std::fs::write(&path, data).unwrap();

    let stale_faces = renderer.invalidate_stale_fonts();

    assert_eq!(stale_faces, vec![face_id]);
    assert!(renderer.glyph_atlas().is_empty());
    assert_eq!(renderer.prewarmed_font_count(), 0);
    assert_eq!(renderer.pending_glyph_request_count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cpu_renderer_reports_backend_capabilities() {
    let renderer = SoftwareRenderer::new();

    let info = renderer.backend_info();
    assert_eq!(info.kind, RendererBackendKind::Software);
    assert_eq!(info.name, "liquide-renderer-cpu");
    assert_eq!(info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

    let capabilities = renderer.capabilities();
    assert!(capabilities.supports_frame_memory(FrameMemoryKind::Cpu));
    assert!(capabilities.supports_pixel_format(PixelFormat::Bgra8));
    assert!(capabilities.supports_pixel_format(PixelFormat::Rgba8));
    assert!(capabilities.supports_pixel_format(PixelFormat::Rgb8));
    assert!(capabilities.supports_partial_damage);
    assert!(capabilities.supports_blur);
    assert!(capabilities.supports_skeleton_window);
    assert!(capabilities.supports_async_glyphs);
}

#[test]
fn cpu_negotiation_accepts_writable_cpu_framebuffers() {
    let renderer = SoftwareRenderer::new();
    let fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let damage = DamageSet::new(64);

    let negotiation = renderer.negotiate_render(&[], &fb, &damage);

    assert!(negotiation.is_accepted());
}

#[test]
fn cpu_negotiation_rejects_gpu_and_unsupported_formats() {
    let renderer = SoftwareRenderer::new();
    let damage = DamageSet::new(64);
    let gpu_fb = FrameBuffer {
        memory: FrameMemory::Gpu {
            handle: 1,
            dmabuf_fd: -1,
            width: 16,
            height: 16,
        },
        width: 16,
        height: 16,
        stride: 64,
        format: PixelFormat::Bgra8,
    };

    let gpu_negotiation = renderer.negotiate_render(&[], &gpu_fb, &damage);
    assert!(matches!(
        gpu_negotiation.reject_reason(),
        Some(RendererRejectReason::UnsupportedFrameMemory {
            memory: FrameMemoryKind::Gpu
        })
    ));

    let unsupported_format = FrameBuffer::new(16, 16, PixelFormat::Rgb565);
    let format_negotiation = renderer.negotiate_render(&[], &unsupported_format, &damage);
    assert!(matches!(
        format_negotiation.reject_reason(),
        Some(RendererRejectReason::UnsupportedPixelFormat {
            format: PixelFormat::Rgb565
        })
    ));
}

#[test]
fn cpu_negotiation_rejects_unwritable_cpu_buffers() {
    let renderer = SoftwareRenderer::new();
    let damage = DamageSet::new(64);
    let fb = FrameBuffer {
        memory: FrameMemory::Cpu(vec![0; 8]),
        width: 4,
        height: 4,
        stride: 16,
        format: PixelFormat::Bgra8,
    };

    let negotiation = renderer.negotiate_render(&[], &fb, &damage);

    assert!(matches!(
        negotiation.reject_reason(),
        Some(RendererRejectReason::Other(reason)) if reason.contains("requires at least")
    ));
}

#[test]
fn render_background() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 1,
        kind: SceneNodeKind::Background {
            color: Color::new(0, 100, 200, 255),
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    renderer.render(&[node], &mut fb, &damage).unwrap();
    let c = fb.get_pixel(32, 32);
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 100);
    assert_eq!(c.b, 200);
}

#[test]
fn render_surface_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 10,
        kind: SceneNodeKind::Surface {
            surface_id: 1,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    // Should not error even with no buffer
    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(
        result.is_ok(),
        "render Surface with no buffer should succeed"
    );
}

#[test]
fn render_glass_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb.clear(Color::new(100, 100, 100, 255));
    let before = fb.pixels().to_vec();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 20,
        kind: SceneNodeKind::Glass(liquide_compositor::scene::GlassParams::default()).into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Glass node should succeed");
    assert_ne!(fb.pixels(), &before[..], "Glass tint should modify pixels");
}

#[test]
fn render_decoration_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 30,
        kind: SceneNodeKind::Decoration {
            title: Some("Test Window".to_string()),
            title_color: Color::WHITE,
            background: Color::new(50, 50, 60, 255),
            border_color: Color::new(100, 100, 120, 255),
            border_width: 1.0,
            corner_radius: 8.0,
            button_state: liquide_compositor::scene::DecorationButtons::default(),
            button_colors: liquide_compositor::scene::DecorationColors::default(),
            button_layout: liquide_compositor::scene::DecorationLayout::default(),
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 32.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render Decoration node should succeed");
    // The background fill should have modified some pixels
    let center = fb.get_pixel(32, 16);
    assert!(
        center.r > 0 || center.g > 0 || center.b > 0,
        "decoration should fill pixels: got {:?}",
        center
    );
}

#[test]
fn render_lock_screen_node() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    // Fill with content first
    for y in 0..128 {
        for x in 0..128 {
            fb.set_pixel(x, y, Color::new((x * 2) as u8, (y * 2) as u8, 128, 255));
        }
    }
    let before = fb.pixels().to_vec();

    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 0,
        y: 1,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 1,
        class: DamageClass::UiPrimitive,
    });

    let node = FlatNode {
        id: 40,
        kind: SceneNodeKind::LockScreen.into(),
        absolute_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let result = renderer.render(&[node], &mut fb, &damage);
    assert!(result.is_ok(), "render LockScreen node should succeed");
    assert_ne!(
        fb.pixels(),
        &before[..],
        "LockScreen should modify pixels (backdrop blur + dark tint)"
    );
}

#[test]
fn render_classifies_cursor_and_surface_damage() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(128, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    damage.add(DamageTile {
        x: 1,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let cursor = FlatNode {
        id: 100,
        kind: SceneNodeKind::Cursor {
            shape: liquide_compositor::scene::CursorShape::Arrow,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 24.0, 24.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let surface = FlatNode {
        id: 101,
        kind: SceneNodeKind::Surface {
            surface_id: 1,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(64.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 1,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let classified = renderer
        .render(&[cursor, surface], &mut fb, &damage)
        .unwrap();
    let classes: std::collections::HashMap<(u32, u32), DamageClass> = classified
        .into_iter()
        .map(|tile| ((tile.x, tile.y), tile.class))
        .collect();

    assert_eq!(classes.get(&(0, 0)), Some(&DamageClass::CursorOnly));
    assert_eq!(classes.get(&(1, 0)), Some(&DamageClass::BitmapRegion));
}

#[test]
fn render_text_damage_overrides_bitmap_damage_on_same_tile() {
    let mut renderer = SoftwareRenderer::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });

    let surface = FlatNode {
        id: 200,
        kind: SceneNodeKind::Surface {
            surface_id: 2,
            buffer: None,
        }
        .into(),
        absolute_bounds: Rect::new(0.0, 0.0, 64.0, 64.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };
    let caret = FlatNode {
        id: 201,
        kind: SceneNodeKind::TextCaret {
            color: Color::WHITE,
            width: 2.0,
        }
        .into(),
        absolute_bounds: Rect::new(8.0, 8.0, 2.0, 24.0),
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 1,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    };

    let classified = renderer
        .render(&[surface, caret], &mut fb, &damage)
        .unwrap();
    assert_eq!(classified.len(), 1);
    assert_eq!(classified[0].class, DamageClass::TextGlyph);
}

// ── word-break / text-emphasis primitive tests ─────────────────────────
//
// These use a renderer with synthetically-inserted glyphs (no system font
// dependency) so wrapping/emphasis behaviour is deterministic. An empty
// font_family with weight 400, upright, yields the `compute_font_id` value used
// by the text path (the single source of truth for atlas keying) and
// glyph_height = 16, and suppresses the common-glyph prewarm path. We derive the
// id via the same function the renderer uses so the inserted glyphs are found.

const TEST_SIZE_PX: u16 = 16;

fn test_font_id() -> u32 {
    crate::renderer::compute_font_id("", 400, false)
}

/// Insert a solid square glyph of the given logical width/height with a known
/// advance into the atlas, so text layout is fully deterministic.
fn insert_block_glyph(renderer: &mut SoftwareRenderer, ch: char, advance: f32) {
    let w = 10u32;
    let h = 12u32;
    let bitmap = vec![255u8; (w * h) as usize];
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: test_font_id(),
                glyph_id: ch as u32,
                size_px: TEST_SIZE_PX,
                subpixel: false,
            },
            &bitmap,
            &GlyphMetrics {
                width: w,
                height: h,
                bearing_x: 0,
                bearing_y: h as i32,
                advance,
            },
        )
        .unwrap();
}

/// Insert a small mark glyph at the emphasis size (round(16 * 0.5) = 8).
fn insert_mark_glyph(renderer: &mut SoftwareRenderer, ch: char) {
    let w = 4u32;
    let h = 4u32;
    let bitmap = vec![255u8; (w * h) as usize];
    renderer
        .glyph_atlas_mut()
        .insert(
            GlyphKey {
                font_id: test_font_id(),
                glyph_id: ch as u32,
                size_px: 8,
                subpixel: false,
            },
            &bitmap,
            &GlyphMetrics {
                width: w,
                height: h,
                bearing_x: 0,
                bearing_y: h as i32,
                advance: 4.0,
            },
        )
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn word_break_text_node(
    text: &str,
    bounds: Rect,
    word_break: liquide_compositor::scene::WordBreak,
    white_space: u8,
    text_emphasis: Option<liquide_compositor::scene::TextEmphasis>,
) -> FlatNode {
    FlatNode {
        id: 42,
        kind: SceneNodeKind::Text {
            text: text.to_string(),
            color: Color::WHITE,
            scale: 1,
            font_family: String::new(),
            font_size: 0.0,
            font_weight: 400,
            font_style_italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 14.0,
            text_align: 0,
            text_transform: 0,
            text_overflow: 0,
            white_space,
            word_break,
            text_indent: 0.0,
            text_decoration: None,
            text_shadows: Vec::new(),
            text_emphasis,
        }
        .into(),
        absolute_bounds: bounds,
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 0,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

fn render_node_to_fb(renderer: &mut SoftwareRenderer, node: FlatNode, w: u32, h: u32) -> FrameBuffer {
    let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
    let mut damage = DamageSet::new(64);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::TextGlyph,
    });
    renderer.render(&[node], &mut fb, &damage).unwrap();
    fb
}

/// Count rows (scanlines) that contain at least one non-transparent pixel.
fn nonempty_rows(fb: &FrameBuffer) -> u32 {
    let mut rows = 0;
    for y in 0..fb.height {
        let mut any = false;
        for x in 0..fb.width {
            if fb.get_pixel(x, y).a > 0 {
                any = true;
                break;
            }
        }
        if any {
            rows += 1;
        }
    }
    rows
}

fn nonempty_pixels(fb: &FrameBuffer) -> u32 {
    let mut n = 0;
    for y in 0..fb.height {
        for x in 0..fb.width {
            if fb.get_pixel(x, y).a > 0 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn word_break_break_all_wraps_long_word_normal_does_not() {
    use liquide_compositor::scene::WordBreak;
    // A single long unbreakable word, in a box only ~3 glyphs wide.
    // advance 10 → box width 35 fits ~3 chars; the 8-char word overflows.
    let word = "AAAAAAAA";
    let bounds = Rect::new(0.0, 0.0, 35.0, 120.0);

    let mut normal = SoftwareRenderer::new();
    insert_block_glyph(&mut normal, 'A', 10.0);
    let fb_normal = render_node_to_fb(
        &mut normal,
        word_break_text_node(word, bounds, WordBreak::Normal, 0, None),
        64,
        128,
    );

    let mut break_all = SoftwareRenderer::new();
    insert_block_glyph(&mut break_all, 'A', 10.0);
    let fb_break = render_node_to_fb(
        &mut break_all,
        word_break_text_node(word, bounds, WordBreak::BreakAll, 0, None),
        64,
        128,
    );

    let rows_normal = nonempty_rows(&fb_normal);
    let rows_break = nonempty_rows(&fb_break);

    // Normal: one overflowing line (single glyph row of height ~12).
    // break-all: the word is split across multiple lines → taller footprint.
    assert!(
        rows_break > rows_normal,
        "break-all should wrap the long word onto more lines \
         (break-all rows={rows_break}, normal rows={rows_normal})"
    );
}

#[test]
fn text_emphasis_renders_extra_marks() {
    use liquide_compositor::scene::{TextEmphasis, TextEmphasisPosition, WordBreak};
    let text = "AAA";
    let bounds = Rect::new(0.0, 20.0, 200.0, 40.0);

    let mut plain = SoftwareRenderer::new();
    insert_block_glyph(&mut plain, 'A', 12.0);
    let fb_plain = render_node_to_fb(
        &mut plain,
        word_break_text_node(text, bounds, WordBreak::Normal, 1, None),
        256,
        80,
    );

    let mut emphasized = SoftwareRenderer::new();
    insert_block_glyph(&mut emphasized, 'A', 12.0);
    insert_mark_glyph(&mut emphasized, '•');
    let emph = TextEmphasis {
        mark: "•".to_string(),
        color: None,
        position: TextEmphasisPosition::Over,
    };
    let fb_emph = render_node_to_fb(
        &mut emphasized,
        word_break_text_node(text, bounds, WordBreak::Normal, 1, Some(emph)),
        256,
        80,
    );

    let plain_px = nonempty_pixels(&fb_plain);
    let emph_px = nonempty_pixels(&fb_emph);
    assert!(
        emph_px > plain_px,
        "text-emphasis should draw additional mark pixels \
         (emphasized={emph_px}, plain={plain_px})"
    );
}

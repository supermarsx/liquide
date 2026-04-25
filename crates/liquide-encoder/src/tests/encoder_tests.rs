use crate::bandwidth::BandwidthBudget;
use crate::encoder::*;
use crate::strategy::CompressionMethod;
use crate::tile::{TileConfig, TileEncoding};

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::pixel::PixelFormat;

fn patterned_framebuffer(width: u32, height: u32) -> FrameBuffer {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let seed = (x + y * width) as u8;
            pixels.extend_from_slice(&[
                seed,
                seed.wrapping_mul(3),
                seed.wrapping_mul(5),
                255,
            ]);
        }
    }

    FrameBuffer {
        memory: FrameMemory::Cpu(pixels),
        width,
        height,
        stride: width * 4,
        format: PixelFormat::Bgra8,
    }
}

#[test]
fn t16_encoder_default_entry_point_matches_no_pressure_hint() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut plain = TileEncoder::new(4, 4, config.clone());
    let mut hinted = TileEncoder::new(4, 4, config);
    let fb = patterned_framebuffer(4, 4);
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::BitmapRegion,
    }];

    let plain_batch = plain.encode_frame(&fb, &damage).unwrap();
    let hinted_batch = hinted.encode_frame_with_budget_hint(&fb, &damage, None).unwrap();

    assert_eq!(plain_batch.tiles.len(), 1);
    assert_eq!(plain_batch.tiles[0].encoding, hinted_batch.tiles[0].encoding);
    assert_eq!(plain_batch.tiles[0].compression, hinted_batch.tiles[0].compression);
}

#[test]
fn t16_encoder_budget_hint_switches_bitmap_tiles_to_lz4() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut normal = TileEncoder::new(4, 4, config.clone());
    let mut pressured = TileEncoder::new(4, 4, config);
    let fb = patterned_framebuffer(4, 4);
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::BitmapRegion,
    }];
    let mut budget = BandwidthBudget::new(64, 0.1);
    budget.observe(128);

    let normal_batch = normal.encode_frame_with_budget_hint(&fb, &damage, None).unwrap();
    let pressured_batch = pressured
        .encode_frame_raw_with_budget_hint(
            fb.pixels(),
            fb.width,
            fb.height,
            fb.stride,
            &damage,
            Some(&budget),
        )
        .unwrap();

    assert!(matches!(normal_batch.tiles[0].compression, CompressionMethod::Zstd { .. }));
    assert_eq!(pressured_batch.tiles[0].compression, CompressionMethod::Lz4);
}

#[test]
fn encoder_creates() {
    let enc = TileEncoder::new(1920, 1080, TileConfig::default());
    assert_eq!(enc.grid().cols, 30);
    assert_eq!(enc.grid().rows, 17);
    assert_eq!(enc.sequence(), 0);
}

#[test]
fn encoder_encode_solid_frame() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    // Create a solid-color frame buffer
    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(255, 0, 0, 255));

    let damage = vec![
        DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 1,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
    ];

    let batch = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch.sequence, 1);
    assert_eq!(batch.tiles.len(), 2);

    // Solid tiles should be detected
    for tile in &batch.tiles {
        assert_eq!(tile.encoding, TileEncoding::Solid);
        assert_eq!(tile.payload.len(), 4);
    }
}

#[test]
fn encoder_skip_unchanged() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(100, 100, 100, 255));

    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];

    // First frame: encoded as solid
    let batch1 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch1.tiles.len(), 1);

    // Second frame: same data → skip
    let batch2 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch2.tiles.len(), 1);
    assert_eq!(batch2.tiles[0].encoding, TileEncoding::Skip);
}

#[test]
fn encoder_delta_on_change() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);

    // Frame 1: all zeros
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];
    let _ = enc.encode_frame(&fb, &damage).unwrap();

    // Frame 2: change one pixel
    fb.set_pixel(0, 0, liquide_compositor::pixel::Color::new(1, 2, 3, 255));

    let batch = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch.tiles.len(), 1);
    // Small change → delta encoding
    assert_eq!(batch.tiles[0].encoding, TileEncoding::Delta);
}

#[test]
fn encoder_resize_clears_state() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    // Encode one frame so there is cached state
    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(50, 60, 70, 255));
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];
    let _ = enc.encode_frame(&fb, &damage).unwrap();

    // Resize to a different resolution
    enc.resize(16, 16);
    assert_eq!(enc.grid().cols, 4); // 16 / 4
    assert_eq!(enc.grid().rows, 4);

    // Encode a frame on the new size — should work cleanly
    let fb2 = liquide_compositor::framebuffer::FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let damage2 = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];
    let batch = enc.encode_frame(&fb2, &damage2).unwrap();
    assert_eq!(batch.tiles.len(), 1);
}

#[test]
fn encoder_sequence_increments() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];

    assert_eq!(enc.sequence(), 0);

    let batch1 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch1.sequence, 1);
    assert_eq!(enc.sequence(), 1);

    let batch2 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch2.sequence, 2);
    assert_eq!(enc.sequence(), 2);

    let batch3 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch3.sequence, 3);
}

#[test]
fn encoder_second_frame_uses_skip() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(200, 100, 50, 255));

    // Damage all 4 tiles (2x2 grid)
    let damage = vec![
        DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 1,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 0,
            y: 1,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 1,
            y: 1,
            class: DamageClass::UiPrimitive,
        },
    ];

    // First frame: tiles are encoded (solid)
    let batch1 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch1.tiles.len(), 4);
    for t in &batch1.tiles {
        assert_ne!(t.encoding, TileEncoding::Skip);
    }

    // Second frame: same content so all tiles should be Skip
    let batch2 = enc.encode_frame(&fb, &damage).unwrap();
    assert_eq!(batch2.tiles.len(), 4);
    for t in &batch2.tiles {
        assert_eq!(
            t.encoding,
            TileEncoding::Skip,
            "tile ({},{}) should be Skip",
            t.tx,
            t.ty
        );
    }
}

#[test]
fn encoder_frame_stats_none_before_encoding() {
    let enc = TileEncoder::new(
        8,
        8,
        TileConfig {
            tile_size: 4,
            bpp: 4,
        },
    );
    assert!(enc.frame_stats().is_none());
}

#[test]
fn encoder_frame_stats_populated_after_encoding() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(255, 0, 0, 255));

    let damage = vec![
        DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 1,
            y: 0,
            class: DamageClass::UiPrimitive,
        },
    ];

    let _ = enc.encode_frame(&fb, &damage).unwrap();

    let stats = enc
        .frame_stats()
        .expect("frame_stats should be Some after encoding");
    assert!(stats.tiles_encoded > 0);
    assert!(stats.encode_time_us > 0 || stats.tiles_encoded > 0);
}

#[test]
fn encoder_frame_stats_updates_each_frame() {
    let config = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(50, 50, 50, 255));

    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];

    // Frame 1
    let _ = enc.encode_frame(&fb, &damage).unwrap();
    let stats1_encoded = enc.frame_stats().unwrap().tiles_encoded;

    // Frame 2: same data → skip → 0 tiles encoded
    let _ = enc.encode_frame(&fb, &damage).unwrap();
    let stats2_encoded = enc.frame_stats().unwrap().tiles_encoded;

    assert!(stats1_encoded > 0, "first frame should encode tiles");
    assert_eq!(stats2_encoded, 0, "second frame should skip all tiles");
}

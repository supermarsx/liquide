use crate::encoder::*;
use crate::tile::{TileConfig, TileEncoding};

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::pixel::PixelFormat;

#[test]
fn encoder_creates() {
    let enc = TileEncoder::new(1920, 1080, TileConfig::default());
    assert_eq!(enc.grid().cols, 30);
    assert_eq!(enc.grid().rows, 17);
    assert_eq!(enc.sequence(), 0);
}

#[test]
fn encoder_encode_solid_frame() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    // Create a solid-color frame buffer
    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(255, 0, 0, 255));

    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
        DamageTile { x: 1, y: 0, class: DamageClass::UiPrimitive },
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
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(100, 100, 100, 255));

    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
    ];

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
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);

    // Frame 1: all zeros
    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
    ];
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
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    // Encode one frame so there is cached state
    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(50, 60, 70, 255));
    let damage = vec![DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive }];
    let _ = enc.encode_frame(&fb, &damage).unwrap();

    // Resize to a different resolution
    enc.resize(16, 16);
    assert_eq!(enc.grid().cols, 4); // 16 / 4
    assert_eq!(enc.grid().rows, 4);

    // Encode a frame on the new size — should work cleanly
    let fb2 = liquide_compositor::framebuffer::FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let damage2 = vec![DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive }];
    let batch = enc.encode_frame(&fb2, &damage2).unwrap();
    assert_eq!(batch.tiles.len(), 1);
}

#[test]
fn encoder_sequence_increments() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    let damage = vec![DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive }];

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
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(200, 100, 50, 255));

    // Damage all 4 tiles (2x2 grid)
    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
        DamageTile { x: 1, y: 0, class: DamageClass::UiPrimitive },
        DamageTile { x: 0, y: 1, class: DamageClass::UiPrimitive },
        DamageTile { x: 1, y: 1, class: DamageClass::UiPrimitive },
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
        assert_eq!(t.encoding, TileEncoding::Skip, "tile ({},{}) should be Skip", t.tx, t.ty);
    }
}

#[test]
fn encoder_frame_stats_none_before_encoding() {
    let enc = TileEncoder::new(8, 8, TileConfig { tile_size: 4, bpp: 4 });
    assert!(enc.frame_stats().is_none());
}

#[test]
fn encoder_frame_stats_populated_after_encoding() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(255, 0, 0, 255));

    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
        DamageTile { x: 1, y: 0, class: DamageClass::UiPrimitive },
    ];

    let _ = enc.encode_frame(&fb, &damage).unwrap();

    let stats = enc.frame_stats().expect("frame_stats should be Some after encoding");
    assert!(stats.tiles_encoded > 0);
    assert!(stats.encode_time_us > 0 || stats.tiles_encoded > 0);
}

#[test]
fn encoder_frame_stats_updates_each_frame() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut enc = TileEncoder::new(8, 8, config);

    let mut fb = liquide_compositor::framebuffer::FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    fb.clear(liquide_compositor::pixel::Color::new(50, 50, 50, 255));

    let damage = vec![
        DamageTile { x: 0, y: 0, class: DamageClass::UiPrimitive },
    ];

    // Frame 1
    let _ = enc.encode_frame(&fb, &damage).unwrap();
    let stats1_encoded = enc.frame_stats().unwrap().tiles_encoded;

    // Frame 2: same data → skip → 0 tiles encoded
    let _ = enc.encode_frame(&fb, &damage).unwrap();
    let stats2_encoded = enc.frame_stats().unwrap().tiles_encoded;

    assert!(stats1_encoded > 0, "first frame should encode tiles");
    assert_eq!(stats2_encoded, 0, "second frame should skip all tiles");
}

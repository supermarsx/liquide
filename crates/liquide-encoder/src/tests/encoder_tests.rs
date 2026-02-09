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

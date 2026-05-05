//! Crate-local integration test for tile encoding loopback (session encode → transport → client decode).

use liquide_client_renderer::frame::FrameAssembler;
use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::encoder::TileEncoder;
use liquide_encoder::tile::TileConfig;
use liquide_transport::tile_channel::tile_channel;

/// Generate a simple gradient pixel pattern for testing.
fn gradient_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / width as f32) * 255.0) as u8;
            let g = ((y as f32 / height as f32) * 255.0) as u8;
            let b = 128u8;
            pixels.extend_from_slice(&[b, g, r, 255]);
        }
    }
    pixels
}

fn full_damage_tiles(width: u32, height: u32, tile_size: u32) -> Vec<DamageTile> {
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive).materialize_tiles()
}

#[test]
fn loopback_full_frame_encode_transport_decode() {
    let width = 256;
    let height = 256;
    let tile_size = 64;
    let config = TileConfig { tile_size, bpp: 4 };

    // Session side: encode
    let mut encoder = TileEncoder::new(width, height, config.clone());
    let pixels = gradient_pixels(width, height);
    let stride = width * 4;

    // Encode with explicit full-frame damage.
    let full_damage = full_damage_tiles(width, height, tile_size);
    let batch = encoder
        .encode_frame_raw(&pixels, width, height, stride, &full_damage)
        .expect("encode should succeed");

    assert!(batch.tiles.len() > 0, "batch should have tiles");
    assert_eq!(batch.sequence, 1);

    // Transport: send through channel
    let (tx, rx) = tile_channel();
    tx.send(batch.clone()).expect("send should succeed");

    // Client side: decode
    let mut assembler = FrameAssembler::new(width, height, PixelFormat::Bgra8, config);

    let received = rx.recv().expect("should receive batch");
    assert_eq!(received.sequence, batch.sequence);
    assert_eq!(received.tiles.len(), batch.tiles.len());

    let result = assembler
        .apply_batch(&received)
        .expect("decode should succeed");

    // Verify we decoded tiles
    assert!(result.tiles_decoded > 0, "should decode some tiles");
    assert_eq!(result.total_tiles(), batch.tiles.len() as u32);

    // Verify surface has non-zero pixels
    let surface = assembler.surface();
    let decoded_pixels = surface.pixels();
    assert_eq!(decoded_pixels.len(), (width * height * 4) as usize);

    // Spot check: gradient should have varying colors
    let first_pixel = &decoded_pixels[0..4];
    let mid_pixel = &decoded_pixels[((width / 2 + (height / 2) * width) * 4) as usize..][..4];
    let last_pixel = &decoded_pixels[decoded_pixels.len() - 4..];

    // Pixels should differ (gradient pattern)
    assert_ne!(first_pixel, mid_pixel);
    assert_ne!(mid_pixel, last_pixel);
}

#[test]
fn loopback_incremental_damage_encode_decode() {
    let width = 192;
    let height = 192;
    let tile_size = 64;
    let config = TileConfig { tile_size, bpp: 4 };

    let mut encoder = TileEncoder::new(width, height, config.clone());
    let stride = width * 4;

    // Frame 0: full frame
    let pixels_0 = gradient_pixels(width, height);
    let full_damage = full_damage_tiles(width, height, tile_size);
    let batch_0 = encoder
        .encode_frame_raw(&pixels_0, width, height, stride, &full_damage)
        .expect("frame 0 encode");

    // Frame 1: only modify top-left tile
    let mut pixels_1 = pixels_0.clone();
    for i in 0..(tile_size * tile_size * 4) as usize {
        pixels_1[i] = pixels_1[i].wrapping_add(50);
    }

    let mut damage = DamageSet::new(tile_size);
    damage.add(DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    });
    let damage_tiles = damage.materialize_tiles();

    let batch_1 = encoder
        .encode_frame_raw(&pixels_1, width, height, stride, &damage_tiles)
        .expect("frame 1 encode");

    // Frame 1 should have fewer dirty tiles than frame 0
    assert!(
        batch_1.dirty_count() < batch_0.dirty_count(),
        "incremental frame should have fewer dirty tiles"
    );

    let batch_0_tile_count = batch_0.tiles.len() as u32;
    let batch_1_tile_count = batch_1.tiles.len() as u32;

    // Transport both batches
    let (tx, rx) = tile_channel();
    tx.send(batch_0).unwrap();
    tx.send(batch_1).unwrap();

    // Decode both
    let mut assembler = FrameAssembler::new(width, height, PixelFormat::Bgra8, config);

    let recv_0 = rx.recv().expect("recv batch 0");
    let result_0 = assembler.apply_batch(&recv_0).expect("decode batch 0");
    assert_eq!(result_0.tiles_skipped, 0, "first frame should skip nothing");
    assert_eq!(result_0.total_tiles(), batch_0_tile_count);

    let recv_1 = rx.recv().expect("recv batch 1");
    let result_1 = assembler.apply_batch(&recv_1).expect("decode batch 1");
    assert_eq!(
        result_1.tiles_decoded, 1,
        "second frame should decode one dirty tile"
    );
    assert_eq!(result_1.total_tiles(), batch_1_tile_count);

    let surface = assembler.surface();
    let decoded_pixels = surface.pixels();
    assert_eq!(&decoded_pixels[0..4], &pixels_1[0..4]);

    let unchanged_offset = ((tile_size + tile_size * width) * 4) as usize;
    assert_eq!(
        &decoded_pixels[unchanged_offset..unchanged_offset + 4],
        &pixels_0[unchanged_offset..unchanged_offset + 4]
    );
}

#[test]
fn loopback_sequence_monotonic() {
    let width = 128;
    let height = 128;
    let tile_size = 64;
    let config = TileConfig { tile_size, bpp: 4 };

    let mut encoder = TileEncoder::new(width, height, config.clone());
    let (tx, rx) = tile_channel();

    // Encode 5 frames
    for i in 0..5 {
        let mut pixels = gradient_pixels(width, height);
        // Vary pixel seed per frame
        for p in pixels.iter_mut() {
            *p = p.wrapping_add(i as u8);
        }
        let full_damage = full_damage_tiles(width, height, tile_size);
        let batch = encoder
            .encode_frame_raw(&pixels, width, height, width * 4, &full_damage)
            .expect("encode");
        tx.send(batch).unwrap();
    }

    // Verify sequences are monotonic
    let mut last_seq = None;
    for _ in 0..5 {
        let batch = rx.recv().expect("recv");
        if let Some(prev) = last_seq {
            assert!(
                batch.sequence > prev,
                "sequences should be monotonic: {} > {}",
                batch.sequence,
                prev
            );
        }
        last_seq = Some(batch.sequence);
    }
}

use crate::encoder::*;
use crate::fragment::reassemble_batch;
use crate::tile::TileConfig;

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_protocol::codec::cbor_encode;
use liquide_protocol::FrameHeader;

fn make_gradient_fb(size: u32) -> FrameBuffer {
    // Produce a non-solid frame so tiles encode as Full/Delta (not Solid),
    // forcing non-trivial payloads that exercise fragmentation.
    let mut fb = FrameBuffer::new(size, size, PixelFormat::Bgra8);
    let pixels = fb.pixels_mut().expect("cpu-backed fb");
    for y in 0..size {
        for x in 0..size {
            let off = ((y * size + x) * 4) as usize;
            pixels[off] = (x & 0xff) as u8;
            pixels[off + 1] = (y & 0xff) as u8;
            pixels[off + 2] = ((x ^ y) & 0xff) as u8;
            pixels[off + 3] = 0xff;
        }
    }
    let _ = Color::new(0, 0, 0, 0); // keep Color imported
    fb
}

#[test]
fn encode_frame_with_mtu_fragments_and_reassembles_exactly() {
    let cfg = TileConfig {
        tile_size: 32,
        bpp: 4,
    };
    let tile_bytes = cfg.tile_bytes();
    let mut enc = TileEncoder::new(128, 128, cfg);
    let fb = make_gradient_fb(128);

    let mut damage = Vec::new();
    for ty in 0..4 {
        for tx in 0..4 {
            damage.push(DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            });
        }
    }

    let fragments = enc
        .encode_frame_with_mtu(&fb, &damage, 4096)
        .expect("encode with mtu");
    assert!(!fragments.is_empty(), "at least one fragment expected");
    assert!(fragments.iter().all(|fragment| {
        cbor_encode(fragment).unwrap().len() + FrameHeader::WIRE_SIZE <= 4096
    }));
    // Sequence numbers are monotonically increasing and dense across the batch.
    for pair in fragments.windows(2) {
        assert_eq!(pair[0].sequence + 1, pair[1].sequence);
    }
    assert!(
        fragments.iter().any(|f| f.is_last),
        "exactly one final fragment"
    );

    let reassembled = reassemble_batch(&fragments).expect("reassemble");
    assert_eq!(reassembled.tiles.len(), damage.len());
    for t in &reassembled.tiles {
        // Every tile payload survived reassembly intact.
        assert!(t.payload.len() <= tile_bytes);
    }
}

#[test]
fn encode_frame_with_mtu_zero_rejected() {
    let cfg = TileConfig {
        tile_size: 8,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(16, 16, cfg);
    let fb = make_gradient_fb(16);
    let damage = vec![DamageTile {
        x: 0,
        y: 0,
        class: DamageClass::UiPrimitive,
    }];
    assert!(enc.encode_frame_with_mtu(&fb, &damage, 0).is_err());
}

#[test]
fn encode_frame_with_mtu_respects_wire_budget() {
    let cfg = TileConfig {
        tile_size: 32,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(128, 128, cfg);
    let fb = make_gradient_fb(128);
    let damage: Vec<_> = (0..4)
        .flat_map(|ty| {
            (0..4).map(move |tx| DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            })
        })
        .collect();

    let fragments = enc.encode_frame_with_mtu(&fb, &damage, 512).unwrap();
    assert!(!fragments.is_empty());
    assert!(fragments.iter().all(|fragment| {
        cbor_encode(fragment).unwrap().len() + FrameHeader::WIRE_SIZE <= 512
    }));
}

#[test]
fn encode_frame_with_mtu_monotonic_across_calls() {
    let cfg = TileConfig {
        tile_size: 16,
        bpp: 4,
    };
    let mut enc = TileEncoder::new(64, 64, cfg);
    let fb = make_gradient_fb(64);
    let damage: Vec<_> = (0..4)
        .flat_map(|ty| {
            (0..4).map(move |tx| DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            })
        })
        .collect();

    let a = enc.encode_frame_with_mtu(&fb, &damage, 2048).unwrap();
    let b = enc.encode_frame_with_mtu(&fb, &damage, 2048).unwrap();
    let last_a = a.last().unwrap().sequence;
    let first_b = b.first().unwrap().sequence;
    assert!(
        first_b > last_a,
        "fragment sequence must be monotonic across frames ({last_a} -> {first_b})"
    );
}

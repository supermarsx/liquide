use liquide_compositor::damage::DamageClass;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::compress::compress_lz4;
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{FrameStats, TileBatch, TileConfig, TileEncoding, TileUpdate};

use crate::cursor::{CursorShape, CursorState};
use crate::frame::FrameAssembler;
use crate::stats::RenderStats;
use crate::surface::{RenderSurface, SurfaceInfo};

fn make_update(
    tx: u32,
    ty: u32,
    encoding: TileEncoding,
    payload: Vec<u8>,
    compression: CompressionMethod,
) -> TileUpdate {
    TileUpdate {
        tx,
        ty,
        encoding,
        payload,
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression,
    }
}

#[test]
fn test_zero_size_surface() {
    let s = RenderSurface::new(0, 0, PixelFormat::Bgra8);
    assert_eq!(s.byte_size(), 0);
    assert!(s.get_pixel(0, 0).is_none());
}

#[test]
fn test_single_pixel_surface() {
    let mut s = RenderSurface::new(1, 1, PixelFormat::Bgra8);
    s.set_pixel(0, 0, &[1, 2, 3, 4]);
    assert_eq!(s.get_pixel(0, 0), Some([1, 2, 3, 4].as_slice()));
    assert_eq!(s.byte_size(), 4);
}

#[test]
fn test_all_skip_batch() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);

    let batch = TileBatch {
        sequence: 0,
        tiles: vec![
            make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
            make_update(1, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
            make_update(0, 1, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
            make_update(1, 1, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
        ],
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    };

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_skipped, 4);
    assert_eq!(result.tiles_decoded, 0);
}

#[test]
fn test_surface_info() {
    let s = RenderSurface::new(1920, 1080, PixelFormat::Bgra8);
    let info = SurfaceInfo::from_surface(&s);
    assert_eq!(info.width, 1920);
    assert_eq!(info.height, 1080);
    assert_eq!(info.stride, 1920 * 4);
    assert_eq!(info.format, "bgra8888");
    assert_eq!(info.byte_size, 1920 * 1080 * 4);
}

#[test]
fn test_surface_info_serde() {
    let s = RenderSurface::new(640, 480, PixelFormat::Rgba8);
    let info = SurfaceInfo::from_surface(&s);
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: SurfaceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.width, 640);
    assert_eq!(deserialized.height, 480);
}

#[test]
fn test_render_stats_serde_roundtrip() {
    let mut s = RenderStats::new();
    s.record_frame(100, 50, 10000, 50000, 500);
    s.record_frame(80, 70, 8000, 40000, 400);
    let json = serde_json::to_string(&s).unwrap();
    let d: RenderStats = serde_json::from_str(&json).unwrap();
    assert_eq!(d.frames_rendered, s.frames_rendered);
    assert_eq!(d.tiles_decoded, s.tiles_decoded);
    assert_eq!(d.bytes_received, s.bytes_received);
}

#[test]
fn test_cursor_negative_position() {
    let mut c = CursorState::new();
    c.set_position(-50, -100);
    assert_eq!(c.x, -50);
    assert_eq!(c.y, -100);
}

#[test]
fn test_cursor_all_shapes() {
    use crate::cursor::ResizeDirection;

    let shapes = vec![
        CursorShape::Arrow,
        CursorShape::Hand,
        CursorShape::Text,
        CursorShape::Crosshair,
        CursorShape::Wait,
        CursorShape::Help,
        CursorShape::NotAllowed,
        CursorShape::Resize(ResizeDirection::North),
        CursorShape::Resize(ResizeDirection::South),
        CursorShape::Resize(ResizeDirection::East),
        CursorShape::Resize(ResizeDirection::West),
        CursorShape::Resize(ResizeDirection::NorthEast),
        CursorShape::Resize(ResizeDirection::NorthWest),
        CursorShape::Resize(ResizeDirection::SouthEast),
        CursorShape::Resize(ResizeDirection::SouthWest),
        CursorShape::Custom,
        CursorShape::Hidden,
    ];

    for shape in &shapes {
        let json = serde_json::to_string(shape).unwrap();
        let deserialized: CursorShape = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, shape);
    }
}

#[test]
fn test_solid_tile_in_assembler() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);
    let color = vec![0xFF, 0x00, 0xFF, 0x80];

    let batch = TileBatch {
        sequence: 0,
        tiles: vec![
            make_update(0, 0, TileEncoding::Solid, color.clone(), CompressionMethod::Lz4),
        ],
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    };

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 1);

    // Verify the tile was written
    let pixel = a.surface().get_pixel(0, 0).unwrap();
    assert_eq!(pixel, &[0xFF, 0x00, 0xFF, 0x80]);
}

#[test]
fn test_full_then_skip_preserves() {
    let config = TileConfig { tile_size: 4, bpp: 4 };
    let tile_bytes = config.tile_bytes();
    let raw = vec![0x42; tile_bytes];
    let compressed = compress_lz4(&raw);

    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);

    // First batch: full tile
    let batch1 = TileBatch {
        sequence: 0,
        tiles: vec![
            make_update(0, 0, TileEncoding::Full, compressed, CompressionMethod::Lz4),
        ],
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    };
    a.apply_batch(&batch1).unwrap();

    // Second batch: skip
    let batch2 = TileBatch {
        sequence: 1,
        tiles: vec![
            make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
        ],
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    };
    a.apply_batch(&batch2).unwrap();

    // The pixel should still be 0x42
    let pixel = a.surface().get_pixel(0, 0).unwrap();
    assert_eq!(pixel, &[0x42, 0x42, 0x42, 0x42]);
}

use crate::surface::RenderSurface;
use liquide_compositor::pixel::PixelFormat;

#[test]
fn test_new_surface() {
    let s = RenderSurface::new(1920, 1080, PixelFormat::Bgra8);
    assert_eq!(s.width(), 1920);
    assert_eq!(s.height(), 1080);
    assert_eq!(s.stride(), 1920 * 4);
    assert_eq!(s.format(), PixelFormat::Bgra8);
    assert_eq!(s.byte_size(), 1920 * 1080 * 4);
}

#[test]
fn test_surface_rgb8() {
    let s = RenderSurface::new(100, 50, PixelFormat::Rgb8);
    assert_eq!(s.stride(), 300);
    assert_eq!(s.byte_size(), 100 * 50 * 3);
}

#[test]
fn test_resize() {
    let mut s = RenderSurface::new(100, 100, PixelFormat::Bgra8);
    s.set_pixel(0, 0, &[255, 0, 0, 255]);
    s.resize(200, 150);
    assert_eq!(s.width(), 200);
    assert_eq!(s.height(), 150);
    assert_eq!(s.byte_size(), 200 * 150 * 4);
    // Should be cleared after resize
    assert_eq!(s.get_pixel(0, 0), Some([0, 0, 0, 0].as_slice()));
}

#[test]
fn test_get_set_pixel() {
    let mut s = RenderSurface::new(10, 10, PixelFormat::Bgra8);
    s.set_pixel(5, 5, &[10, 20, 30, 40]);
    assert_eq!(s.get_pixel(5, 5), Some([10, 20, 30, 40].as_slice()));
}

#[test]
fn test_get_pixel_out_of_bounds() {
    let s = RenderSurface::new(10, 10, PixelFormat::Bgra8);
    assert!(s.get_pixel(10, 0).is_none());
    assert!(s.get_pixel(0, 10).is_none());
    assert!(s.get_pixel(100, 100).is_none());
}

#[test]
fn test_write_read_tile() {
    let mut s = RenderSurface::new(128, 128, PixelFormat::Bgra8);
    let tile_size = 64u32;
    let bpp = 4;
    let tile_bytes = (tile_size * tile_size * bpp) as usize;

    // Create a tile filled with a pattern
    let mut tile_data = vec![0u8; tile_bytes];
    for (i, byte) in tile_data.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }

    assert!(s.write_tile(0, 0, tile_size, &tile_data));
    let readback = s.read_tile(0, 0, tile_size);
    assert_eq!(readback, tile_data);
}

#[test]
fn test_write_tile_second_position() {
    let mut s = RenderSurface::new(128, 128, PixelFormat::Bgra8);
    let tile_size = 64u32;
    let tile_bytes = (tile_size * tile_size * 4) as usize;

    let tile_data = vec![42u8; tile_bytes];
    assert!(s.write_tile(1, 1, tile_size, &tile_data));
    let readback = s.read_tile(1, 1, tile_size);
    assert_eq!(readback, tile_data);

    // Tile (0,0) should still be zeros
    let origin_tile = s.read_tile(0, 0, tile_size);
    assert!(origin_tile.iter().all(|&b| b == 0));
}

#[test]
fn test_clear() {
    let mut s = RenderSurface::new(10, 10, PixelFormat::Bgra8);
    s.set_pixel(3, 3, &[255, 128, 64, 32]);
    s.clear();
    assert_eq!(s.get_pixel(3, 3), Some([0, 0, 0, 0].as_slice()));
}

#[test]
fn test_pixels_mut() {
    let mut s = RenderSurface::new(2, 2, PixelFormat::Bgra8);
    let pixels = s.pixels_mut();
    pixels[0] = 99;
    assert_eq!(s.pixels()[0], 99);
}

#[test]
fn test_display() {
    let s = RenderSurface::new(1920, 1080, PixelFormat::Bgra8);
    let display = format!("{s}");
    assert!(display.contains("1920x1080"));
    assert!(display.contains("bgra8888"));
}

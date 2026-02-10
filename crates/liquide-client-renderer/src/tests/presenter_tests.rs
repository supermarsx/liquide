use liquide_compositor::pixel::PixelFormat;

use crate::presenter::{BufferPresenter, NullPresenter, Presenter};
use crate::surface::RenderSurface;

#[test]
fn test_null_presenter() {
    let mut p = NullPresenter;
    let surface = RenderSurface::new(100, 100, PixelFormat::Bgra8);
    assert!(p.present(&surface).is_ok());
}

#[test]
fn test_null_presenter_supports_all_formats() {
    let p = NullPresenter;
    assert!(p.supports_format(PixelFormat::Bgra8));
    assert!(p.supports_format(PixelFormat::Rgba8));
    assert!(p.supports_format(PixelFormat::Rgb8));
    assert!(p.supports_format(PixelFormat::Rgb565));
}

#[test]
fn test_buffer_presenter_captures() {
    let mut p = BufferPresenter::new();
    assert_eq!(p.frame_count(), 0);
    assert!(p.buffer().is_empty());

    let mut surface = RenderSurface::new(10, 10, PixelFormat::Bgra8);
    surface.set_pixel(0, 0, &[255, 128, 64, 32]);
    p.present(&surface).unwrap();

    assert_eq!(p.frame_count(), 1);
    assert_eq!(p.width(), 10);
    assert_eq!(p.height(), 10);
    assert_eq!(p.buffer().len(), 10 * 10 * 4);
    assert_eq!(&p.buffer()[0..4], &[255, 128, 64, 32]);
}

#[test]
fn test_buffer_presenter_multiple_frames() {
    let mut p = BufferPresenter::new();

    let s1 = RenderSurface::new(10, 10, PixelFormat::Bgra8);
    p.present(&s1).unwrap();
    assert_eq!(p.frame_count(), 1);

    let s2 = RenderSurface::new(20, 20, PixelFormat::Bgra8);
    p.present(&s2).unwrap();
    assert_eq!(p.frame_count(), 2);
    assert_eq!(p.width(), 20);
    assert_eq!(p.height(), 20);
}

#[test]
fn test_buffer_presenter_default() {
    let p = BufferPresenter::default();
    assert_eq!(p.frame_count(), 0);
}

#[test]
fn test_null_presenter_display() {
    let p = NullPresenter;
    assert_eq!(format!("{p}"), "NullPresenter");
}

#[test]
fn test_buffer_presenter_display() {
    let mut p = BufferPresenter::new();
    let s = RenderSurface::new(640, 480, PixelFormat::Bgra8);
    p.present(&s).unwrap();
    let display = format!("{p}");
    assert!(display.contains("640x480"));
    assert!(display.contains("frames=1"));
}

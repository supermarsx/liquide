//! Per-monitor wallpaper management.
//!
//! Provides wallpaper mode computation (Fill, Fit, Stretch, Tile, Center, Span),
//! per-monitor wallpaper configuration, and slideshow support.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Wallpaper mode
// ---------------------------------------------------------------------------

/// How the wallpaper image is scaled/positioned on a monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WallpaperMode {
    /// Scale uniformly to fill the monitor; crop excess.
    Fill,
    /// Scale uniformly to fit inside the monitor; letterbox with background color.
    Fit,
    /// Stretch non-uniformly to exactly match the monitor dimensions.
    Stretch,
    /// Tile the image at its original size.
    Tile,
    /// Center the image at its original size; pad with background color.
    Center,
    /// Span a single image across all monitors (compute per-monitor crop).
    Span,
}

impl Default for WallpaperMode {
    fn default() -> Self {
        WallpaperMode::Fill
    }
}

// ---------------------------------------------------------------------------
// Wallpaper config
// ---------------------------------------------------------------------------

/// Wallpaper configuration for a single monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperConfig {
    /// Path to the wallpaper image file (or directory for slideshows).
    pub path: String,
    /// How the image is placed on the monitor.
    pub mode: WallpaperMode,
    /// Background / letterbox color as `(r, g, b)` (0-255).
    pub background_color: (u8, u8, u8),
    /// Optional slideshow settings. When `Some`, the `path` field is treated
    /// as a directory containing images.
    pub slideshow: Option<SlideshowConfig>,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            mode: WallpaperMode::Fill,
            background_color: (0, 0, 0),
            slideshow: None,
        }
    }
}

impl WallpaperConfig {
    /// Create a config for a single static wallpaper.
    pub fn new(path: impl Into<String>, mode: WallpaperMode) -> Self {
        Self {
            path: path.into(),
            mode,
            background_color: (0, 0, 0),
            slideshow: None,
        }
    }

    /// Create a slideshow config from a directory path.
    pub fn slideshow(
        directory: impl Into<String>,
        mode: WallpaperMode,
        interval_secs: u32,
        order: SlideshowOrder,
    ) -> Self {
        Self {
            path: directory.into(),
            mode,
            background_color: (0, 0, 0),
            slideshow: Some(SlideshowConfig {
                interval_secs,
                order,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Slideshow
// ---------------------------------------------------------------------------

/// Slideshow configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideshowConfig {
    /// Interval between image changes, in seconds.
    pub interval_secs: u32,
    /// Order in which images are cycled.
    pub order: SlideshowOrder,
}

/// Order of images in a slideshow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlideshowOrder {
    /// Cycle through images in alphabetical file-name order.
    Sequential,
    /// Pick a random image each time (no immediate repeats if possible).
    Random,
}

impl Default for SlideshowOrder {
    fn default() -> Self {
        SlideshowOrder::Sequential
    }
}

// ---------------------------------------------------------------------------
// Wallpaper transform computation
// ---------------------------------------------------------------------------

/// Source and destination rectangles describing how to blit a wallpaper image
/// onto a monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallpaperTransform {
    /// Region of the source image to sample: (x, y, width, height).
    pub src: (f64, f64, f64, f64),
    /// Region on the monitor to draw into: (x, y, width, height).
    pub dst: (f64, f64, f64, f64),
}

/// Compute the source/destination rectangles for placing an image on a monitor.
///
/// - `monitor_w`, `monitor_h`: monitor logical size in pixels.
/// - `image_w`, `image_h`: source image size in pixels.
/// - `mode`: placement mode.
///
/// For `Tile` mode, this returns a single tile placement at the origin;
/// the caller is responsible for repeating the blit across the monitor.
///
/// For `Span` mode, this function computes the same as `Fill` (the caller
/// should use a separate multi-monitor-aware span function).
pub fn compute_wallpaper_transform(
    monitor_w: u32,
    monitor_h: u32,
    image_w: u32,
    image_h: u32,
    mode: WallpaperMode,
) -> WallpaperTransform {
    let mw = monitor_w as f64;
    let mh = monitor_h as f64;
    let iw = image_w as f64;
    let ih = image_h as f64;

    if mw <= 0.0 || mh <= 0.0 || iw <= 0.0 || ih <= 0.0 {
        return WallpaperTransform {
            src: (0.0, 0.0, iw, ih),
            dst: (0.0, 0.0, mw, mh),
        };
    }

    match mode {
        WallpaperMode::Fill | WallpaperMode::Span => {
            // Scale to fill, crop centered.
            let scale = (mw / iw).max(mh / ih);
            let crop_w = mw / scale;
            let crop_h = mh / scale;
            let sx = (iw - crop_w) / 2.0;
            let sy = (ih - crop_h) / 2.0;
            WallpaperTransform {
                src: (sx, sy, crop_w, crop_h),
                dst: (0.0, 0.0, mw, mh),
            }
        }
        WallpaperMode::Fit => {
            // Scale to fit inside, letterbox.
            let scale = (mw / iw).min(mh / ih);
            let dw = iw * scale;
            let dh = ih * scale;
            let dx = (mw - dw) / 2.0;
            let dy = (mh - dh) / 2.0;
            WallpaperTransform {
                src: (0.0, 0.0, iw, ih),
                dst: (dx, dy, dw, dh),
            }
        }
        WallpaperMode::Stretch => WallpaperTransform {
            src: (0.0, 0.0, iw, ih),
            dst: (0.0, 0.0, mw, mh),
        },
        WallpaperMode::Tile => {
            // Single tile at origin, full source image, clipped to monitor.
            let dw = iw.min(mw);
            let dh = ih.min(mh);
            WallpaperTransform {
                src: (0.0, 0.0, dw, dh),
                dst: (0.0, 0.0, dw, dh),
            }
        }
        WallpaperMode::Center => {
            // Center at original size, clip if larger.
            let dx = (mw - iw) / 2.0;
            let dy = (mh - ih) / 2.0;

            if iw <= mw && ih <= mh {
                // Image fits entirely.
                WallpaperTransform {
                    src: (0.0, 0.0, iw, ih),
                    dst: (dx, dy, iw, ih),
                }
            } else {
                // Image larger than monitor — crop centered.
                let sx = if iw > mw { (iw - mw) / 2.0 } else { 0.0 };
                let sy = if ih > mh { (ih - mh) / 2.0 } else { 0.0 };
                let sw = iw.min(mw);
                let sh = ih.min(mh);
                let ddx = if iw > mw { 0.0 } else { dx };
                let ddy = if ih > mh { 0.0 } else { dy };
                WallpaperTransform {
                    src: (sx, sy, sw, sh),
                    dst: (ddx, ddy, sw, sh),
                }
            }
        }
    }
}

/// For `Span` mode across multiple monitors, compute the source crop for one
/// monitor given the total virtual desktop bounds and the image dimensions.
///
/// - `total_w`, `total_h`: bounding box of all monitors.
/// - `monitor_x`, `monitor_y`: this monitor's position in the virtual desktop.
/// - `monitor_w`, `monitor_h`: this monitor's logical size.
/// - `image_w`, `image_h`: source image dimensions.
pub fn compute_span_crop(
    total_w: u32,
    total_h: u32,
    monitor_x: i32,
    monitor_y: i32,
    monitor_w: u32,
    monitor_h: u32,
    image_w: u32,
    image_h: u32,
) -> WallpaperTransform {
    let tw = total_w as f64;
    let th = total_h as f64;
    let iw = image_w as f64;
    let ih = image_h as f64;
    let mw = monitor_w as f64;
    let mh = monitor_h as f64;

    if tw <= 0.0 || th <= 0.0 || iw <= 0.0 || ih <= 0.0 {
        return WallpaperTransform {
            src: (0.0, 0.0, iw, ih),
            dst: (0.0, 0.0, mw, mh),
        };
    }

    // Scale image to fill total desktop (same as Fill for total bounds).
    let scale = (tw / iw).max(th / ih);
    let scaled_w = iw * scale;
    let scaled_h = ih * scale;
    let offset_x = (tw - scaled_w) / 2.0;
    let offset_y = (th - scaled_h) / 2.0;

    // Map this monitor's rectangle back to image coordinates.
    let sx = (monitor_x as f64 - offset_x) / scale;
    let sy = (monitor_y as f64 - offset_y) / scale;
    let sw = mw / scale;
    let sh = mh / scale;

    WallpaperTransform {
        src: (sx.max(0.0), sy.max(0.0), sw.min(iw), sh.min(ih)),
        dst: (0.0, 0.0, mw, mh),
    }
}

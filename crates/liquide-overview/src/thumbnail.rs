//! Live window thumbnails — downscaled snapshots of window content for use in
//! the overview, taskbar previews, and expose views.

/// Unique identifier for a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThumbnailId(pub u64);

/// Quality level for thumbnail rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailQuality {
    /// Fast, lower quality (skip every other row in downscale).
    Low,
    /// Balanced bilinear downscale.
    Medium,
    /// High quality bilinear with 4-tap sampling.
    High,
}

/// Configuration for thumbnail generation and caching.
#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    /// Maximum width of generated thumbnails in pixels.
    pub max_width: u32,
    /// Maximum height of generated thumbnails in pixels.
    pub max_height: u32,
    /// How often to re-capture the window content (milliseconds).
    pub update_interval_ms: u64,
    /// Quality setting for the downscale algorithm.
    pub quality: ThumbnailQuality,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            max_width: 320,
            max_height: 240,
            update_interval_ms: 200,
            quality: ThumbnailQuality::Medium,
        }
    }
}

/// A live thumbnail of a window's content.
#[derive(Debug, Clone)]
pub struct Thumbnail {
    /// Unique thumbnail identifier.
    pub id: ThumbnailId,
    /// The window this thumbnail represents.
    pub source_window_id: u64,
    /// Current thumbnail width in pixels.
    pub width: u32,
    /// Current thumbnail height in pixels.
    pub height: u32,
    /// Scale factor from source window to thumbnail (0.0..=1.0).
    pub scale: f32,
    /// BGRA pixel data, or `None` if not yet captured.
    pub pixel_data: Option<Vec<u8>>,
    /// Timestamp (ms) when the pixel data was last updated.
    pub last_update_ms: u64,
    /// Whether the thumbnail content is likely out of date.
    pub is_stale: bool,
}

impl Thumbnail {
    /// Create a new thumbnail with no pixel data.
    pub fn new(
        id: ThumbnailId,
        source_window_id: u64,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Self {
        Self {
            id,
            source_window_id,
            width,
            height,
            scale,
            pixel_data: None,
            last_update_ms: 0,
            is_stale: true,
        }
    }

    /// Returns the expected byte length of the BGRA pixel buffer.
    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Mark the thumbnail as stale (needs re-capture).
    pub fn invalidate(&mut self) {
        self.is_stale = true;
    }

    /// Update pixel data and reset staleness.
    pub fn update_pixels(&mut self, data: Vec<u8>, now_ms: u64) {
        debug_assert_eq!(data.len(), self.byte_len());
        self.pixel_data = Some(data);
        self.last_update_ms = now_ms;
        self.is_stale = false;
    }

    /// Resize the thumbnail when the source window geometry changes.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        if self.width == width
            && self.height == height
            && (self.scale - scale).abs() <= f32::EPSILON
        {
            return;
        }

        self.width = width;
        self.height = height;
        self.scale = scale;
        self.pixel_data = None;
        self.last_update_ms = 0;
        self.is_stale = true;
    }
}

/// Compute the thumbnail dimensions that fit within `(max_w, max_h)` while
/// preserving the aspect ratio of `(source_w, source_h)`.
///
/// Returns `(width, height)`, both >= 1.
pub fn compute_thumbnail_size(source_w: u32, source_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let sw = source_w.max(1);
    let sh = source_h.max(1);
    let mw = max_w.max(1);
    let mh = max_h.max(1);

    let scale_w = mw as f64 / sw as f64;
    let scale_h = mh as f64 / sh as f64;
    let scale = scale_w.min(scale_h).min(1.0); // never upscale

    let w = ((sw as f64 * scale).round() as u32).max(1);
    let h = ((sh as f64 * scale).round() as u32).max(1);
    (w, h)
}

/// Downscale a BGRA image from `(src_w, src_h)` to `(dst_w, dst_h)` using
/// bilinear interpolation.
///
/// `src` must contain exactly `src_w * src_h * 4` bytes in BGRA order.
/// Returns a new buffer of `dst_w * dst_h * 4` bytes.
pub fn downscale_bilinear(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let sw = src_w.max(1) as usize;
    let sh = src_h.max(1) as usize;
    let dw = dst_w.max(1) as usize;
    let dh = dst_h.max(1) as usize;

    debug_assert_eq!(src.len(), sw * sh * 4);

    let mut dst = vec![0u8; dw * dh * 4];

    let x_ratio = if dw > 1 {
        (sw - 1) as f64 / (dw - 1) as f64
    } else {
        0.0
    };
    let y_ratio = if dh > 1 {
        (sh - 1) as f64 / (dh - 1) as f64
    } else {
        0.0
    };

    for dy in 0..dh {
        let src_y = dy as f64 * y_ratio;
        let y0 = src_y as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = src_y - y0 as f64;

        for dx in 0..dw {
            let src_x = dx as f64 * x_ratio;
            let x0 = src_x as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = src_x - x0 as f64;

            let i00 = (y0 * sw + x0) * 4;
            let i10 = (y0 * sw + x1) * 4;
            let i01 = (y1 * sw + x0) * 4;
            let i11 = (y1 * sw + x1) * 4;

            let dst_i = (dy * dw + dx) * 4;
            for c in 0..4 {
                let top = src[i00 + c] as f64 * (1.0 - fx) + src[i10 + c] as f64 * fx;
                let bot = src[i01 + c] as f64 * (1.0 - fx) + src[i11 + c] as f64 * fx;
                let val = top * (1.0 - fy) + bot * fy;
                dst[dst_i + c] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

fn downscale_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let sw = src_w.max(1) as usize;
    let sh = src_h.max(1) as usize;
    let dw = dst_w.max(1) as usize;
    let dh = dst_h.max(1) as usize;
    let mut dst = vec![0u8; dw * dh * 4];

    let scale_x = sw as f64 / dw as f64;
    let scale_y = sh as f64 / dh as f64;

    for dy in 0..dh {
        let src_y = ((dy as f64 + 0.5) * scale_y - 0.5)
            .round()
            .clamp(0.0, (sh - 1) as f64) as usize;
        for dx in 0..dw {
            let src_x = ((dx as f64 + 0.5) * scale_x - 0.5)
                .round()
                .clamp(0.0, (sw - 1) as f64) as usize;
            let src_i = (src_y * sw + src_x) * 4;
            let dst_i = (dy * dw + dx) * 4;
            dst[dst_i..dst_i + 4].copy_from_slice(&src[src_i..src_i + 4]);
        }
    }

    dst
}

fn sample_bilinear(src: &[u8], sw: usize, sh: usize, x: f64, y: f64) -> [f64; 4] {
    let x = x.clamp(0.0, (sw - 1) as f64);
    let y = y.clamp(0.0, (sh - 1) as f64);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(sw - 1);
    let y1 = (y0 + 1).min(sh - 1);
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let i00 = (y0 * sw + x0) * 4;
    let i10 = (y0 * sw + x1) * 4;
    let i01 = (y1 * sw + x0) * 4;
    let i11 = (y1 * sw + x1) * 4;

    let mut sample = [0.0; 4];
    for channel in 0..4 {
        let top = src[i00 + channel] as f64 * (1.0 - fx) + src[i10 + channel] as f64 * fx;
        let bottom = src[i01 + channel] as f64 * (1.0 - fx) + src[i11 + channel] as f64 * fx;
        sample[channel] = top * (1.0 - fy) + bottom * fy;
    }

    sample
}

fn downscale_4tap(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let sw = src_w.max(1) as usize;
    let sh = src_h.max(1) as usize;
    let dw = dst_w.max(1) as usize;
    let dh = dst_h.max(1) as usize;
    let mut dst = vec![0u8; dw * dh * 4];

    let scale_x = sw as f64 / dw as f64;
    let scale_y = sh as f64 / dh as f64;
    let tap_offset_x = (scale_x / 4.0).max(0.25);
    let tap_offset_y = (scale_y / 4.0).max(0.25);

    for dy in 0..dh {
        let center_y = (dy as f64 + 0.5) * scale_y - 0.5;
        for dx in 0..dw {
            let center_x = (dx as f64 + 0.5) * scale_x - 0.5;
            let taps = [
                sample_bilinear(
                    src,
                    sw,
                    sh,
                    center_x - tap_offset_x,
                    center_y - tap_offset_y,
                ),
                sample_bilinear(
                    src,
                    sw,
                    sh,
                    center_x + tap_offset_x,
                    center_y - tap_offset_y,
                ),
                sample_bilinear(
                    src,
                    sw,
                    sh,
                    center_x - tap_offset_x,
                    center_y + tap_offset_y,
                ),
                sample_bilinear(
                    src,
                    sw,
                    sh,
                    center_x + tap_offset_x,
                    center_y + tap_offset_y,
                ),
            ];

            let dst_i = (dy * dw + dx) * 4;
            for channel in 0..4 {
                let value = taps.iter().map(|tap| tap[channel]).sum::<f64>() / taps.len() as f64;
                dst[dst_i + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

fn downscale_with_quality(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    quality: ThumbnailQuality,
) -> Vec<u8> {
    match quality {
        ThumbnailQuality::Low => downscale_nearest(src, src_w, src_h, dst_w, dst_h),
        ThumbnailQuality::Medium => downscale_bilinear(src, src_w, src_h, dst_w, dst_h),
        ThumbnailQuality::High => downscale_4tap(src, src_w, src_h, dst_w, dst_h),
    }
}

fn compute_thumbnail_geometry(
    config: &ThumbnailConfig,
    source_w: u32,
    source_h: u32,
) -> (u32, u32, f32) {
    let (thumb_w, thumb_h) =
        compute_thumbnail_size(source_w, source_h, config.max_width, config.max_height);
    let scale = thumb_w as f32 / source_w.max(1) as f32;
    (thumb_w, thumb_h, scale)
}

/// Registry managing all active thumbnails, keyed by source window ID.
pub struct ThumbnailRegistry {
    thumbnails: Vec<Thumbnail>,
    next_id: u64,
    config: ThumbnailConfig,
}

impl ThumbnailRegistry {
    /// Create a new registry with the given configuration.
    pub fn new(config: ThumbnailConfig) -> Self {
        Self {
            thumbnails: Vec::new(),
            next_id: 1,
            config,
        }
    }

    /// The current configuration.
    pub fn config(&self) -> &ThumbnailConfig {
        &self.config
    }

    /// Create a thumbnail entry for a window. Returns the `ThumbnailId`.
    ///
    /// If a thumbnail already exists for this window, the existing entry is
    /// resized in place when the source geometry changed.
    pub fn create(&mut self, window_id: u64, source_w: u32, source_h: u32) -> ThumbnailId {
        let (thumb_w, thumb_h, scale) =
            compute_thumbnail_geometry(&self.config, source_w, source_h);

        // Return existing if present.
        if let Some(t) = self
            .thumbnails
            .iter_mut()
            .find(|t| t.source_window_id == window_id)
        {
            t.resize(thumb_w, thumb_h, scale);
            return t.id;
        }
        let id = ThumbnailId(self.next_id);
        self.next_id += 1;

        let thumb = Thumbnail::new(id, window_id, thumb_w, thumb_h, scale);
        self.thumbnails.push(thumb);
        id
    }

    /// Destroy the thumbnail for a given window, freeing its pixel buffer.
    pub fn destroy(&mut self, window_id: u64) -> bool {
        let before = self.thumbnails.len();
        self.thumbnails.retain(|t| t.source_window_id != window_id);
        self.thumbnails.len() < before
    }

    /// Get an immutable reference to a thumbnail by window ID.
    pub fn get(&self, window_id: u64) -> Option<&Thumbnail> {
        self.thumbnails
            .iter()
            .find(|t| t.source_window_id == window_id)
    }

    /// Get a mutable reference to a thumbnail by window ID.
    pub fn get_mut(&mut self, window_id: u64) -> Option<&mut Thumbnail> {
        self.thumbnails
            .iter_mut()
            .find(|t| t.source_window_id == window_id)
    }

    /// Get a thumbnail by its `ThumbnailId`.
    pub fn get_by_id(&self, id: ThumbnailId) -> Option<&Thumbnail> {
        self.thumbnails.iter().find(|t| t.id == id)
    }

    /// Update the pixel data for a window's thumbnail.
    ///
    /// The provided `pixels` should be BGRA data at the source window's
    /// resolution — this method handles downscaling internally.
    pub fn update(
        &mut self,
        window_id: u64,
        source_pixels: &[u8],
        source_w: u32,
        source_h: u32,
        now_ms: u64,
    ) -> bool {
        let (thumb_w, thumb_h, scale) =
            compute_thumbnail_geometry(&self.config, source_w, source_h);
        let quality = self.config.quality;
        let thumb = match self
            .thumbnails
            .iter_mut()
            .find(|t| t.source_window_id == window_id)
        {
            Some(t) => t,
            None => return false,
        };

        thumb.resize(thumb_w, thumb_h, scale);

        let expected = source_w as usize * source_h as usize * 4;
        if source_pixels.len() != expected {
            return false;
        }

        let scaled = downscale_with_quality(
            source_pixels,
            source_w,
            source_h,
            thumb.width,
            thumb.height,
            quality,
        );
        thumb.update_pixels(scaled, now_ms);
        true
    }

    /// Mark all thumbnails as stale.
    pub fn invalidate_all(&mut self) {
        for t in &mut self.thumbnails {
            t.invalidate();
        }
    }

    /// Mark a single window's thumbnail as stale.
    pub fn invalidate(&mut self, window_id: u64) {
        if let Some(t) = self
            .thumbnails
            .iter_mut()
            .find(|t| t.source_window_id == window_id)
        {
            t.invalidate();
        }
    }

    /// Return window IDs of all thumbnails that need updating.
    pub fn stale_windows(&self, now_ms: u64) -> Vec<u64> {
        self.thumbnails
            .iter()
            .filter(|t| {
                t.is_stale
                    || now_ms.saturating_sub(t.last_update_ms) >= self.config.update_interval_ms
            })
            .map(|t| t.source_window_id)
            .collect()
    }

    /// Number of thumbnails currently tracked.
    pub fn len(&self) -> usize {
        self.thumbnails.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.thumbnails.is_empty()
    }

    /// Iterate over all thumbnails.
    pub fn iter(&self) -> impl Iterator<Item = &Thumbnail> {
        self.thumbnails.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ThumbnailConfig {
        ThumbnailConfig::default()
    }

    // ── compute_thumbnail_size ─────────────────────────────────

    #[test]
    fn size_fits_within_max() {
        let (w, h) = compute_thumbnail_size(1920, 1080, 320, 240);
        assert!(w <= 320);
        assert!(h <= 240);
    }

    #[test]
    fn size_preserves_aspect_ratio() {
        let (w, h) = compute_thumbnail_size(1920, 1080, 320, 240);
        let src_ratio = 1920.0 / 1080.0;
        let dst_ratio = w as f64 / h as f64;
        assert!((src_ratio - dst_ratio).abs() < 0.05);
    }

    #[test]
    fn size_no_upscale_small_source() {
        let (w, h) = compute_thumbnail_size(100, 80, 320, 240);
        assert_eq!(w, 100);
        assert_eq!(h, 80);
    }

    #[test]
    fn size_zero_source_returns_one() {
        let (w, h) = compute_thumbnail_size(0, 0, 320, 240);
        assert!(w >= 1);
        assert!(h >= 1);
    }

    #[test]
    fn size_wide_window() {
        let (w, h) = compute_thumbnail_size(3840, 1080, 320, 240);
        assert!(w <= 320);
        assert!(h <= 240);
        // Width-constrained, so width should be near max.
        assert!(w >= 300);
    }

    #[test]
    fn size_tall_window() {
        let (w, h) = compute_thumbnail_size(800, 2400, 320, 240);
        assert!(w <= 320);
        assert!(h <= 240);
        // Height-constrained, so height should be near max.
        assert!(h >= 230);
    }

    #[test]
    fn size_square_window() {
        let (w, h) = compute_thumbnail_size(1000, 1000, 320, 240);
        // Limited by height (240 < 320).
        assert_eq!(w, h);
        assert!(w <= 240);
    }

    // ── downscale_bilinear ─────────────────────────────────────

    #[test]
    fn downscale_1x1() {
        let src = vec![10, 20, 30, 255]; // single BGRA pixel
        let dst = downscale_bilinear(&src, 1, 1, 1, 1);
        assert_eq!(dst, src);
    }

    #[test]
    fn downscale_2x2_to_1x1() {
        // 2x2 image: all red (BGRA: 0, 0, 255, 255).
        let src = vec![
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ];
        let dst = downscale_bilinear(&src, 2, 2, 1, 1);
        assert_eq!(dst.len(), 4);
        // All pixels same, so average should be the same.
        assert_eq!(dst[0], 0);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 255);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn downscale_preserves_length() {
        let src = vec![128u8; 64 * 48 * 4];
        let dst = downscale_bilinear(&src, 64, 48, 16, 12);
        assert_eq!(dst.len(), 16 * 12 * 4);
    }

    #[test]
    fn downscale_gradient_smooth() {
        // 4x1 horizontal gradient: 0, 85, 170, 255 (blue channel).
        let mut src = vec![0u8; 4 * 1 * 4];
        for x in 0..4 {
            src[x * 4] = (x as u8) * 85; // blue
            src[x * 4 + 3] = 255; // alpha
        }
        let dst = downscale_bilinear(&src, 4, 1, 2, 1);
        assert_eq!(dst.len(), 8);
        // First output pixel samples between src[0] and src[1] (at src_x=0).
        assert_eq!(dst[0], 0);
        // Second output pixel samples between src[2] and src[3] (at src_x=3).
        assert_eq!(dst[4], 255);
    }

    // ── Thumbnail struct ───────────────────────────────────────

    #[test]
    fn thumbnail_byte_len() {
        let t = Thumbnail::new(ThumbnailId(1), 42, 320, 240, 0.5);
        assert_eq!(t.byte_len(), 320 * 240 * 4);
    }

    #[test]
    fn thumbnail_starts_stale() {
        let t = Thumbnail::new(ThumbnailId(1), 1, 100, 100, 0.5);
        assert!(t.is_stale);
        assert!(t.pixel_data.is_none());
    }

    #[test]
    fn thumbnail_update_clears_stale() {
        let mut t = Thumbnail::new(ThumbnailId(1), 1, 2, 2, 0.5);
        let data = vec![0u8; 2 * 2 * 4];
        t.update_pixels(data, 1000);
        assert!(!t.is_stale);
        assert_eq!(t.last_update_ms, 1000);
        assert!(t.pixel_data.is_some());
    }

    #[test]
    fn thumbnail_invalidate() {
        let mut t = Thumbnail::new(ThumbnailId(1), 1, 2, 2, 0.5);
        t.update_pixels(vec![0u8; 16], 1000);
        assert!(!t.is_stale);
        t.invalidate();
        assert!(t.is_stale);
    }

    // ── ThumbnailRegistry ──────────────────────────────────────

    #[test]
    fn registry_create_and_get() {
        let mut reg = ThumbnailRegistry::new(default_config());
        let id = reg.create(1, 1920, 1080);
        assert_eq!(reg.len(), 1);
        let t = reg.get(1).unwrap();
        assert_eq!(t.id, id);
        assert_eq!(t.source_window_id, 1);
        assert!(t.width <= 320);
        assert!(t.height <= 240);
    }

    #[test]
    fn registry_create_idempotent() {
        let mut reg = ThumbnailRegistry::new(default_config());
        let id1 = reg.create(1, 1920, 1080);
        let id2 = reg.create(1, 1920, 1080);
        assert_eq!(id1, id2);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_create_resizes_existing_thumbnail_when_source_changes() {
        let mut reg = ThumbnailRegistry::new(default_config());
        let id = reg.create(1, 640, 480);
        let before = reg.get(1).unwrap().clone();

        let same_id = reg.create(1, 1920, 1080);

        let thumb = reg.get(1).unwrap();
        assert_eq!(same_id, id);
        assert_ne!((before.width, before.height), (thumb.width, thumb.height));
        assert_eq!((thumb.width, thumb.height), (320, 180));
        assert!(thumb.pixel_data.is_none());
        assert!(thumb.is_stale);
    }

    #[test]
    fn registry_destroy() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 800, 600);
        reg.create(2, 800, 600);
        assert_eq!(reg.len(), 2);
        assert!(reg.destroy(1));
        assert_eq!(reg.len(), 1);
        assert!(reg.get(1).is_none());
        assert!(reg.get(2).is_some());
    }

    #[test]
    fn registry_destroy_nonexistent() {
        let mut reg = ThumbnailRegistry::new(default_config());
        assert!(!reg.destroy(99));
    }

    #[test]
    fn registry_update_pixels() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 4, 4);
        let src = vec![128u8; 4 * 4 * 4];
        assert!(reg.update(1, &src, 4, 4, 500));
        let t = reg.get(1).unwrap();
        assert!(!t.is_stale);
        assert_eq!(t.last_update_ms, 500);
    }

    #[test]
    fn registry_update_wrong_size_fails() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 4, 4);
        let bad_src = vec![0u8; 10]; // wrong size
        assert!(!reg.update(1, &bad_src, 4, 4, 500));
    }

    #[test]
    fn registry_update_nonexistent_fails() {
        let mut reg = ThumbnailRegistry::new(default_config());
        assert!(!reg.update(99, &[], 4, 4, 500));
    }

    #[test]
    fn registry_update_resizes_when_source_changes() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 640, 480);
        let src_small = vec![128u8; 640 * 480 * 4];
        assert!(reg.update(1, &src_small, 640, 480, 100));

        let src_large = vec![64u8; 1920 * 1080 * 4];
        assert!(reg.update(1, &src_large, 1920, 1080, 200));

        let thumb = reg.get(1).unwrap();
        assert_eq!((thumb.width, thumb.height), (320, 180));
        assert_eq!(thumb.pixel_data.as_ref().unwrap().len(), thumb.byte_len());
        assert_eq!(thumb.last_update_ms, 200);
    }

    #[test]
    fn registry_invalidate_all() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 4, 4);
        reg.create(2, 4, 4);
        let src = vec![0u8; 4 * 4 * 4];
        reg.update(1, &src, 4, 4, 100);
        reg.update(2, &src, 4, 4, 100);
        reg.invalidate_all();
        assert!(reg.get(1).unwrap().is_stale);
        assert!(reg.get(2).unwrap().is_stale);
    }

    #[test]
    fn registry_invalidate_single() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 4, 4);
        reg.create(2, 4, 4);
        let src = vec![0u8; 4 * 4 * 4];
        reg.update(1, &src, 4, 4, 100);
        reg.update(2, &src, 4, 4, 100);
        reg.invalidate(1);
        assert!(reg.get(1).unwrap().is_stale);
        assert!(!reg.get(2).unwrap().is_stale);
    }

    #[test]
    fn registry_stale_windows() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 4, 4);
        reg.create(2, 4, 4);
        // Both start stale.
        let stale = reg.stale_windows(0);
        assert_eq!(stale.len(), 2);

        // Update one.
        let src = vec![0u8; 4 * 4 * 4];
        reg.update(1, &src, 4, 4, 100);
        // At time=100, only window 2 is stale (never updated).
        let stale = reg.stale_windows(100);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0], 2);

        // At time=400, window 1 is past its update interval (200ms default).
        let stale = reg.stale_windows(400);
        assert_eq!(stale.len(), 2);
    }

    #[test]
    fn registry_is_empty() {
        let reg = ThumbnailRegistry::new(default_config());
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_get_by_id() {
        let mut reg = ThumbnailRegistry::new(default_config());
        let id = reg.create(42, 800, 600);
        let t = reg.get_by_id(id).unwrap();
        assert_eq!(t.source_window_id, 42);
    }

    #[test]
    fn registry_get_mut() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 100, 100);
        let t = reg.get_mut(1).unwrap();
        t.is_stale = false;
        assert!(!reg.get(1).unwrap().is_stale);
    }

    #[test]
    fn registry_iter() {
        let mut reg = ThumbnailRegistry::new(default_config());
        reg.create(1, 100, 100);
        reg.create(2, 200, 200);
        let ids: Vec<u64> = reg.iter().map(|t| t.source_window_id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn registry_unique_ids() {
        let mut reg = ThumbnailRegistry::new(default_config());
        let id1 = reg.create(1, 100, 100);
        let id2 = reg.create(2, 100, 100);
        assert_ne!(id1, id2);
    }

    #[test]
    fn quality_levels_change_downscale_output() {
        let mut src = vec![0u8; 5 * 5 * 4];
        for y in 0..5 {
            for x in 0..5 {
                let idx = (y * 5 + x) * 4;
                src[idx] = (x as u8) * 40 + (y as u8) * 10;
                src[idx + 1] = (x as u8) * 10 + (y as u8) * 40;
                src[idx + 2] = (x as u8) * 20 + (y as u8) * 15;
                src[idx + 3] = 255;
            }
        }

        let low = downscale_with_quality(&src, 5, 5, 2, 2, ThumbnailQuality::Low);
        let high = downscale_with_quality(&src, 5, 5, 2, 2, ThumbnailQuality::High);

        assert_ne!(low, high);
    }

    #[test]
    fn config_default_values() {
        let cfg = ThumbnailConfig::default();
        assert_eq!(cfg.max_width, 320);
        assert_eq!(cfg.max_height, 240);
        assert_eq!(cfg.update_interval_ms, 200);
        assert_eq!(cfg.quality, ThumbnailQuality::Medium);
    }
}

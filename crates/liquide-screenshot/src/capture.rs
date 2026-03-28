/// Capture mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Full screen (all monitors)
    FullScreen,
    /// Single monitor by index
    Monitor(usize),
    /// Specific window by ID
    Window(u64),
    /// User-selected rectangular region
    Region,
    /// Active window
    ActiveWindow,
}

/// Region selection state (for interactive region capture)
#[derive(Debug, Clone, Copy)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    pub fn from_corners(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        let x = x1.min(x2);
        let y = y1.min(y2);
        let width = (x1 - x2).unsigned_abs();
        let height = (y1 - y2).unsigned_abs();
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32
            && py >= self.y && py < self.y + self.height as i32
    }
}

/// Result of a screen capture
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    /// BGRA8 pixel data
    pub pixels: Vec<u8>,
    pub region: CaptureRegion,
    pub timestamp: u64,
}

impl CaptureResult {
    /// Get pixel at (x, y) as (r, g, b, a)
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y * self.stride + x * 4) as usize;
        if offset + 3 >= self.pixels.len() {
            return None;
        }
        // BGRA → RGBA
        Some((
            self.pixels[offset + 2],
            self.pixels[offset + 1],
            self.pixels[offset],
            self.pixels[offset + 3],
        ))
    }

    /// Crop to a sub-region
    pub fn crop(&self, region: CaptureRegion) -> Option<CaptureResult> {
        let rx = (region.x - self.region.x).max(0) as u32;
        let ry = (region.y - self.region.y).max(0) as u32;

        if rx >= self.width || ry >= self.height {
            return None;
        }

        let cw = region.width.min(self.width - rx);
        let ch = region.height.min(self.height - ry);

        let mut pixels = Vec::with_capacity((cw * ch * 4) as usize);
        for row in ry..ry + ch {
            let start = (row * self.stride + rx * 4) as usize;
            let end = start + (cw * 4) as usize;
            if end <= self.pixels.len() {
                pixels.extend_from_slice(&self.pixels[start..end]);
            }
        }

        Some(CaptureResult {
            width: cw,
            height: ch,
            stride: cw * 4,
            pixels,
            region,
            timestamp: self.timestamp,
        })
    }
}

/// Screen capture abstraction
/// The actual capture is done by the compositor — this struct manages the state
pub struct ScreenCapture {
    mode: CaptureMode,
    /// For region capture: selection state
    selection_start: Option<(i32, i32)>,
    selection_end: Option<(i32, i32)>,
    selecting: bool,
    /// Delay before capture (seconds)
    delay_secs: u32,
    /// Include cursor in capture
    include_cursor: bool,
    /// Include window decorations (for window capture)
    include_decorations: bool,
    /// Flash effect on capture
    flash_effect: bool,
    /// Sound effect on capture
    sound_effect: bool,
}

impl ScreenCapture {
    pub fn new(mode: CaptureMode) -> Self {
        Self {
            mode,
            selection_start: None,
            selection_end: None,
            selecting: false,
            delay_secs: 0,
            include_cursor: false,
            include_decorations: true,
            flash_effect: true,
            sound_effect: true,
        }
    }

    pub fn with_delay(mut self, secs: u32) -> Self {
        self.delay_secs = secs;
        self
    }

    pub fn with_cursor(mut self, include: bool) -> Self {
        self.include_cursor = include;
        self
    }

    pub fn with_decorations(mut self, include: bool) -> Self {
        self.include_decorations = include;
        self
    }

    pub fn mode(&self) -> CaptureMode { self.mode }
    pub fn delay(&self) -> u32 { self.delay_secs }
    pub fn include_cursor(&self) -> bool { self.include_cursor }
    pub fn include_decorations(&self) -> bool { self.include_decorations }
    pub fn wants_flash(&self) -> bool { self.flash_effect }
    pub fn wants_sound(&self) -> bool { self.sound_effect }

    // Region selection methods
    pub fn begin_selection(&mut self, x: i32, y: i32) {
        self.selection_start = Some((x, y));
        self.selection_end = Some((x, y));
        self.selecting = true;
    }

    pub fn update_selection(&mut self, x: i32, y: i32) {
        if self.selecting {
            self.selection_end = Some((x, y));
        }
    }

    pub fn finish_selection(&mut self) -> Option<CaptureRegion> {
        self.selecting = false;
        match (self.selection_start, self.selection_end) {
            (Some((x1, y1)), Some((x2, y2))) => {
                let region = CaptureRegion::from_corners(x1, y1, x2, y2);
                if region.width > 0 && region.height > 0 {
                    Some(region)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn cancel_selection(&mut self) {
        self.selecting = false;
        self.selection_start = None;
        self.selection_end = None;
    }

    pub fn is_selecting(&self) -> bool { self.selecting }

    pub fn current_selection(&self) -> Option<CaptureRegion> {
        match (self.selection_start, self.selection_end) {
            (Some((x1, y1)), Some((x2, y2))) => Some(CaptureRegion::from_corners(x1, y1, x2, y2)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_capture(w: u32, h: u32) -> CaptureResult {
        // Create a BGRA image where pixel (x,y) = (B=x, G=y, R=x+y, A=255)
        let stride = w * 4;
        let mut pixels = vec![0u8; (stride * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let off = (y * stride + x * 4) as usize;
                pixels[off] = (x & 0xFF) as u8;           // B
                pixels[off + 1] = (y & 0xFF) as u8;       // G
                pixels[off + 2] = ((x + y) & 0xFF) as u8; // R
                pixels[off + 3] = 255;                     // A
            }
        }
        CaptureResult {
            width: w,
            height: h,
            stride,
            pixels,
            region: CaptureRegion { x: 0, y: 0, width: w, height: h },
            timestamp: 1000,
        }
    }

    #[test]
    fn region_from_corners_normalizes() {
        // Bottom-right to top-left
        let r = CaptureRegion::from_corners(100, 200, 10, 20);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 90);
        assert_eq!(r.height, 180);

        // Top-left to bottom-right (already normalized)
        let r2 = CaptureRegion::from_corners(10, 20, 100, 200);
        assert_eq!(r2.x, 10);
        assert_eq!(r2.y, 20);
        assert_eq!(r2.width, 90);
        assert_eq!(r2.height, 180);
    }

    #[test]
    fn region_contains() {
        let r = CaptureRegion { x: 10, y: 20, width: 100, height: 50 };
        assert!(r.contains(10, 20));   // top-left corner
        assert!(r.contains(50, 40));   // interior
        assert!(!r.contains(110, 20)); // right edge (exclusive)
        assert!(!r.contains(10, 70));  // bottom edge (exclusive)
        assert!(!r.contains(9, 20));   // just outside left
    }

    #[test]
    fn pixel_at_returns_rgba() {
        let cap = make_capture(4, 4);
        // pixel (2, 1): B=2, G=1, R=3, A=255 → RGBA = (3, 1, 2, 255)
        let (r, g, b, a) = cap.pixel_at(2, 1).unwrap();
        assert_eq!((r, g, b, a), (3, 1, 2, 255));
    }

    #[test]
    fn pixel_at_out_of_bounds() {
        let cap = make_capture(4, 4);
        assert!(cap.pixel_at(4, 0).is_none());
        assert!(cap.pixel_at(0, 4).is_none());
        assert!(cap.pixel_at(100, 100).is_none());
    }

    #[test]
    fn crop_returns_correct_subregion() {
        let cap = make_capture(10, 10);
        let region = CaptureRegion { x: 2, y: 3, width: 4, height: 5 };
        let cropped = cap.crop(region).unwrap();
        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 5);
        assert_eq!(cropped.stride, 16); // 4 * 4

        // Verify pixel (0,0) of cropped = pixel (2,3) of original
        let (r, g, b, a) = cropped.pixel_at(0, 0).unwrap();
        let (or, og, ob, oa) = cap.pixel_at(2, 3).unwrap();
        assert_eq!((r, g, b, a), (or, og, ob, oa));
    }

    #[test]
    fn crop_out_of_bounds_returns_none() {
        let cap = make_capture(10, 10);
        let region = CaptureRegion { x: 20, y: 20, width: 5, height: 5 };
        assert!(cap.crop(region).is_none());
    }

    #[test]
    fn selection_workflow() {
        let mut sc = ScreenCapture::new(CaptureMode::Region);
        assert!(!sc.is_selecting());

        sc.begin_selection(10, 20);
        assert!(sc.is_selecting());

        sc.update_selection(100, 200);
        let sel = sc.current_selection().unwrap();
        assert_eq!(sel.x, 10);
        assert_eq!(sel.width, 90);

        let region = sc.finish_selection().unwrap();
        assert!(!sc.is_selecting());
        assert_eq!(region.width, 90);
        assert_eq!(region.height, 180);
    }

    #[test]
    fn cancel_selection_clears_state() {
        let mut sc = ScreenCapture::new(CaptureMode::Region);
        sc.begin_selection(10, 20);
        sc.cancel_selection();
        assert!(!sc.is_selecting());
        assert!(sc.current_selection().is_none());
    }

    #[test]
    fn builder_methods() {
        let sc = ScreenCapture::new(CaptureMode::FullScreen)
            .with_delay(5)
            .with_cursor(true)
            .with_decorations(false);
        assert_eq!(sc.mode(), CaptureMode::FullScreen);
        assert_eq!(sc.delay(), 5);
        assert!(sc.include_cursor());
        assert!(!sc.include_decorations());
    }
}

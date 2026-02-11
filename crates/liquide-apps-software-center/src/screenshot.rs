//! Screenshot gallery handling.

/// A screenshot for an application.
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// URL of the screenshot image.
    pub url: String,
    /// Thumbnail URL.
    pub thumbnail_url: String,
    /// Caption/description.
    pub caption: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Screenshot {
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        thumbnail_url: impl Into<String>,
        caption: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            url: url.into(),
            thumbnail_url: thumbnail_url.into(),
            caption: caption.into(),
            width,
            height,
        }
    }

    /// Aspect ratio.
    #[must_use]
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 { return 0.0; }
        self.width as f64 / self.height as f64
    }
}

/// Screenshot gallery for an application.
pub struct Gallery {
    screenshots: Vec<Screenshot>,
    current: usize,
}

impl Gallery {
    #[must_use]
    pub fn new(screenshots: Vec<Screenshot>) -> Self {
        Self { screenshots, current: 0 }
    }

    /// Get all screenshots.
    #[must_use]
    pub fn screenshots(&self) -> &[Screenshot] { &self.screenshots }

    /// Get current screenshot.
    #[must_use]
    pub fn current(&self) -> Option<&Screenshot> { self.screenshots.get(self.current) }

    /// Number of screenshots.
    #[must_use]
    pub fn count(&self) -> usize { self.screenshots.len() }

    /// Navigate to the next screenshot.
    pub fn next(&mut self) {
        if !self.screenshots.is_empty() {
            self.current = (self.current + 1) % self.screenshots.len();
        }
    }

    /// Navigate to the previous screenshot.
    pub fn prev(&mut self) {
        if !self.screenshots.is_empty() {
            self.current = if self.current == 0 {
                self.screenshots.len() - 1
            } else {
                self.current - 1
            };
        }
    }

    /// Jump to a specific index.
    pub fn goto(&mut self, index: usize) {
        if index < self.screenshots.len() {
            self.current = index;
        }
    }

    /// Current index.
    #[must_use]
    pub fn current_index(&self) -> usize { self.current }
}

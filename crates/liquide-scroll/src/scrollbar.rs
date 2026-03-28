use crate::state::ScrollState;

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Visual style of the scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarStyle {
    /// Thin, auto-hiding overlay scrollbar (macOS style).
    Overlay,
    /// Always-visible, wider scrollbar (classic style).
    Classic,
    /// Scrollbar is never shown.
    Hidden,
}

/// Result of a hit test on a scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHit {
    /// No hit on the scrollbar.
    None,
    /// Hit on the track area.
    Track {
        /// True if the click was before (above/left of) the thumb.
        before_thumb: bool,
    },
    /// Hit on the thumb itself.
    Thumb,
    /// Hit on the up/left arrow button.
    UpArrow,
    /// Hit on the down/right arrow button.
    DownArrow,
}

/// A simple axis-aligned rectangle.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether a point is inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Computed scrollbar visual state.
#[derive(Debug, Clone)]
pub struct ScrollbarState {
    /// Whether the scrollbar is currently visible.
    pub visible: bool,
    /// Position of the thumb along the track (pixels from track start).
    pub thumb_position: f32,
    /// Size of the thumb (pixels along the track axis).
    pub thumb_size: f32,
    /// Total track length (pixels).
    pub track_size: f32,
    /// Scrollbar orientation.
    pub orientation: Orientation,
}

/// Minimum thumb size in pixels.
const MIN_THUMB_SIZE: f32 = 30.0;

/// Compute scrollbar visual state from scroll state and track dimensions.
pub fn compute(state: &ScrollState, track_length: f32, orientation: Orientation) -> ScrollbarState {
    let (content, viewport, offset) = match orientation {
        Orientation::Vertical => (state.content_size.1, state.viewport_size.1, state.offset.1),
        Orientation::Horizontal => (state.content_size.0, state.viewport_size.0, state.offset.0),
    };

    // If content fits in viewport, scrollbar is not needed.
    if content <= viewport || content <= 0.0 {
        return ScrollbarState {
            visible: false,
            thumb_position: 0.0,
            thumb_size: track_length,
            track_size: track_length,
            orientation,
        };
    }

    // Thumb size is proportional to viewport/content ratio.
    let ratio = viewport / content;
    let thumb_size = (track_length * ratio).max(MIN_THUMB_SIZE).min(track_length);

    // Available space for thumb to move.
    let available = track_length - thumb_size;

    // Thumb position.
    let max_scroll = content - viewport;
    let scroll_fraction = if max_scroll > 0.0 {
        (offset / max_scroll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_position = available * scroll_fraction;

    ScrollbarState {
        visible: true,
        thumb_position,
        thumb_size,
        track_size: track_length,
        orientation,
    }
}

/// Hit-test a point against a scrollbar within the given rectangle.
///
/// `scrollbar_rect` defines the entire scrollbar area including arrows.
/// Arrow buttons are assumed to be square, sized by the short dimension of the rect.
pub fn hit_test(
    point: (f32, f32),
    scrollbar_rect: Rect,
    scrollbar_state: &ScrollbarState,
) -> ScrollbarHit {
    if !scrollbar_rect.contains(point.0, point.1) {
        return ScrollbarHit::None;
    }

    let (pos_along, rect_start, rect_length, arrow_size) = match scrollbar_state.orientation {
        Orientation::Vertical => (
            point.1 - scrollbar_rect.y,
            scrollbar_rect.y,
            scrollbar_rect.height,
            scrollbar_rect.width,
        ),
        Orientation::Horizontal => (
            point.0 - scrollbar_rect.x,
            scrollbar_rect.x,
            scrollbar_rect.width,
            scrollbar_rect.height,
        ),
    };
    let _ = rect_start;

    // Arrow zones at start and end.
    if pos_along < arrow_size {
        return ScrollbarHit::UpArrow;
    }
    if pos_along >= rect_length - arrow_size {
        return ScrollbarHit::DownArrow;
    }

    // Track zone between arrows.
    let track_start = arrow_size;
    let pos_in_track = pos_along - track_start;

    let thumb_start = scrollbar_state.thumb_position;
    let thumb_end = thumb_start + scrollbar_state.thumb_size;

    if pos_in_track >= thumb_start && pos_in_track < thumb_end {
        ScrollbarHit::Thumb
    } else {
        ScrollbarHit::Track {
            before_thumb: pos_in_track < thumb_start,
        }
    }
}

/// Scrollbar auto-hide controller.
#[derive(Debug, Clone)]
pub struct AutoHideController {
    /// Style of scrollbar.
    pub style: ScrollbarStyle,
    /// Delay before auto-hiding starts (ms).
    pub auto_hide_delay_ms: u32,
    /// Fade-out animation duration (ms).
    pub fade_out_ms: u32,
    /// Time since last scroll activity (ms).
    idle_time_ms: u32,
    /// Current opacity (1.0 = fully visible, 0.0 = hidden).
    opacity: f32,
    /// Whether the scrollbar is being hovered or dragged.
    forced_visible: bool,
}

impl AutoHideController {
    pub fn new(style: ScrollbarStyle) -> Self {
        let opacity = match style {
            ScrollbarStyle::Overlay => 0.0,
            ScrollbarStyle::Classic => 1.0,
            ScrollbarStyle::Hidden => 0.0,
        };
        Self {
            style,
            auto_hide_delay_ms: 1000,
            fade_out_ms: 300,
            idle_time_ms: 0,
            opacity,
            forced_visible: false,
        }
    }

    /// Called when scroll activity occurs (scroll, hover, drag).
    pub fn on_activity(&mut self) {
        self.idle_time_ms = 0;
        match self.style {
            ScrollbarStyle::Overlay => self.opacity = 1.0,
            ScrollbarStyle::Classic => self.opacity = 1.0,
            ScrollbarStyle::Hidden => {}
        }
    }

    /// Set whether the scrollbar is being directly interacted with.
    pub fn set_forced_visible(&mut self, forced: bool) {
        self.forced_visible = forced;
        if forced {
            self.opacity = 1.0;
        }
    }

    /// Advance the auto-hide timer by `elapsed_ms`.
    /// Returns the current opacity (0.0 to 1.0).
    pub fn tick(&mut self, elapsed_ms: u32) -> f32 {
        match self.style {
            ScrollbarStyle::Hidden => return 0.0,
            ScrollbarStyle::Classic => return 1.0,
            ScrollbarStyle::Overlay => {}
        }

        if self.forced_visible {
            return 1.0;
        }

        self.idle_time_ms += elapsed_ms;

        if self.idle_time_ms <= self.auto_hide_delay_ms {
            // Still in the visible period.
            return self.opacity;
        }

        // Fade out.
        let fade_elapsed = self.idle_time_ms - self.auto_hide_delay_ms;
        if self.fade_out_ms > 0 {
            let fade_progress = (fade_elapsed as f32 / self.fade_out_ms as f32).min(1.0);
            self.opacity = 1.0 - fade_progress;
        } else {
            self.opacity = 0.0;
        }

        self.opacity
    }

    /// Current opacity.
    pub fn opacity(&self) -> f32 {
        self.opacity
    }
}

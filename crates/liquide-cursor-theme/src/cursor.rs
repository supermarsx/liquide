/// Standard cursor shapes (matches CSS cursor property values)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorShape {
    Default,
    Pointer,    // hand/link
    Text,       // I-beam
    Crosshair,
    Move,
    Wait,
    Progress,   // wait + arrow
    Help,       // arrow + question mark
    NotAllowed, // circle with line
    Grab,
    Grabbing,
    ResizeN,
    ResizeS,
    ResizeE,
    ResizeW,
    ResizeNE,
    ResizeNW,
    ResizeSE,
    ResizeSW,
    ResizeNS,   // vertical double arrow
    ResizeEW,   // horizontal double arrow
    ResizeNESW, // diagonal
    ResizeNWSE, // diagonal
    ColResize,
    RowResize,
    ZoomIn,
    ZoomOut,
    Copy,       // arrow + plus
    Alias,      // arrow + curved arrow
    ContextMenu,// arrow + menu
    Cell,       // plus/cross
    VerticalText,
    NoDrop,
    None,       // invisible cursor
}

impl CursorShape {
    /// CSS cursor name to shape
    pub fn from_css(name: &str) -> Option<Self> {
        match name {
            "default" | "auto" => Some(Self::Default),
            "pointer" => Some(Self::Pointer),
            "text" => Some(Self::Text),
            "crosshair" => Some(Self::Crosshair),
            "move" => Some(Self::Move),
            "wait" => Some(Self::Wait),
            "progress" => Some(Self::Progress),
            "help" => Some(Self::Help),
            "not-allowed" => Some(Self::NotAllowed),
            "grab" => Some(Self::Grab),
            "grabbing" => Some(Self::Grabbing),
            "n-resize" => Some(Self::ResizeN),
            "s-resize" => Some(Self::ResizeS),
            "e-resize" => Some(Self::ResizeE),
            "w-resize" => Some(Self::ResizeW),
            "ne-resize" => Some(Self::ResizeNE),
            "nw-resize" => Some(Self::ResizeNW),
            "se-resize" => Some(Self::ResizeSE),
            "sw-resize" => Some(Self::ResizeSW),
            "ns-resize" | "row-resize" => Some(Self::ResizeNS),
            "ew-resize" | "col-resize" => Some(Self::ResizeEW),
            "nesw-resize" => Some(Self::ResizeNESW),
            "nwse-resize" => Some(Self::ResizeNWSE),
            "zoom-in" => Some(Self::ZoomIn),
            "zoom-out" => Some(Self::ZoomOut),
            "copy" => Some(Self::Copy),
            "alias" => Some(Self::Alias),
            "context-menu" => Some(Self::ContextMenu),
            "cell" => Some(Self::Cell),
            "vertical-text" => Some(Self::VerticalText),
            "no-drop" => Some(Self::NoDrop),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    /// X11/Wayland cursor name
    pub fn x11_name(&self) -> &str {
        match self {
            Self::Default => "left_ptr",
            Self::Pointer => "hand2",
            Self::Text => "xterm",
            Self::Crosshair => "crosshair",
            Self::Move => "fleur",
            Self::Wait => "watch",
            Self::Progress => "left_ptr_watch",
            Self::Help => "question_arrow",
            Self::NotAllowed => "crossed_circle",
            Self::Grab => "openhand",
            Self::Grabbing => "closedhand",
            Self::ResizeN | Self::ResizeS | Self::ResizeNS => "sb_v_double_arrow",
            Self::ResizeE | Self::ResizeW | Self::ResizeEW => "sb_h_double_arrow",
            Self::ResizeNE | Self::ResizeSW | Self::ResizeNESW => "fd_double_arrow",
            Self::ResizeNW | Self::ResizeSE | Self::ResizeNWSE => "bd_double_arrow",
            Self::ColResize => "sb_h_double_arrow",
            Self::RowResize => "sb_v_double_arrow",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::Copy => "copy",
            Self::Alias => "alias",
            Self::ContextMenu => "context-menu",
            Self::Cell => "plus",
            Self::VerticalText => "vertical-text",
            Self::NoDrop => "no-drop",
            Self::None => "none",
        }
    }

    /// Win32 cursor resource name
    pub fn win32_id(&self) -> u32 {
        match self {
            Self::Default => 32512,     // IDC_ARROW
            Self::Pointer => 32649,     // IDC_HAND
            Self::Text => 32513,        // IDC_IBEAM
            Self::Crosshair => 32515,   // IDC_CROSS
            Self::Move | Self::Grab | Self::Grabbing => 32646, // IDC_SIZEALL
            Self::Wait => 32514,        // IDC_WAIT
            Self::Progress => 32650,    // IDC_APPSTARTING
            Self::Help => 32651,        // IDC_HELP
            Self::NotAllowed | Self::NoDrop => 32648, // IDC_NO
            Self::ResizeN | Self::ResizeS | Self::ResizeNS | Self::RowResize => 32645, // IDC_SIZENS
            Self::ResizeE | Self::ResizeW | Self::ResizeEW | Self::ColResize => 32644, // IDC_SIZEWE
            Self::ResizeNE | Self::ResizeSW | Self::ResizeNESW => 32643, // IDC_SIZENESW
            Self::ResizeNW | Self::ResizeSE | Self::ResizeNWSE => 32642, // IDC_SIZENWSE
            _ => 32512, // fallback to arrow
        }
    }

    /// All standard shapes
    pub fn all() -> &'static [Self] {
        &[
            Self::Default, Self::Pointer, Self::Text, Self::Crosshair,
            Self::Move, Self::Wait, Self::Progress, Self::Help,
            Self::NotAllowed, Self::Grab, Self::Grabbing,
            Self::ResizeN, Self::ResizeS, Self::ResizeE, Self::ResizeW,
            Self::ResizeNE, Self::ResizeNW, Self::ResizeSE, Self::ResizeSW,
            Self::ResizeNS, Self::ResizeEW, Self::ResizeNESW, Self::ResizeNWSE,
            Self::ColResize, Self::RowResize,
            Self::ZoomIn, Self::ZoomOut, Self::Copy, Self::Alias,
            Self::ContextMenu, Self::Cell, Self::VerticalText, Self::NoDrop,
            Self::None,
        ]
    }
}

/// A cursor image (single frame)
#[derive(Debug, Clone)]
pub struct CursorImage {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    /// RGBA pixels
    pub pixels: Vec<u8>,
    /// Nominal size (e.g., 24, 32, 48)
    pub nominal_size: u32,
}

impl CursorImage {
    pub fn new(width: u32, height: u32, hotspot_x: u32, hotspot_y: u32, pixels: Vec<u8>) -> Self {
        Self {
            width, height, hotspot_x, hotspot_y, pixels,
            nominal_size: width,
        }
    }

    /// Create a simple colored square cursor (for testing)
    pub fn solid_square(size: u32, r: u8, g: u8, b: u8) -> Self {
        let pixel_count = (size * size) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
        Self::new(size, size, 0, 0, pixels)
    }
}

/// Animated cursor (multiple frames)
#[derive(Debug, Clone)]
pub struct AnimatedCursor {
    pub frames: Vec<CursorImage>,
    pub frame_delays_ms: Vec<u32>,
    pub current_frame: usize,
    pub elapsed_ms: u32,
}

impl AnimatedCursor {
    pub fn new(frames: Vec<CursorImage>, frame_delays_ms: Vec<u32>) -> Self {
        Self {
            frames,
            frame_delays_ms,
            current_frame: 0,
            elapsed_ms: 0,
        }
    }

    /// Advance animation by delta_ms, return true if frame changed
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if self.frames.len() <= 1 {
            return false;
        }

        self.elapsed_ms += delta_ms;
        let delay = self.frame_delays_ms.get(self.current_frame).copied().unwrap_or(100);

        if self.elapsed_ms >= delay {
            self.elapsed_ms -= delay;
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            true
        } else {
            false
        }
    }

    pub fn current_image(&self) -> Option<&CursorImage> {
        self.frames.get(self.current_frame)
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

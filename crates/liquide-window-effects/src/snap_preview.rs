use crate::effects::Rect;

/// Snap zone types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
    None,
}

/// Snap preview state
pub struct SnapPreview {
    pub active: bool,
    pub zone: SnapZone,
    pub target_rect: Rect,
    pub opacity: f32,
    pub corner_radius: f32,
    pub border_width: f32,
    pub color: (u8, u8, u8, u8),  // RGBA
}

impl SnapPreview {
    pub fn new() -> Self {
        Self {
            active: false,
            zone: SnapZone::None,
            target_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            opacity: 0.0,
            corner_radius: 8.0,
            border_width: 2.0,
            color: (100, 150, 255, 80), // semi-transparent blue
        }
    }

    /// Show the snap preview for a zone given screen work area
    pub fn show(&mut self, zone: SnapZone, work_area: Rect, gap: f32) {
        self.active = true;
        self.zone = zone;
        self.opacity = 0.3;

        let half_w = (work_area.width - gap * 3.0) / 2.0;
        let half_h = (work_area.height - gap * 3.0) / 2.0;
        let x = work_area.x + gap;
        let y = work_area.y + gap;

        self.target_rect = match zone {
            SnapZone::Left => Rect::new(x, y, half_w, work_area.height - gap * 2.0),
            SnapZone::Right => Rect::new(x + half_w + gap, y, half_w, work_area.height - gap * 2.0),
            SnapZone::Top => Rect::new(x, y, work_area.width - gap * 2.0, half_h),
            SnapZone::Bottom => Rect::new(x, y + half_h + gap, work_area.width - gap * 2.0, half_h),
            SnapZone::TopLeft => Rect::new(x, y, half_w, half_h),
            SnapZone::TopRight => Rect::new(x + half_w + gap, y, half_w, half_h),
            SnapZone::BottomLeft => Rect::new(x, y + half_h + gap, half_w, half_h),
            SnapZone::BottomRight => Rect::new(x + half_w + gap, y + half_h + gap, half_w, half_h),
            SnapZone::Maximize => Rect::new(x, y, work_area.width - gap * 2.0, work_area.height - gap * 2.0),
            SnapZone::None => Rect::new(0.0, 0.0, 0.0, 0.0),
        };
    }

    /// Hide the snap preview
    pub fn hide(&mut self) {
        self.active = false;
        self.zone = SnapZone::None;
        self.opacity = 0.0;
    }

    /// Detect snap zone from cursor position relative to screen edges
    pub fn detect_zone(cursor_x: f32, cursor_y: f32, screen: Rect, threshold: f32) -> SnapZone {
        let at_left = cursor_x - screen.x < threshold;
        let at_right = (screen.x + screen.width) - cursor_x < threshold;
        let at_top = cursor_y - screen.y < threshold;
        let at_bottom = (screen.y + screen.height) - cursor_y < threshold;

        match (at_left, at_right, at_top, at_bottom) {
            (true, false, true, false) => SnapZone::TopLeft,
            (false, true, true, false) => SnapZone::TopRight,
            (true, false, false, true) => SnapZone::BottomLeft,
            (false, true, false, true) => SnapZone::BottomRight,
            (true, false, _, _) => SnapZone::Left,
            (false, true, _, _) => SnapZone::Right,
            (_, _, true, false) => SnapZone::Maximize, // top edge = maximize
            (_, _, false, true) => SnapZone::Bottom,
            _ => SnapZone::None,
        }
    }
}

impl Default for SnapPreview {
    fn default() -> Self { Self::new() }
}

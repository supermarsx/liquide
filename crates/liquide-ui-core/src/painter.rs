//! Paint command buffer — deferred rendering abstraction.
//!
//! Widgets paint into a `Painter` which records draw commands. The
//! renderer then consumes these commands to produce actual pixels.
//! This is similar to Qt's `QPainter` or GTK's `cairo_t`.

use crate::color::UiColor;

/// A recorded paint command.
#[derive(Debug, Clone)]
pub enum PaintCommand {
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: UiColor,
    },
    FillRoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: UiColor,
    },
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: UiColor,
        width: f32,
    },
    StrokeRoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: UiColor,
        width: f32,
    },
    DrawText {
        text: String,
        x: f32,
        y: f32,
        size: f32,
        color: UiColor,
        font_family: String,
        bold: bool,
    },
    DrawLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: UiColor,
        width: f32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: UiColor,
    },
    PushClip {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    PopClip,
    /// Draw an icon by id (icon atlas index).
    DrawIcon {
        icon_id: u32,
        x: f32,
        y: f32,
        size: f32,
        color: UiColor,
    },
}

/// A paint context that records commands for deferred rendering.
pub struct Painter {
    commands: Vec<PaintCommand>,
    clip_stack: Vec<(f32, f32, f32, f32)>,
    /// Translation offset (for nested widget painting).
    offset_x: f32,
    offset_y: f32,
}

impl Painter {
    pub fn new() -> Self {
        Self {
            commands: Vec::with_capacity(256),
            clip_stack: Vec::new(),
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// All recorded commands.
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }

    /// Consume the painter and return all commands.
    pub fn into_commands(self) -> Vec<PaintCommand> {
        self.commands
    }

    /// Clear all recorded commands.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.clip_stack.clear();
    }

    /// Push a translation offset for child painting.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    /// Pop the last translation.
    pub fn restore_translation(&mut self, dx: f32, dy: f32) {
        self.offset_x -= dx;
        self.offset_y -= dy;
    }

    // -- Drawing primitives --

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: UiColor) {
        self.commands.push(PaintCommand::FillRect {
            x: x + self.offset_x,
            y: y + self.offset_y,
            w,
            h,
            color,
        });
    }

    pub fn fill_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: UiColor) {
        self.commands.push(PaintCommand::FillRoundedRect {
            x: x + self.offset_x,
            y: y + self.offset_y,
            w,
            h,
            radius,
            color,
        });
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: UiColor, width: f32) {
        self.commands.push(PaintCommand::StrokeRect {
            x: x + self.offset_x,
            y: y + self.offset_y,
            w,
            h,
            color,
            width,
        });
    }

    pub fn stroke_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: UiColor, width: f32) {
        self.commands.push(PaintCommand::StrokeRoundedRect {
            x: x + self.offset_x,
            y: y + self.offset_y,
            w,
            h,
            radius,
            color,
            width,
        });
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: UiColor,
        font_family: &str,
        bold: bool,
    ) {
        self.commands.push(PaintCommand::DrawText {
            text: text.to_string(),
            x: x + self.offset_x,
            y: y + self.offset_y,
            size,
            color,
            font_family: font_family.to_string(),
            bold,
        });
    }

    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: UiColor, width: f32) {
        self.commands.push(PaintCommand::DrawLine {
            x1: x1 + self.offset_x,
            y1: y1 + self.offset_y,
            x2: x2 + self.offset_x,
            y2: y2 + self.offset_y,
            color,
            width,
        });
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: UiColor) {
        self.commands.push(PaintCommand::FillCircle {
            cx: cx + self.offset_x,
            cy: cy + self.offset_y,
            r,
            color,
        });
    }

    pub fn draw_icon(&mut self, icon_id: u32, x: f32, y: f32, size: f32, color: UiColor) {
        self.commands.push(PaintCommand::DrawIcon {
            icon_id,
            x: x + self.offset_x,
            y: y + self.offset_y,
            size,
            color,
        });
    }

    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.clip_stack.push((x + self.offset_x, y + self.offset_y, w, h));
        self.commands.push(PaintCommand::PushClip {
            x: x + self.offset_x,
            y: y + self.offset_y,
            w,
            h,
        });
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.commands.push(PaintCommand::PopClip);
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

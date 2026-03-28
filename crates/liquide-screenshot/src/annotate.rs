/// Annotation tool types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationTool {
    Arrow,
    Rectangle,
    Ellipse,
    FreehandDraw,
    Text,
    Highlight,
    Blur,
    Pixelate,
    Line,
    NumberMarker,
    Crop,
}

/// Color with alpha
#[derive(Debug, Clone, Copy)]
pub struct AnnotColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl AnnotColor {
    pub const RED: Self = Self { r: 255, g: 59, b: 48, a: 255 };
    pub const GREEN: Self = Self { r: 52, g: 199, b: 89, a: 255 };
    pub const BLUE: Self = Self { r: 0, g: 122, b: 255, a: 255 };
    pub const YELLOW: Self = Self { r: 255, g: 204, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const HIGHLIGHT: Self = Self { r: 255, g: 255, b: 0, a: 128 };
}

/// A single annotation on the screenshot
#[derive(Debug, Clone)]
pub struct Annotation {
    pub id: u64,
    pub tool: AnnotationTool,
    pub color: AnnotColor,
    pub stroke_width: f32,
    pub points: Vec<(f32, f32)>,
    pub text: Option<String>,
    pub font_size: f32,
    pub number: Option<u32>,
}

/// State machine for the annotation editor
pub struct AnnotationState {
    annotations: Vec<Annotation>,
    undo_stack: Vec<Vec<Annotation>>,
    redo_stack: Vec<Vec<Annotation>>,
    current_tool: AnnotationTool,
    current_color: AnnotColor,
    current_stroke_width: f32,
    current_font_size: f32,
    next_id: u64,
    next_number: u32,
    drawing: bool,
    current_points: Vec<(f32, f32)>,
}

impl AnnotationState {
    pub fn new() -> Self {
        Self {
            annotations: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_tool: AnnotationTool::Arrow,
            current_color: AnnotColor::RED,
            current_stroke_width: 2.0,
            current_font_size: 16.0,
            next_id: 1,
            next_number: 1,
            drawing: false,
            current_points: Vec::new(),
        }
    }

    pub fn set_tool(&mut self, tool: AnnotationTool) { self.current_tool = tool; }
    pub fn set_color(&mut self, color: AnnotColor) { self.current_color = color; }
    pub fn set_stroke_width(&mut self, w: f32) { self.current_stroke_width = w; }
    pub fn set_font_size(&mut self, s: f32) { self.current_font_size = s; }
    pub fn current_tool(&self) -> AnnotationTool { self.current_tool }
    pub fn annotations(&self) -> &[Annotation] { &self.annotations }

    pub fn begin_draw(&mut self, x: f32, y: f32) {
        self.drawing = true;
        self.current_points = vec![(x, y)];
    }

    pub fn continue_draw(&mut self, x: f32, y: f32) {
        if self.drawing {
            self.current_points.push((x, y));
        }
    }

    pub fn end_draw(&mut self, x: f32, y: f32) -> Option<u64> {
        if !self.drawing {
            return None;
        }
        self.drawing = false;
        self.current_points.push((x, y));

        if self.current_points.len() < 2 {
            return None;
        }

        self.save_undo();

        let number = if self.current_tool == AnnotationTool::NumberMarker {
            let n = self.next_number;
            self.next_number += 1;
            Some(n)
        } else {
            None
        };

        let id = self.next_id;
        self.next_id += 1;

        let annotation = Annotation {
            id,
            tool: self.current_tool,
            color: self.current_color,
            stroke_width: self.current_stroke_width,
            points: std::mem::take(&mut self.current_points),
            text: None,
            font_size: self.current_font_size,
            number,
        };

        self.annotations.push(annotation);
        Some(id)
    }

    pub fn add_text(&mut self, x: f32, y: f32, text: String) -> u64 {
        self.save_undo();
        let id = self.next_id;
        self.next_id += 1;
        self.annotations.push(Annotation {
            id,
            tool: AnnotationTool::Text,
            color: self.current_color,
            stroke_width: self.current_stroke_width,
            points: vec![(x, y)],
            text: Some(text),
            font_size: self.current_font_size,
            number: None,
        });
        id
    }

    pub fn delete_annotation(&mut self, id: u64) -> bool {
        if let Some(pos) = self.annotations.iter().position(|a| a.id == id) {
            self.save_undo();
            self.annotations.remove(pos);
            true
        } else {
            false
        }
    }

    fn save_undo(&mut self) {
        self.undo_stack.push(self.annotations.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.annotations.clone());
            self.annotations = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.annotations.clone());
            self.annotations = next;
            true
        } else {
            false
        }
    }

    pub fn clear_all(&mut self) {
        if !self.annotations.is_empty() {
            self.save_undo();
            self.annotations.clear();
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn annotation_count(&self) -> usize { self.annotations.len() }
    pub fn is_drawing(&self) -> bool { self.drawing }
    pub fn current_drawing_points(&self) -> &[(f32, f32)] { &self.current_points }
}

impl Default for AnnotationState {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_creates_annotation() {
        let mut state = AnnotationState::new();
        state.begin_draw(10.0, 20.0);
        state.continue_draw(30.0, 40.0);
        let id = state.end_draw(50.0, 60.0);
        assert!(id.is_some());
        assert_eq!(state.annotation_count(), 1);

        let ann = &state.annotations()[0];
        assert_eq!(ann.tool, AnnotationTool::Arrow); // default tool
        assert_eq!(ann.points.len(), 3);
        assert_eq!(ann.points[0], (10.0, 20.0));
        assert_eq!(ann.points[2], (50.0, 60.0));
    }

    #[test]
    fn end_draw_without_begin_returns_none() {
        let mut state = AnnotationState::new();
        assert!(state.end_draw(10.0, 20.0).is_none());
    }

    #[test]
    fn undo_redo() {
        let mut state = AnnotationState::new();
        assert!(!state.can_undo());
        assert!(!state.can_redo());

        // Draw one annotation
        state.begin_draw(0.0, 0.0);
        state.end_draw(10.0, 10.0);
        assert_eq!(state.annotation_count(), 1);
        assert!(state.can_undo());

        // Undo removes it
        assert!(state.undo());
        assert_eq!(state.annotation_count(), 0);
        assert!(state.can_redo());

        // Redo restores it
        assert!(state.redo());
        assert_eq!(state.annotation_count(), 1);

        // Redo again when nothing to redo
        assert!(!state.redo());
    }

    #[test]
    fn clear_all_and_undo() {
        let mut state = AnnotationState::new();
        state.begin_draw(0.0, 0.0);
        state.end_draw(10.0, 10.0);
        state.begin_draw(20.0, 20.0);
        state.end_draw(30.0, 30.0);
        assert_eq!(state.annotation_count(), 2);

        state.clear_all();
        assert_eq!(state.annotation_count(), 0);

        // Undo restores all
        assert!(state.undo());
        assert_eq!(state.annotation_count(), 2);
    }

    #[test]
    fn tool_color_stroke_changes() {
        let mut state = AnnotationState::new();
        state.set_tool(AnnotationTool::Rectangle);
        state.set_color(AnnotColor::BLUE);
        state.set_stroke_width(5.0);
        state.set_font_size(24.0);

        assert_eq!(state.current_tool(), AnnotationTool::Rectangle);

        state.begin_draw(0.0, 0.0);
        state.end_draw(100.0, 100.0);

        let ann = &state.annotations()[0];
        assert_eq!(ann.tool, AnnotationTool::Rectangle);
        assert_eq!(ann.color.r, AnnotColor::BLUE.r);
        assert_eq!(ann.color.g, AnnotColor::BLUE.g);
        assert_eq!(ann.color.b, AnnotColor::BLUE.b);
        assert!((ann.stroke_width - 5.0).abs() < f32::EPSILON);
        assert!((ann.font_size - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn add_text_annotation() {
        let mut state = AnnotationState::new();
        let id = state.add_text(50.0, 50.0, "Hello".to_string());
        assert_eq!(state.annotation_count(), 1);

        let ann = &state.annotations()[0];
        assert_eq!(ann.id, id);
        assert_eq!(ann.tool, AnnotationTool::Text);
        assert_eq!(ann.text.as_deref(), Some("Hello"));
    }

    #[test]
    fn delete_annotation() {
        let mut state = AnnotationState::new();
        state.begin_draw(0.0, 0.0);
        let id = state.end_draw(10.0, 10.0).unwrap();

        assert!(state.delete_annotation(id));
        assert_eq!(state.annotation_count(), 0);

        // Deleting non-existent returns false
        assert!(!state.delete_annotation(999));
    }

    #[test]
    fn number_marker_increments() {
        let mut state = AnnotationState::new();
        state.set_tool(AnnotationTool::NumberMarker);

        state.begin_draw(0.0, 0.0);
        state.end_draw(10.0, 10.0);
        assert_eq!(state.annotations()[0].number, Some(1));

        state.begin_draw(20.0, 20.0);
        state.end_draw(30.0, 30.0);
        assert_eq!(state.annotations()[1].number, Some(2));
    }

    #[test]
    fn drawing_state() {
        let mut state = AnnotationState::new();
        assert!(!state.is_drawing());
        assert!(state.current_drawing_points().is_empty());

        state.begin_draw(5.0, 10.0);
        assert!(state.is_drawing());
        assert_eq!(state.current_drawing_points().len(), 1);

        state.continue_draw(15.0, 20.0);
        assert_eq!(state.current_drawing_points().len(), 2);
    }

    #[test]
    fn default_impl() {
        let state = AnnotationState::default();
        assert_eq!(state.annotation_count(), 0);
        assert_eq!(state.current_tool(), AnnotationTool::Arrow);
    }
}

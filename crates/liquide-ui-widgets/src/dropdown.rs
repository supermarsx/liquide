//! Dropdown (combo box) widget.
//!
//! Displays a selectable list of items in a popup. Inspired by Qt's
//! QComboBox and GTK's GtkComboBox.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// A single item in a dropdown.
#[derive(Debug, Clone)]
pub struct DropdownItem {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub enabled: bool,
}

impl DropdownItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, enabled: true }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// A dropdown / combo-box widget.
pub struct Dropdown {
    state: WidgetState,
    items: Vec<DropdownItem>,
    selected: Option<usize>,
    placeholder: String,
    open: bool,
    hover_index: Option<usize>,
    on_select: Option<Box<dyn FnMut(usize, &DropdownItem) + Send>>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Dropdown {
    pub fn new() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            items: Vec::new(),
            selected: None,
            placeholder: "Select…".into(),
            open: false,
            hover_index: None,
            on_select: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn with_items(mut self, items: Vec<DropdownItem>) -> Self {
        self.items = items;
        self
    }

    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self
    }

    pub fn with_placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn on_select(mut self, f: impl FnMut(usize, &DropdownItem) + Send + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    pub fn selected_item(&self) -> Option<&DropdownItem> {
        self.selected.and_then(|i| self.items.get(i))
    }

    pub fn is_open(&self) -> bool { self.open }

    fn item_height(theme: &UiTheme) -> f32 {
        theme.font_size + 12.0
    }

    fn popup_height(&self, theme: &UiTheme) -> f32 {
        let ih = Self::item_height(theme);
        let count = self.items.len().min(8) as f32; // max 8 visible
        count * ih + 4.0 // padding
    }

    fn select_index(&mut self, idx: usize) {
        if idx < self.items.len() && self.items[idx].enabled {
            self.selected = Some(idx);
            self.open = false;
            if let Some(cb) = &mut self.on_select {
                let item = self.items[idx].clone();
                cb(idx, &item);
            }
        }
    }
}

impl Default for Dropdown {
    fn default() -> Self { Self::new() }
}

impl Widget for Dropdown {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let w = 200.0;
        let h = theme.font_size + 16.0;
        let (w, h) = constraints.clamp(w, h);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x; self.y = y; self.width = w; self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let radius = theme.radius_md;

        // Main button area
        let bg = if self.state.hovered || self.open { colors.surface_hover } else { colors.surface };
        painter.fill_rounded_rect(self.x, self.y, self.width, self.height, radius, bg);
        painter.stroke_rounded_rect(self.x, self.y, self.width, self.height, radius, colors.border, 1.0);

        // Selected text / placeholder
        let padding = 10.0;
        let text_y = self.y + (self.height - theme.font_size) / 2.0;
        let display_text = self.selected_item()
            .map(|i| i.label.as_str())
            .unwrap_or(&self.placeholder);
        let text_color = if self.selected.is_some() { colors.text_primary } else { colors.text_secondary };
        painter.draw_text(display_text, self.x + padding, text_y, theme.font_size, text_color, &theme.font_family, false);

        // Dropdown arrow (chevron)
        let arrow_x = self.x + self.width - 20.0;
        let arrow_y = self.y + self.height / 2.0;
        painter.draw_line(arrow_x, arrow_y - 3.0, arrow_x + 5.0, arrow_y + 2.0, colors.text_secondary, 1.5);
        painter.draw_line(arrow_x + 5.0, arrow_y + 2.0, arrow_x + 10.0, arrow_y - 3.0, colors.text_secondary, 1.5);

        // Focus ring
        if self.state.focused && !self.open {
            painter.stroke_rounded_rect(
                self.x - 1.5, self.y - 1.5, self.width + 3.0, self.height + 3.0,
                radius + 1.0, colors.focus_ring, 1.5,
            );
        }

        // Popup
        if self.open {
            let ih = Self::item_height(theme);
            let popup_y = self.y + self.height + 2.0;
            let popup_h = self.popup_height(theme);

            // Popup background + shadow
            painter.fill_rounded_rect(self.x, popup_y, self.width, popup_h, radius, colors.surface_elevated);
            painter.stroke_rounded_rect(self.x, popup_y, self.width, popup_h, radius, colors.border, 1.0);

            // Items
            for (i, item) in self.items.iter().enumerate().take(8) {
                let iy = popup_y + 2.0 + i as f32 * ih;
                let is_hover = self.hover_index == Some(i);
                let is_selected = self.selected == Some(i);

                if is_hover || is_selected {
                    let highlight = if is_selected { colors.accent } else { colors.surface_hover };
                    painter.fill_rounded_rect(self.x + 2.0, iy, self.width - 4.0, ih, radius * 0.5, highlight);
                }

                let tc = if !item.enabled {
                    colors.text_disabled
                } else if is_selected {
                    colors.text_on_accent
                } else {
                    colors.text_primary
                };
                painter.draw_text(&item.label, self.x + padding, iy + (ih - theme.font_size) / 2.0, theme.font_size, tc, &theme.font_family, false);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => {
                self.state.hovered = false;
                self.hover_index = None;
                EventResponse::Consumed
            }
            Event::MouseDown { x, y, .. } => {
                self.state.pressed = true;
                if self.open {
                    // Check if click is on an item in the popup
                    let ih = 28.0;  // approximate
                    let popup_y = self.y + self.height + 2.0;
                    if *y >= popup_y {
                        let idx = ((*y - popup_y - 2.0) / ih) as usize;
                        if idx < self.items.len().min(8) {
                            self.select_index(idx);
                            return EventResponse::Consumed;
                        }
                    }
                    self.open = false;
                } else if self.state.enabled {
                    self.open = true;
                }
                EventResponse::RequestFocus
            }
            Event::MouseUp { .. } => { self.state.pressed = false; EventResponse::Consumed }
            Event::FocusIn => { self.state.focused = true; EventResponse::Consumed }
            Event::FocusOut => {
                self.state.focused = false;
                self.open = false;
                EventResponse::Consumed
            }
            Event::MouseMove { x: _, y } if self.open => {
                let ih = 28.0;
                let popup_y = self.y + self.height + 2.0;
                if *y >= popup_y {
                    let idx = ((*y - popup_y - 2.0) / ih) as usize;
                    self.hover_index = if idx < self.items.len().min(8) { Some(idx) } else { None };
                } else {
                    self.hover_index = None;
                }
                EventResponse::Consumed
            }
            Event::KeyDown { key, .. } if self.state.focused => {
                match key {
                    Key::Space | Key::Enter => {
                        if self.open {
                            if let Some(idx) = self.hover_index {
                                self.select_index(idx);
                            } else {
                                self.open = false;
                            }
                        } else {
                            self.open = true;
                        }
                        EventResponse::Consumed
                    }
                    Key::ArrowDown if self.open => {
                        let max = self.items.len().min(8);
                        self.hover_index = Some(self.hover_index.map(|i| (i + 1).min(max - 1)).unwrap_or(0));
                        EventResponse::Consumed
                    }
                    Key::ArrowUp if self.open => {
                        self.hover_index = self.hover_index.map(|i| i.saturating_sub(1)).or(Some(0));
                        EventResponse::Consumed
                    }
                    Key::Escape if self.open => {
                        self.open = false;
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }
            _ => EventResponse::Ignored,
        }
    }
}

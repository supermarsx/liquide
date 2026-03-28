use crate::{Dialog, DialogId, DialogResult};

/// A font family with its available styles
#[derive(Debug, Clone)]
pub struct FontFamily {
    pub name: String,
    pub styles: Vec<FontStyle>,
}

/// A specific font style within a family
#[derive(Debug, Clone, PartialEq)]
pub struct FontStyle {
    pub weight: FontWeight,
    pub style: FontSlant,
    pub stretch: FontStretch,
}

/// Font weight (100-900 scale)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontWeight {
    Thin,       // 100
    ExtraLight, // 200
    Light,      // 300
    Regular,    // 400
    Medium,     // 500
    SemiBold,   // 600
    Bold,       // 700
    ExtraBold,  // 800
    Black,      // 900
}

impl FontWeight {
    pub fn numeric(&self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::ExtraLight => 200,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
            Self::Black => 900,
        }
    }

    pub fn from_numeric(n: u16) -> Self {
        match n {
            0..=149 => Self::Thin,
            150..=249 => Self::ExtraLight,
            250..=349 => Self::Light,
            350..=449 => Self::Regular,
            450..=549 => Self::Medium,
            550..=649 => Self::SemiBold,
            650..=749 => Self::Bold,
            750..=849 => Self::ExtraBold,
            _ => Self::Black,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Thin => "Thin",
            Self::ExtraLight => "Extra Light",
            Self::Light => "Light",
            Self::Regular => "Regular",
            Self::Medium => "Medium",
            Self::SemiBold => "Semi Bold",
            Self::Bold => "Bold",
            Self::ExtraBold => "Extra Bold",
            Self::Black => "Black",
        }
    }
}

/// Font slant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSlant {
    Normal,
    Italic,
    Oblique,
}

impl FontSlant {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
        }
    }
}

/// Font stretch / width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

/// The result of confirming a font pick
#[derive(Debug, Clone, PartialEq)]
pub struct FontSelection {
    pub family: String,
    pub size: f32,
    pub weight: FontWeight,
    pub slant: FontSlant,
}

/// Font picker state machine
#[derive(Debug)]
pub struct FontPickerState {
    pub id: DialogId,
    pub title: String,
    pub available_fonts: Vec<FontFamily>,
    pub filtered_indices: Vec<usize>,
    pub selected_family: Option<usize>,
    pub selected_size: f32,
    pub selected_weight: FontWeight,
    pub selected_slant: FontSlant,
    pub preview_text: String,
    pub search_query: String,
}

/// Minimum and maximum allowed font sizes
pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 144.0;

/// Common font sizes for the quick-pick list
pub const COMMON_SIZES: &[f32] = &[
    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 64.0,
    72.0, 96.0, 144.0,
];

impl FontPickerState {
    pub fn new(id: DialogId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            available_fonts: Vec::new(),
            filtered_indices: Vec::new(),
            selected_family: None,
            selected_size: 12.0,
            selected_weight: FontWeight::Regular,
            selected_slant: FontSlant::Normal,
            preview_text: "The quick brown fox jumps over the lazy dog.".into(),
            search_query: String::new(),
        }
    }

    /// Load font families into the picker
    pub fn set_fonts(&mut self, fonts: Vec<FontFamily>) {
        self.available_fonts = fonts;
        self.update_filtered();
    }

    /// Filter fonts by search query
    pub fn filter_fonts(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.update_filtered();
    }

    fn update_filtered(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.available_fonts.len()).collect();
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered_indices = self
                .available_fonts
                .iter()
                .enumerate()
                .filter(|(_, f)| f.name.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Select a font family by index into filtered list
    pub fn set_family(&mut self, filtered_index: usize) {
        if filtered_index < self.filtered_indices.len() {
            self.selected_family = Some(self.filtered_indices[filtered_index]);
        }
    }

    /// Set the font size, clamped to valid range
    pub fn set_size(&mut self, size: f32) {
        self.selected_size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    }

    /// Set the font weight
    pub fn set_weight(&mut self, weight: FontWeight) {
        self.selected_weight = weight;
    }

    /// Set the font slant
    pub fn set_slant(&mut self, slant: FontSlant) {
        self.selected_slant = slant;
    }

    /// Set the preview text
    pub fn set_preview_text(&mut self, text: impl Into<String>) {
        self.preview_text = text.into();
    }

    /// Get the currently selected family name
    pub fn selected_family_name(&self) -> Option<&str> {
        self.selected_family
            .and_then(|i| self.available_fonts.get(i))
            .map(|f| f.name.as_str())
    }

    /// Get available styles for the currently selected family
    pub fn available_styles(&self) -> &[FontStyle] {
        self.selected_family
            .and_then(|i| self.available_fonts.get(i))
            .map(|f| f.styles.as_slice())
            .unwrap_or(&[])
    }

    /// Confirm selection
    pub fn confirm(&self) -> DialogResult<FontSelection> {
        if let Some(name) = self.selected_family_name() {
            DialogResult::Ok(FontSelection {
                family: name.to_string(),
                size: self.selected_size,
                weight: self.selected_weight,
                slant: self.selected_slant,
            })
        } else {
            DialogResult::Cancelled
        }
    }
}

impl Dialog for FontPickerState {
    type Output = FontSelection;
    fn id(&self) -> DialogId {
        self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fonts() -> Vec<FontFamily> {
        vec![
            FontFamily {
                name: "Arial".into(),
                styles: vec![
                    FontStyle {
                        weight: FontWeight::Regular,
                        style: FontSlant::Normal,
                        stretch: FontStretch::Normal,
                    },
                    FontStyle {
                        weight: FontWeight::Bold,
                        style: FontSlant::Normal,
                        stretch: FontStretch::Normal,
                    },
                    FontStyle {
                        weight: FontWeight::Regular,
                        style: FontSlant::Italic,
                        stretch: FontStretch::Normal,
                    },
                ],
            },
            FontFamily {
                name: "Courier New".into(),
                styles: vec![FontStyle {
                    weight: FontWeight::Regular,
                    style: FontSlant::Normal,
                    stretch: FontStretch::Normal,
                }],
            },
            FontFamily {
                name: "Times New Roman".into(),
                styles: vec![
                    FontStyle {
                        weight: FontWeight::Regular,
                        style: FontSlant::Normal,
                        stretch: FontStretch::Normal,
                    },
                    FontStyle {
                        weight: FontWeight::Bold,
                        style: FontSlant::Normal,
                        stretch: FontStretch::Normal,
                    },
                ],
            },
        ]
    }

    #[test]
    fn test_new_picker() {
        let picker = FontPickerState::new(DialogId(1), "Choose Font");
        assert_eq!(picker.title, "Choose Font");
        assert_eq!(picker.selected_size, 12.0);
        assert_eq!(picker.selected_weight, FontWeight::Regular);
        assert_eq!(picker.selected_slant, FontSlant::Normal);
        assert!(picker.selected_family.is_none());
    }

    #[test]
    fn test_set_fonts() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());
        assert_eq!(picker.available_fonts.len(), 3);
        assert_eq!(picker.filtered_indices.len(), 3);
    }

    #[test]
    fn test_filter_fonts() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());

        picker.filter_fonts("arial");
        assert_eq!(picker.filtered_indices.len(), 1);
        assert_eq!(picker.available_fonts[picker.filtered_indices[0]].name, "Arial");

        picker.filter_fonts("new");
        assert_eq!(picker.filtered_indices.len(), 2); // Courier New, Times New Roman

        picker.filter_fonts("");
        assert_eq!(picker.filtered_indices.len(), 3);
    }

    #[test]
    fn test_set_family() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());
        picker.set_family(1); // Courier New
        assert_eq!(picker.selected_family_name(), Some("Courier New"));
    }

    #[test]
    fn test_set_size_clamped() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_size(4.0);
        assert_eq!(picker.selected_size, MIN_FONT_SIZE);
        picker.set_size(200.0);
        assert_eq!(picker.selected_size, MAX_FONT_SIZE);
        picker.set_size(24.0);
        assert_eq!(picker.selected_size, 24.0);
    }

    #[test]
    fn test_available_styles() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());
        picker.set_family(0); // Arial
        assert_eq!(picker.available_styles().len(), 3);
        picker.set_family(1); // Courier New
        assert_eq!(picker.available_styles().len(), 1);
    }

    #[test]
    fn test_confirm_no_selection() {
        let picker = FontPickerState::new(DialogId(1), "Font");
        assert_eq!(picker.confirm(), DialogResult::Cancelled);
    }

    #[test]
    fn test_confirm_with_selection() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());
        picker.set_family(0);
        picker.set_size(16.0);
        picker.set_weight(FontWeight::Bold);
        picker.set_slant(FontSlant::Italic);

        match picker.confirm() {
            DialogResult::Ok(sel) => {
                assert_eq!(sel.family, "Arial");
                assert_eq!(sel.size, 16.0);
                assert_eq!(sel.weight, FontWeight::Bold);
                assert_eq!(sel.slant, FontSlant::Italic);
            }
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn test_font_weight_numeric() {
        assert_eq!(FontWeight::Regular.numeric(), 400);
        assert_eq!(FontWeight::Bold.numeric(), 700);
        assert_eq!(FontWeight::from_numeric(400), FontWeight::Regular);
        assert_eq!(FontWeight::from_numeric(700), FontWeight::Bold);
        assert_eq!(FontWeight::from_numeric(150), FontWeight::ExtraLight);
    }

    #[test]
    fn test_font_weight_labels() {
        assert_eq!(FontWeight::Regular.label(), "Regular");
        assert_eq!(FontWeight::Bold.label(), "Bold");
        assert_eq!(FontSlant::Italic.label(), "Italic");
    }

    #[test]
    fn test_filter_then_select() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        picker.set_fonts(sample_fonts());
        picker.filter_fonts("times");
        assert_eq!(picker.filtered_indices.len(), 1);
        picker.set_family(0); // first in filtered = Times New Roman
        assert_eq!(picker.selected_family_name(), Some("Times New Roman"));
    }

    #[test]
    fn test_preview_text() {
        let mut picker = FontPickerState::new(DialogId(1), "Font");
        assert!(picker.preview_text.contains("quick brown fox"));
        picker.set_preview_text("Hello, world!");
        assert_eq!(picker.preview_text, "Hello, world!");
    }
}

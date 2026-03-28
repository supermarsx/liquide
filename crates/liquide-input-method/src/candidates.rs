//! Candidate list data model and layout computation.

/// A single candidate entry in the candidate list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The candidate text (the string that would be committed).
    pub text: String,
    /// Optional short label (e.g. "1", "a", etc.).
    pub label: Option<String>,
    /// Optional annotation (e.g. reading, meaning, usage note).
    pub annotation: Option<String>,
}

impl Candidate {
    /// Create a candidate with just text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: None,
            annotation: None,
        }
    }

    /// Create a candidate with text and a label.
    #[must_use]
    pub fn with_label(text: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: Some(label.into()),
            annotation: None,
        }
    }

    /// Set annotation (builder pattern).
    #[must_use]
    pub fn annotated(mut self, annotation: impl Into<String>) -> Self {
        self.annotation = Some(annotation.into());
        self
    }
}

/// Layout metrics for a single candidate item in the candidate window.
#[derive(Debug, Clone)]
pub struct CandidateLayoutItem {
    /// Display text for this candidate.
    pub text: String,
    /// Label text (number or letter shortcut).
    pub label: String,
    /// X position relative to the candidate window.
    pub x: f32,
    /// Y position relative to the candidate window.
    pub y: f32,
    /// Width of this item.
    pub width: f32,
    /// Height of this item.
    pub height: f32,
    /// Whether this item is currently selected.
    pub selected: bool,
}

/// Layout for the entire candidate window.
#[derive(Debug, Clone)]
pub struct CandidateLayout {
    /// X position of the window (screen coordinates).
    pub x: f32,
    /// Y position of the window (screen coordinates).
    pub y: f32,
    /// Total width of the candidate window.
    pub width: f32,
    /// Total height of the candidate window.
    pub height: f32,
    /// Individual candidate items with layout info.
    pub items: Vec<CandidateLayoutItem>,
}

/// Constants for candidate window layout.
const ITEM_HEIGHT: f32 = 28.0;
const ITEM_PADDING_X: f32 = 8.0;
const LABEL_WIDTH: f32 = 24.0;
const MIN_TEXT_WIDTH: f32 = 60.0;
const WINDOW_PADDING: f32 = 4.0;
const CHAR_WIDTH_ESTIMATE: f32 = 8.0;

/// Compute the layout for a candidate window.
///
/// Positions the window anchored at `(anchor_x, anchor_y)` — typically the
/// bottom-left of the preedit text. Shows at most `max_visible` candidates.
///
/// Returns a `CandidateLayout` with screen-coordinate positions for each item.
#[must_use]
pub fn compute_layout(
    candidates: &[Candidate],
    selected: usize,
    anchor_x: f32,
    anchor_y: f32,
    max_visible: usize,
) -> CandidateLayout {
    if candidates.is_empty() {
        return CandidateLayout {
            x: anchor_x,
            y: anchor_y,
            width: 0.0,
            height: 0.0,
            items: Vec::new(),
        };
    }

    let visible_count = candidates.len().min(max_visible);

    // Estimate the width needed based on candidate text lengths.
    let max_text_chars = candidates[..visible_count]
        .iter()
        .map(|c| c.text.chars().count())
        .max()
        .unwrap_or(0);
    let text_width = (max_text_chars as f32 * CHAR_WIDTH_ESTIMATE).max(MIN_TEXT_WIDTH);
    let item_width = LABEL_WIDTH + text_width + ITEM_PADDING_X * 2.0;
    let window_width = item_width + WINDOW_PADDING * 2.0;
    let window_height = visible_count as f32 * ITEM_HEIGHT + WINDOW_PADDING * 2.0;

    let mut items = Vec::with_capacity(visible_count);

    for (i, candidate) in candidates[..visible_count].iter().enumerate() {
        let label = candidate
            .label
            .clone()
            .unwrap_or_else(|| format!("{}", i + 1));
        let y_offset = WINDOW_PADDING + i as f32 * ITEM_HEIGHT;

        items.push(CandidateLayoutItem {
            text: candidate.text.clone(),
            label,
            x: WINDOW_PADDING,
            y: y_offset,
            width: item_width,
            height: ITEM_HEIGHT,
            selected: i == selected,
        });
    }

    CandidateLayout {
        x: anchor_x,
        y: anchor_y,
        width: window_width,
        height: window_height,
        items,
    }
}

/// Hit-test a point against candidate layout items.
/// Returns the index of the candidate under the point, if any.
/// Coordinates are relative to the candidate window origin.
#[must_use]
pub fn hit_test_candidate(layout: &CandidateLayout, rel_x: f32, rel_y: f32) -> Option<usize> {
    for (i, item) in layout.items.iter().enumerate() {
        if rel_x >= item.x
            && rel_x < item.x + item.width
            && rel_y >= item.y
            && rel_y < item.y + item.height
        {
            return Some(i);
        }
    }
    None
}

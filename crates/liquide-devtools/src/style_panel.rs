//! Style inspector — displays all computed CSS properties for a selected
//! element, grouped by category with inherited property tracking.

use liquide_dom::NodeId;
use liquide_style_engine::computed::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Display, FlexDirection,
    FlexWrap, JustifyContent, Position, Visibility,
};
use liquide_style_engine::StyleMap;
use serde::{Deserialize, Serialize};

/// A single property entry in the style inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleProperty {
    /// CSS property name (e.g., "display", "margin-left").
    pub name: String,
    /// Resolved value as a string (e.g., "block", "16px").
    pub value: String,
    /// Whether this property was inherited from a parent.
    pub inherited: bool,
    /// Category for grouping in the UI.
    pub category: StyleCategory,
}

/// Categories for grouping style properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StyleCategory {
    Layout,
    Box,
    Typography,
    Background,
    Border,
    Flex,
    Grid,
    Position,
    Visual,
    Transform,
    Animation,
    Other,
}

impl StyleCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::Box => "Box Model",
            Self::Typography => "Typography",
            Self::Background => "Background",
            Self::Border => "Border",
            Self::Flex => "Flexbox",
            Self::Grid => "Grid",
            Self::Position => "Position",
            Self::Visual => "Visual",
            Self::Transform => "Transform",
            Self::Animation => "Animation",
            Self::Other => "Other",
        }
    }

    /// Machine-readable identifier for data attributes.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Box => "box",
            Self::Typography => "typography",
            Self::Background => "background",
            Self::Border => "border",
            Self::Flex => "flex",
            Self::Grid => "grid",
            Self::Position => "position",
            Self::Visual => "visual",
            Self::Transform => "transform",
            Self::Animation => "animation",
            Self::Other => "other",
        }
    }

    /// Parse a category from its ID string.
    pub fn from_id(id: &str) -> Option<StyleCategory> {
        match id {
            "layout" => Some(Self::Layout),
            "box" => Some(Self::Box),
            "typography" => Some(Self::Typography),
            "background" => Some(Self::Background),
            "border" => Some(Self::Border),
            "flex" => Some(Self::Flex),
            "grid" => Some(Self::Grid),
            "position" => Some(Self::Position),
            "visual" => Some(Self::Visual),
            "transform" => Some(Self::Transform),
            "animation" => Some(Self::Animation),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Ordered list of all categories for display.
    pub fn all() -> &'static [StyleCategory] {
        &[
            Self::Layout,
            Self::Box,
            Self::Position,
            Self::Typography,
            Self::Background,
            Self::Border,
            Self::Flex,
            Self::Grid,
            Self::Visual,
            Self::Transform,
            Self::Animation,
            Self::Other,
        ]
    }
}

/// The style inspector: extracts and formats computed styles for display.
pub struct StyleInspector {
    /// Currently inspected node.
    selected_node: Option<NodeId>,
    /// Cached properties for the selected node.
    properties: Vec<StyleProperty>,
    /// Which categories are collapsed in the UI.
    collapsed_categories: std::collections::HashSet<StyleCategory>,
    /// Whether to show inherited properties.
    show_inherited: bool,
    /// Whether to show default/initial values.
    show_defaults: bool,
}

impl StyleInspector {
    pub fn new() -> Self {
        Self {
            selected_node: None,
            properties: Vec::new(),
            collapsed_categories: std::collections::HashSet::new(),
            show_inherited: true,
            show_defaults: false,
        }
    }

    /// Update the inspector for a new selected node.
    pub fn inspect(&mut self, node_id: NodeId, styles: &StyleMap) {
        self.selected_node = Some(node_id);
        self.properties = Self::extract_properties(node_id, styles);
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.selected_node = None;
        self.properties.clear();
    }

    /// Get the currently selected node.
    pub fn selected_node(&self) -> Option<NodeId> {
        self.selected_node
    }

    /// Get all extracted properties.
    pub fn properties(&self) -> &[StyleProperty] {
        &self.properties
    }

    /// Get properties filtered by current display settings.
    pub fn visible_properties(&self) -> Vec<&StyleProperty> {
        self.properties
            .iter()
            .filter(|p| {
                if !self.show_inherited && p.inherited {
                    return false;
                }
                if self.collapsed_categories.contains(&p.category) {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Get properties grouped by category.
    pub fn grouped_properties(&self) -> Vec<(StyleCategory, Vec<&StyleProperty>)> {
        let mut groups: Vec<(StyleCategory, Vec<&StyleProperty>)> = Vec::new();

        for cat in StyleCategory::all() {
            let props: Vec<&StyleProperty> = self
                .properties
                .iter()
                .filter(|p| {
                    p.category == *cat
                        && (self.show_inherited || !p.inherited)
                        && (self.show_defaults || p.value != "auto" && p.value != "0px" && p.value != "none")
                })
                .collect();

            if !props.is_empty() {
                groups.push((*cat, props));
            }
        }

        groups
    }

    /// Toggle category collapse.
    pub fn toggle_category(&mut self, category: StyleCategory) {
        if self.collapsed_categories.contains(&category) {
            self.collapsed_categories.remove(&category);
        } else {
            self.collapsed_categories.insert(category);
        }
    }

    /// Toggle showing inherited properties.
    pub fn set_show_inherited(&mut self, show: bool) {
        self.show_inherited = show;
    }

    /// Toggle showing default values.
    pub fn set_show_defaults(&mut self, show: bool) {
        self.show_defaults = show;
    }

    /// Extract all computed properties from a style into display format.
    fn extract_properties(node_id: NodeId, styles: &StyleMap) -> Vec<StyleProperty> {
        let style = match styles.get(node_id) {
            Some(s) => s,
            None => return vec![],
        };

        let mut props = Vec::with_capacity(80);

        // ── Layout ──
        props.push(prop("display", format_display(style.display), StyleCategory::Layout));
        props.push(prop("visibility", format_visibility(style.visibility), StyleCategory::Layout));
        props.push(prop("box-sizing", format_box_sizing(style.box_sizing), StyleCategory::Layout));
        props.push(prop("overflow-x", format!("{:?}", style.overflow_x), StyleCategory::Layout));
        props.push(prop("overflow-y", format!("{:?}", style.overflow_y), StyleCategory::Layout));
        if style.float != Default::default() {
            props.push(prop("float", format!("{:?}", style.float), StyleCategory::Layout));
        }
        if style.clear != Default::default() {
            props.push(prop("clear", format!("{:?}", style.clear), StyleCategory::Layout));
        }

        // ── Box Model ──
        props.push(prop("width", format_dim(&style.width), StyleCategory::Box));
        props.push(prop("height", format_dim(&style.height), StyleCategory::Box));
        props.push(prop("min-width", format_dim(&style.min_width), StyleCategory::Box));
        props.push(prop("min-height", format_dim(&style.min_height), StyleCategory::Box));
        props.push(prop("max-width", format_dim(&style.max_width), StyleCategory::Box));
        props.push(prop("max-height", format_dim(&style.max_height), StyleCategory::Box));
        props.push(prop("margin-top", format_dim(&style.margin.top), StyleCategory::Box));
        props.push(prop("margin-right", format_dim(&style.margin.right), StyleCategory::Box));
        props.push(prop("margin-bottom", format_dim(&style.margin.bottom), StyleCategory::Box));
        props.push(prop("margin-left", format_dim(&style.margin.left), StyleCategory::Box));
        props.push(prop("padding-top", format_dim(&style.padding.top), StyleCategory::Box));
        props.push(prop("padding-right", format_dim(&style.padding.right), StyleCategory::Box));
        props.push(prop("padding-bottom", format_dim(&style.padding.bottom), StyleCategory::Box));
        props.push(prop("padding-left", format_dim(&style.padding.left), StyleCategory::Box));

        // ── Position ──
        props.push(prop("position", format_position(style.position), StyleCategory::Position));
        props.push(prop("top", format_dim(&style.top), StyleCategory::Position));
        props.push(prop("right", format_dim(&style.right), StyleCategory::Position));
        props.push(prop("bottom", format_dim(&style.bottom), StyleCategory::Position));
        props.push(prop("left", format_dim(&style.left), StyleCategory::Position));
        props.push(prop("z-index", match style.z_index { Some(z) => format!("{}", z), None => "auto".to_string() }, StyleCategory::Position));

        // ── Typography ──
        props.push(prop("font-size", format!("{}px", style.font_size), StyleCategory::Typography));
        props.push(prop("font-weight", format!("{}", style.font_weight), StyleCategory::Typography));
        props.push(prop("font-family", style.font_family.join(", "), StyleCategory::Typography));
        props.push(prop("line-height", format!("{:?}", style.line_height), StyleCategory::Typography));
        props.push(prop("text-align", format!("{:?}", style.text_align), StyleCategory::Typography));
        props.push(prop("color", format_color(&style.color), StyleCategory::Typography));
        props.push(prop("white-space", format!("{:?}", style.white_space), StyleCategory::Typography));
        props.push(prop("letter-spacing", format!("{}px", style.letter_spacing), StyleCategory::Typography));
        props.push(prop("word-spacing", format!("{}px", style.word_spacing), StyleCategory::Typography));
        props.push(prop("text-indent", format!("{}px", style.text_indent), StyleCategory::Typography));

        // ── Background ──
        props.push(prop("background-color", format_color(&style.background_color), StyleCategory::Background));
        props.push(prop("opacity", format!("{}", style.opacity), StyleCategory::Background));

        // ── Border ──
        props.push(prop("border-top-width", format!("{}px", style.border_width.top), StyleCategory::Border));
        props.push(prop("border-right-width", format!("{}px", style.border_width.right), StyleCategory::Border));
        props.push(prop("border-bottom-width", format!("{}px", style.border_width.bottom), StyleCategory::Border));
        props.push(prop("border-left-width", format!("{}px", style.border_width.left), StyleCategory::Border));
        props.push(prop("border-top-color", format_color(&style.border_color.top), StyleCategory::Border));
        props.push(prop("border-top-style", format!("{:?}", style.border_style.top), StyleCategory::Border));
        props.push(prop("border-radius-tl", format!("{}px", style.border_radius.top_left), StyleCategory::Border));
        props.push(prop("border-radius-tr", format!("{}px", style.border_radius.top_right), StyleCategory::Border));
        props.push(prop("border-radius-bl", format!("{}px", style.border_radius.bottom_left), StyleCategory::Border));
        props.push(prop("border-radius-br", format!("{}px", style.border_radius.bottom_right), StyleCategory::Border));

        // ── Flexbox ──
        if matches!(style.display, Display::Flex | Display::InlineFlex) {
            props.push(prop("flex-direction", format_flex_dir(style.flex_direction), StyleCategory::Flex));
            props.push(prop("flex-wrap", format_flex_wrap(style.flex_wrap), StyleCategory::Flex));
            props.push(prop("justify-content", format_justify(style.justify_content), StyleCategory::Flex));
            props.push(prop("align-items", format_align_items(style.align_items), StyleCategory::Flex));
            props.push(prop("align-content", format_align_content(style.align_content), StyleCategory::Flex));
            props.push(prop("gap", format!("{:?} {:?}", style.row_gap, style.column_gap), StyleCategory::Flex));
        }
        props.push(prop("flex-grow", format!("{}", style.flex_grow), StyleCategory::Flex));
        props.push(prop("flex-shrink", format!("{}", style.flex_shrink), StyleCategory::Flex));
        props.push(prop("flex-basis", format_dim(&style.flex_basis), StyleCategory::Flex));
        props.push(prop("align-self", format_align_self(style.align_self), StyleCategory::Flex));
        props.push(prop("order", format!("{}", style.order), StyleCategory::Flex));

        // ── Grid ──
        if matches!(style.display, Display::Grid | Display::InlineGrid) {
            props.push(prop("grid-template-columns", format!("{:?}", style.grid_template_columns), StyleCategory::Grid));
            props.push(prop("grid-template-rows", format!("{:?}", style.grid_template_rows), StyleCategory::Grid));
            props.push(prop("grid-auto-flow", format!("{:?}", style.grid_auto_flow), StyleCategory::Grid));
            props.push(prop("gap", format!("{:?} {:?}", style.row_gap, style.column_gap), StyleCategory::Grid));
        }

        // ── Visual ──
        props.push(prop("cursor", format!("{:?}", style.cursor), StyleCategory::Visual));
        props.push(prop("pointer-events", format!("{:?}", style.pointer_events), StyleCategory::Visual));

        // ── Transform ──
        if !style.transform.is_empty() {
            props.push(prop("transform", format!("{:?}", style.transform), StyleCategory::Transform));
        }
        if !style.transition.is_empty() {
            props.push(prop("transition", format!("{} transitions", style.transition.len()), StyleCategory::Animation));
        }

        props
    }

    /// Export as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.properties).unwrap_or_default()
    }
}

impl Default for StyleInspector {
    fn default() -> Self {
        Self::new()
    }
}

// ── Formatting helpers ──

fn prop(name: &str, value: String, category: StyleCategory) -> StyleProperty {
    StyleProperty {
        name: name.to_string(),
        value,
        inherited: false,
        category,
    }
}

fn format_display(d: Display) -> String {
    match d {
        Display::Block => "block".into(),
        Display::Inline => "inline".into(),
        Display::InlineBlock => "inline-block".into(),
        Display::Flex => "flex".into(),
        Display::InlineFlex => "inline-flex".into(),
        Display::Grid => "grid".into(),
        Display::InlineGrid => "inline-grid".into(),
        Display::Table => "table".into(),
        Display::TableRow => "table-row".into(),
        Display::TableCell => "table-cell".into(),
        Display::TableRowGroup => "table-row-group".into(),
        Display::TableHeaderGroup => "table-header-group".into(),
        Display::TableFooterGroup => "table-footer-group".into(),
        Display::TableColumn => "table-column".into(),
        Display::TableColumnGroup => "table-column-group".into(),
        Display::TableCaption => "table-caption".into(),
        Display::None => "none".into(),
        Display::Contents => "contents".into(),
        Display::FlowRoot => "flow-root".into(),
        Display::ListItem => "list-item".into(),
        Display::Ruby => "ruby".into(),
        Display::RubyText => "ruby-text".into(),
        Display::RunIn => "run-in".into(),
    }
}

fn format_position(p: Position) -> String {
    match p {
        Position::Static => "static".into(),
        Position::Relative => "relative".into(),
        Position::Absolute => "absolute".into(),
        Position::Fixed => "fixed".into(),
        Position::Sticky => "sticky".into(),
    }
}

fn format_visibility(v: Visibility) -> String {
    match v {
        Visibility::Visible => "visible".into(),
        Visibility::Hidden => "hidden".into(),
        Visibility::Collapse => "collapse".into(),
    }
}

fn format_box_sizing(bs: BoxSizing) -> String {
    match bs {
        BoxSizing::ContentBox => "content-box".into(),
        BoxSizing::BorderBox => "border-box".into(),
    }
}

fn format_flex_dir(d: FlexDirection) -> String {
    match d {
        FlexDirection::Row => "row".into(),
        FlexDirection::RowReverse => "row-reverse".into(),
        FlexDirection::Column => "column".into(),
        FlexDirection::ColumnReverse => "column-reverse".into(),
    }
}

fn format_flex_wrap(w: FlexWrap) -> String {
    match w {
        FlexWrap::NoWrap => "nowrap".into(),
        FlexWrap::Wrap => "wrap".into(),
        FlexWrap::WrapReverse => "wrap-reverse".into(),
    }
}

fn format_justify(j: JustifyContent) -> String {
    match j {
        JustifyContent::FlexStart => "flex-start".into(),
        JustifyContent::FlexEnd => "flex-end".into(),
        JustifyContent::Center => "center".into(),
        JustifyContent::SpaceBetween => "space-between".into(),
        JustifyContent::SpaceAround => "space-around".into(),
        JustifyContent::SpaceEvenly => "space-evenly".into(),
    }
}

fn format_align_items(a: AlignItems) -> String {
    match a {
        AlignItems::FlexStart => "flex-start".into(),
        AlignItems::FlexEnd => "flex-end".into(),
        AlignItems::Center => "center".into(),
        AlignItems::Baseline => "baseline".into(),
        AlignItems::Stretch => "stretch".into(),
    }
}

fn format_align_content(a: AlignContent) -> String {
    match a {
        AlignContent::FlexStart => "flex-start".into(),
        AlignContent::FlexEnd => "flex-end".into(),
        AlignContent::Center => "center".into(),
        AlignContent::SpaceBetween => "space-between".into(),
        AlignContent::SpaceAround => "space-around".into(),
        AlignContent::Stretch => "stretch".into(),
    }
}

fn format_align_self(a: AlignSelf) -> String {
    match a {
        AlignSelf::Auto => "auto".into(),
        AlignSelf::FlexStart => "flex-start".into(),
        AlignSelf::FlexEnd => "flex-end".into(),
        AlignSelf::Center => "center".into(),
        AlignSelf::Baseline => "baseline".into(),
        AlignSelf::Stretch => "stretch".into(),
    }
}

use liquide_compositor::pixel::Color;

fn format_color(c: &Color) -> String {
    if c.a == 255 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        format!("rgba({}, {}, {}, {:.2})", c.r, c.g, c.b, c.a as f32 / 255.0)
    }
}

use liquide_style_engine::dimension::Dimension;

fn format_dim(d: &Dimension) -> String {
    match d {
        Dimension::Px(v) => format!("{v}px"),
        Dimension::Percent(v) => format!("{v}%"),
        Dimension::Em(v) => format!("{v}em"),
        Dimension::Rem(v) => format!("{v}rem"),
        Dimension::Vw(v) => format!("{v}vw"),
        Dimension::Vh(v) => format!("{v}vh"),
        Dimension::Vmin(v) => format!("{v}vmin"),
        Dimension::Vmax(v) => format!("{v}vmax"),
        Dimension::Ch(v) => format!("{v}ch"),
        Dimension::Auto => "auto".into(),
        Dimension::None => "none".into(),
        Dimension::Zero => "0".into(),
        Dimension::MaxContent => "max-content".into(),
        Dimension::MinContent => "min-content".into(),
        Dimension::FitContent(inner) => format!("fit-content({})", format_dim(inner)),
        Dimension::Calc(_) => "calc(…)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_display() {
        assert_eq!(format_display(Display::Block), "block");
        assert_eq!(format_display(Display::Flex), "flex");
        assert_eq!(format_display(Display::Contents), "contents");
    }

    #[test]
    fn test_format_color() {
        let c = Color::new(255, 0, 128, 255);
        assert_eq!(format_color(&c), "rgb(255, 0, 128)");

        let c2 = Color::new(255, 0, 128, 128);
        assert!(format_color(&c2).starts_with("rgba(255, 0, 128,"));
    }

    #[test]
    fn test_category_labels() {
        assert_eq!(StyleCategory::Layout.label(), "Layout");
        assert_eq!(StyleCategory::Box.label(), "Box Model");
    }
}

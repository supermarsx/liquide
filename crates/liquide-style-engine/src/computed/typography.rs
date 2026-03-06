//! Text and font enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LineHeight {
    Normal,
    Number(f32),
    Px(f32),
}

impl Default for LineHeight {
    fn default() -> Self {
        LineHeight::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignLast {
    Auto,
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

impl Default for TextAlignLast {
    fn default() -> Self {
        TextAlignLast::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextJustify {
    Auto,
    InterCharacter,
    InterWord,
    None,
}

impl Default for TextJustify {
    fn default() -> Self {
        TextJustify::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextTransform {
    None,
    Capitalize,
    Uppercase,
    Lowercase,
}

impl Default for TextTransform {
    fn default() -> Self {
        TextTransform::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

impl Default for TextOverflow {
    fn default() -> Self {
        TextOverflow::Clip
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

impl Default for WhiteSpace {
    fn default() -> Self {
        WhiteSpace::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordBreak {
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

impl Default for WordBreak {
    fn default() -> Self {
        WordBreak::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowWrap {
    Normal,
    BreakWord,
    Anywhere,
}

impl Default for OverflowWrap {
    fn default() -> Self {
        OverflowWrap::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hyphens {
    None,
    Manual,
    Auto,
}

impl Default for Hyphens {
    fn default() -> Self {
        Hyphens::Manual
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerticalAlign {
    Baseline,
    Sub,
    Super,
    Top,
    TextTop,
    Middle,
    Bottom,
    TextBottom,
    Length(f32),
}

impl Default for VerticalAlign {
    fn default() -> Self {
        VerticalAlign::Baseline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextRendering {
    Auto,
    OptimizeSpeed,
    OptimizeLegibility,
    GeometricPrecision,
}

impl Default for TextRendering {
    fn default() -> Self {
        TextRendering::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl Default for FontStretch {
    fn default() -> Self {
        FontStretch::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontKerning {
    Auto,
    Normal,
    None,
}

impl Default for FontKerning {
    fn default() -> Self {
        FontKerning::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantCaps {
    Normal,
    SmallCaps,
    AllSmallCaps,
    PetiteCaps,
    AllPetiteCaps,
    Unicase,
    TitlingCaps,
}

impl Default for FontVariantCaps {
    fn default() -> Self {
        FontVariantCaps::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantNumeric {
    Normal,
    OldstyleNums,
    LiningNums,
    TabularNums,
    ProportionalNums,
}

impl Default for FontVariantNumeric {
    fn default() -> Self {
        FontVariantNumeric::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontOpticalSizing {
    Auto,
    None,
}

impl Default for FontOpticalSizing {
    fn default() -> Self {
        FontOpticalSizing::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FontSizeAdjust {
    None,
    Number(f32),
}

impl Default for FontSizeAdjust {
    fn default() -> Self {
        FontSizeAdjust::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineClamp {
    None,
    Count(u32),
}

impl Default for LineClamp {
    fn default() -> Self {
        LineClamp::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorationSkipInk {
    Auto,
    All,
    None,
}

impl Default for TextDecorationSkipInk {
    fn default() -> Self {
        TextDecorationSkipInk::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextUnderlinePosition {
    Auto,
    Under,
    Left,
    Right,
    FromFont,
}

impl Default for TextUnderlinePosition {
    fn default() -> Self {
        TextUnderlinePosition::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantAlternates {
    Normal,
    HistoricalForms,
}
impl Default for FontVariantAlternates {
    fn default() -> Self {
        FontVariantAlternates::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantEastAsian {
    Normal,
    Jis78,
    Jis83,
    Jis90,
    Jis04,
    Simplified,
    Traditional,
    FullWidth,
    ProportionalWidth,
    Ruby,
}
impl Default for FontVariantEastAsian {
    fn default() -> Self {
        FontVariantEastAsian::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantLigatures {
    Normal,
    None,
    CommonLigatures,
    NoCommonLigatures,
    DiscretionaryLigatures,
    NoDiscretionaryLigatures,
    HistoricalLigatures,
    NoHistoricalLigatures,
    Contextual,
    NoContextual,
}
impl Default for FontVariantLigatures {
    fn default() -> Self {
        FontVariantLigatures::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantPosition {
    Normal,
    Sub,
    Super,
}
impl Default for FontVariantPosition {
    fn default() -> Self {
        FontVariantPosition::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontVariantEmoji {
    Normal,
    Text,
    Emoji,
    Unicode,
}
impl Default for FontVariantEmoji {
    fn default() -> Self {
        FontVariantEmoji::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisWeight {
    Auto,
    None,
}
impl Default for FontSynthesisWeight {
    fn default() -> Self {
        FontSynthesisWeight::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisStyle {
    Auto,
    None,
}
impl Default for FontSynthesisStyle {
    fn default() -> Self {
        FontSynthesisStyle::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSynthesisSmallCaps {
    Auto,
    None,
}
impl Default for FontSynthesisSmallCaps {
    fn default() -> Self {
        FontSynthesisSmallCaps::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOrientation {
    Mixed,
    Upright,
    Sideways,
}
impl Default for TextOrientation {
    fn default() -> Self {
        TextOrientation::Mixed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextCombineUpright {
    None,
    All,
    Digits(u8),
}
impl Default for TextCombineUpright {
    fn default() -> Self {
        TextCombineUpright::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextWrapMode {
    Wrap,
    NoWrap,
}
impl Default for TextWrapMode {
    fn default() -> Self {
        TextWrapMode::Wrap
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextWrapStyle {
    Auto,
    Balance,
    Pretty,
    Stable,
}
impl Default for TextWrapStyle {
    fn default() -> Self {
        TextWrapStyle::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextBoxTrim {
    None,
    TrimStart,
    TrimEnd,
    TrimBoth,
}
impl Default for TextBoxTrim {
    fn default() -> Self {
        TextBoxTrim::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhiteSpaceCollapse {
    Collapse,
    Preserve,
    PreserveBreaks,
    PreserveSpaces,
    BreakSpaces,
}
impl Default for WhiteSpaceCollapse {
    fn default() -> Self {
        WhiteSpaceCollapse::Collapse
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineBreak {
    Auto,
    Loose,
    Normal,
    Strict,
    Anywhere,
}
impl Default for LineBreak {
    fn default() -> Self {
        LineBreak::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageRendering {
    Auto,
    CrispEdges,
    Pixelated,
    HighQuality,
    Smooth,
}

impl Default for ImageRendering {
    fn default() -> Self {
        ImageRendering::Auto
    }
}

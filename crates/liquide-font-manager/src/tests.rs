//! Tests for the font-manager crate.

use crate::fallback::{FallbackChain, FontFallback};
use crate::font_info::FontInfo;
use crate::format::FontFormat;
use crate::manager::FontManager;
use crate::preview::{FontPreview, PreviewConfig};
use crate::stretch::FontStretch;
use crate::style::FontStyle;
use crate::unicode_block::UnicodeBlock;
use crate::weight::FontWeight;

// ── FontWeight ───────────────────────────────────────────────────────

#[test]
fn weight_from_value_exact() {
    assert_eq!(FontWeight::from_value(100), FontWeight::Thin);
    assert_eq!(FontWeight::from_value(400), FontWeight::Regular);
    assert_eq!(FontWeight::from_value(700), FontWeight::Bold);
    assert_eq!(FontWeight::from_value(900), FontWeight::Black);
}

#[test]
fn weight_from_value_rounded() {
    assert_eq!(FontWeight::from_value(150), FontWeight::ExtraLight);
    assert_eq!(FontWeight::from_value(349), FontWeight::Light);
    assert_eq!(FontWeight::from_value(350), FontWeight::Regular);
    assert_eq!(FontWeight::from_value(450), FontWeight::Medium);
}

#[test]
fn weight_from_value_clamped() {
    assert_eq!(FontWeight::from_value(0), FontWeight::Thin);
    assert_eq!(FontWeight::from_value(50), FontWeight::Thin);
    assert_eq!(FontWeight::from_value(1000), FontWeight::Black);
}

#[test]
fn weight_value_roundtrip() {
    for w in FontWeight::ALL {
        assert_eq!(FontWeight::from_value(w.value()), w);
    }
}

#[test]
fn weight_from_style_name() {
    assert_eq!(FontWeight::from_style_name("Bold"), FontWeight::Bold);
    assert_eq!(FontWeight::from_style_name("thin"), FontWeight::Thin);
    assert_eq!(FontWeight::from_style_name("SemiBold"), FontWeight::SemiBold);
    assert_eq!(FontWeight::from_style_name("Regular"), FontWeight::Regular);
    assert_eq!(FontWeight::from_style_name("Heavy"), FontWeight::Black);
    assert_eq!(FontWeight::from_style_name("Hairline"), FontWeight::Thin);
}

#[test]
fn weight_distance() {
    assert_eq!(FontWeight::Regular.distance(FontWeight::Bold), 300);
    assert_eq!(FontWeight::Thin.distance(FontWeight::Black), 800);
    assert_eq!(FontWeight::Medium.distance(FontWeight::Medium), 0);
}

#[test]
fn weight_display() {
    let s = format!("{}", FontWeight::Bold);
    assert!(s.contains("Bold"));
    assert!(s.contains("700"));
}

// ── FontStyle ────────────────────────────────────────────────────────

#[test]
fn style_from_name() {
    assert_eq!(FontStyle::from_name("Italic"), FontStyle::Italic);
    assert_eq!(FontStyle::from_name("BoldItalic"), FontStyle::Italic);
    assert_eq!(FontStyle::from_name("Oblique"), FontStyle::Oblique);
    assert_eq!(FontStyle::from_name("Regular"), FontStyle::Regular);
    assert_eq!(FontStyle::from_name("slanted"), FontStyle::Oblique);
}

#[test]
fn style_default() {
    assert_eq!(FontStyle::default(), FontStyle::Regular);
}

// ── FontStretch ──────────────────────────────────────────────────────

#[test]
fn stretch_percentage_roundtrip() {
    for s in FontStretch::ALL {
        assert_eq!(FontStretch::from_percentage(s.percentage()), s);
    }
}

#[test]
fn stretch_from_name() {
    assert_eq!(FontStretch::from_name("Condensed"), FontStretch::Condensed);
    assert_eq!(
        FontStretch::from_name("SemiCondensed"),
        FontStretch::SemiCondensed
    );
    assert_eq!(FontStretch::from_name("Expanded"), FontStretch::Expanded);
    assert_eq!(FontStretch::from_name("Normal"), FontStretch::Normal);
    assert_eq!(FontStretch::from_name("Blah"), FontStretch::Normal);
}

#[test]
fn stretch_display() {
    let s = format!("{}", FontStretch::Condensed);
    assert!(s.contains("Condensed"));
    assert!(s.contains("75%"));
}

// ── FontFormat ───────────────────────────────────────────────────────

#[test]
fn format_from_extension() {
    assert_eq!(FontFormat::from_extension("ttf"), Some(FontFormat::TrueType));
    assert_eq!(FontFormat::from_extension("TTF"), Some(FontFormat::TrueType));
    assert_eq!(FontFormat::from_extension("otf"), Some(FontFormat::OpenType));
    assert_eq!(FontFormat::from_extension("woff"), Some(FontFormat::WOFF));
    assert_eq!(FontFormat::from_extension("woff2"), Some(FontFormat::WOFF2));
    assert_eq!(FontFormat::from_extension("pfb"), Some(FontFormat::Type1));
    assert_eq!(FontFormat::from_extension("xyz"), None);
}

#[test]
fn format_extension_name() {
    assert_eq!(FontFormat::TrueType.extension(), "ttf");
    assert_eq!(FontFormat::OpenType.name(), "OpenType");
}

// ── UnicodeBlock ─────────────────────────────────────────────────────

#[test]
fn block_for_ascii() {
    assert_eq!(
        UnicodeBlock::for_codepoint('A' as u32),
        Some(UnicodeBlock::BasicLatin)
    );
}

#[test]
fn block_for_cjk() {
    // U+4E00 is the start of CJK Unified Ideographs.
    assert_eq!(
        UnicodeBlock::for_codepoint(0x4E00),
        Some(UnicodeBlock::CJKUnified)
    );
}

#[test]
fn block_for_unknown() {
    // A private-use area code point should not match any of our blocks.
    assert_eq!(UnicodeBlock::for_codepoint(0xF0000), None);
}

#[test]
fn blocks_for_text() {
    let blocks = UnicodeBlock::blocks_for_text("Hello");
    assert_eq!(blocks, vec![UnicodeBlock::BasicLatin]);
}

#[test]
fn blocks_for_mixed_text() {
    let blocks = UnicodeBlock::blocks_for_text("Hi\u{00e9}"); // "Hié"
    assert!(blocks.contains(&UnicodeBlock::BasicLatin));
    assert!(blocks.contains(&UnicodeBlock::Latin1Supplement));
}

#[test]
fn covers_text_positive() {
    let coverage = vec![UnicodeBlock::BasicLatin, UnicodeBlock::Latin1Supplement];
    assert!(UnicodeBlock::covers_text(&coverage, "Hello world!"));
}

#[test]
fn covers_text_negative() {
    let coverage = vec![UnicodeBlock::BasicLatin];
    // \u{00e9} is in Latin-1 Supplement, not in BasicLatin.
    assert!(!UnicodeBlock::covers_text(&coverage, "\u{00e9}"));
}

#[test]
fn block_display() {
    let s = format!("{}", UnicodeBlock::BasicLatin);
    assert!(s.contains("Basic Latin"));
    assert!(s.contains("U+0000"));
}

// ── FontInfo ─────────────────────────────────────────────────────────

#[test]
fn font_info_from_path_ttf() {
    let info = FontInfo::from_path("/usr/share/fonts/NotoSans-Bold.ttf", true).unwrap();
    assert_eq!(info.family, "Noto Sans");
    assert_eq!(info.weight, FontWeight::Bold);
    assert_eq!(info.style, FontStyle::Regular);
    assert_eq!(info.format, FontFormat::TrueType);
    assert!(info.is_system);
}

#[test]
fn font_info_from_path_italic() {
    let info =
        FontInfo::from_path("/home/user/.fonts/Roboto-BoldItalic.otf", false).unwrap();
    assert_eq!(info.family, "Roboto");
    assert_eq!(info.weight, FontWeight::Bold);
    assert_eq!(info.style, FontStyle::Italic);
    assert_eq!(info.format, FontFormat::OpenType);
    assert!(!info.is_system);
}

#[test]
fn font_info_from_path_unknown_ext() {
    assert!(FontInfo::from_path("/some/font.xyz", true).is_none());
}

#[test]
fn font_info_monospace_detection() {
    let info = FontInfo::from_path("/usr/share/fonts/JetBrainsMono-Regular.ttf", true).unwrap();
    assert!(info.is_monospace);
}

#[test]
fn font_info_display_name() {
    let info = FontInfo::from_path("/fonts/Arial-Bold.ttf", true).unwrap();
    let name = info.display_name();
    assert!(name.contains("Arial"));
    assert!(name.contains("Bold"));
}

// ── FontManager ──────────────────────────────────────────────────────

fn make_test_manager() -> FontManager {
    let mut mgr = FontManager::new();
    mgr.add(FontInfo {
        family: "TestSans".into(),
        style: FontStyle::Regular,
        weight: FontWeight::Regular,
        stretch: FontStretch::Normal,
        file_path: "/fonts/TestSans-Regular.ttf".into(),
        format: FontFormat::TrueType,
        is_variable: false,
        is_monospace: false,
        is_system: true,
        coverage: vec![UnicodeBlock::BasicLatin, UnicodeBlock::Latin1Supplement],
    });
    mgr.add(FontInfo {
        family: "TestSans".into(),
        style: FontStyle::Regular,
        weight: FontWeight::Bold,
        stretch: FontStretch::Normal,
        file_path: "/fonts/TestSans-Bold.ttf".into(),
        format: FontFormat::TrueType,
        is_variable: false,
        is_monospace: false,
        is_system: true,
        coverage: vec![UnicodeBlock::BasicLatin, UnicodeBlock::Latin1Supplement],
    });
    mgr.add(FontInfo {
        family: "TestSans".into(),
        style: FontStyle::Italic,
        weight: FontWeight::Regular,
        stretch: FontStretch::Normal,
        file_path: "/fonts/TestSans-Italic.ttf".into(),
        format: FontFormat::TrueType,
        is_variable: false,
        is_monospace: false,
        is_system: true,
        coverage: vec![UnicodeBlock::BasicLatin],
    });
    mgr.add(FontInfo {
        family: "TestMono".into(),
        style: FontStyle::Regular,
        weight: FontWeight::Regular,
        stretch: FontStretch::Normal,
        file_path: "/fonts/TestMono-Regular.ttf".into(),
        format: FontFormat::TrueType,
        is_variable: false,
        is_monospace: true,
        is_system: false,
        coverage: vec![UnicodeBlock::BasicLatin, UnicodeBlock::Cyrillic],
    });
    mgr
}

#[test]
fn manager_families() {
    let mgr = make_test_manager();
    let fams = mgr.families();
    assert_eq!(fams.len(), 2);
    assert!(fams.contains(&"TestMono".to_string()));
    assert!(fams.contains(&"TestSans".to_string()));
}

#[test]
fn manager_fonts_in_family() {
    let mgr = make_test_manager();
    assert_eq!(mgr.fonts_in_family("TestSans").len(), 3);
    assert_eq!(mgr.fonts_in_family("TestMono").len(), 1);
    assert_eq!(mgr.fonts_in_family("Nonexistent").len(), 0);
}

#[test]
fn manager_fonts_in_family_case_insensitive() {
    let mgr = make_test_manager();
    assert_eq!(mgr.fonts_in_family("testsans").len(), 3);
    assert_eq!(mgr.fonts_in_family("TESTMONO").len(), 1);
}

#[test]
fn manager_find_font_exact() {
    let mgr = make_test_manager();
    let found = mgr
        .find_font("TestSans", FontWeight::Bold, FontStyle::Regular)
        .unwrap();
    assert_eq!(found.weight, FontWeight::Bold);
    assert_eq!(found.style, FontStyle::Regular);
}

#[test]
fn manager_find_font_italic() {
    let mgr = make_test_manager();
    let found = mgr
        .find_font("TestSans", FontWeight::Regular, FontStyle::Italic)
        .unwrap();
    assert_eq!(found.style, FontStyle::Italic);
}

#[test]
fn manager_find_font_closest_weight() {
    let mgr = make_test_manager();
    // Ask for Medium (500) in Regular style — should get Regular (400)
    // since it's closer than Bold (700).
    let found = mgr
        .find_font("TestSans", FontWeight::Medium, FontStyle::Regular)
        .unwrap();
    assert_eq!(found.weight, FontWeight::Regular);
}

#[test]
fn manager_find_font_none() {
    let mgr = make_test_manager();
    assert!(mgr
        .find_font("Nonexistent", FontWeight::Regular, FontStyle::Regular)
        .is_none());
}

#[test]
fn manager_monospace_fonts() {
    let mgr = make_test_manager();
    let mono = mgr.monospace_fonts();
    assert_eq!(mono.len(), 1);
    assert_eq!(mono[0].family, "TestMono");
}

#[test]
fn manager_font_for_text() {
    let mgr = make_test_manager();
    // ASCII text — all fonts that cover BasicLatin.
    let covering = mgr.font_for_text("Hello");
    assert_eq!(covering.len(), 4);
}

#[test]
fn manager_font_for_text_cyrillic() {
    let mgr = make_test_manager();
    // Cyrillic text — only TestMono has Cyrillic coverage.
    let covering = mgr.font_for_text("\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}");
    assert_eq!(covering.len(), 1);
    assert_eq!(covering[0].family, "TestMono");
}

#[test]
fn manager_len() {
    let mgr = make_test_manager();
    assert_eq!(mgr.len(), 4);
    assert!(!mgr.is_empty());
}

#[test]
fn manager_empty() {
    let mgr = FontManager::new();
    assert_eq!(mgr.len(), 0);
    assert!(mgr.is_empty());
}

// ── FontPreview ──────────────────────────────────────────────────────

#[test]
fn preview_default_text() {
    let text = FontPreview::default_preview_text();
    assert!(text.contains("quick brown fox"));
    assert!(text.contains("0123456789"));
}

#[test]
fn preview_pangram_english() {
    let p = FontPreview::pangram_for_language("en");
    assert!(p.contains("quick brown fox"));
}

#[test]
fn preview_pangram_german() {
    let p = FontPreview::pangram_for_language("de");
    assert!(p.contains("Victor"));
}

#[test]
fn preview_pangram_fallback() {
    // Unknown language falls back to English.
    let p = FontPreview::pangram_for_language("xx");
    assert!(p.contains("quick brown fox"));
}

#[test]
fn preview_supported_languages() {
    let langs = FontPreview::supported_languages();
    assert!(langs.len() >= 10);
    assert!(langs.contains(&"en"));
    assert!(langs.contains(&"ja"));
    assert!(langs.contains(&"ko"));
}

#[test]
fn preview_config_default() {
    let cfg = PreviewConfig::default();
    assert!(cfg.size_pt > 0.0);
    assert!(cfg.line_height > 0.0);
    assert!(!cfg.text.is_empty());
}

#[test]
fn preview_config_for_size() {
    let cfg = FontPreview::config_for_size(24.0);
    assert!((cfg.size_pt - 24.0).abs() < f32::EPSILON);
}

#[test]
fn preview_charset_sample() {
    let sample = FontPreview::charset_sample();
    assert!(sample.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
    assert!(sample.contains("0123456789"));
}

// ── FallbackChain ────────────────────────────────────────────────────

#[test]
fn fallback_chain_from_css() {
    let chain = FallbackChain::from_css("\"Inter\", 'Helvetica Neue', Arial, sans-serif");
    assert_eq!(chain.families.len(), 4);
    assert_eq!(chain.families[0], "Inter");
    assert_eq!(chain.families[1], "Helvetica Neue");
    assert_eq!(chain.families[2], "Arial");
    assert_eq!(chain.families[3], "sans-serif");
}

#[test]
fn fallback_chain_len() {
    let chain = FallbackChain::new(vec!["A".into(), "B".into()]);
    assert_eq!(chain.len(), 2);
    assert!(!chain.is_empty());
}

#[test]
fn fallback_defaults() {
    assert!(!FontFallback::default_sans().is_empty());
    assert!(!FontFallback::default_serif().is_empty());
    assert!(!FontFallback::default_mono().is_empty());
}

#[test]
fn fallback_resolve() {
    let mgr = make_test_manager();
    let chain = FallbackChain::new(vec![
        "Nonexistent".into(),
        "TestSans".into(),
        "TestMono".into(),
    ]);
    let resolved = FontFallback::resolve(&chain, mgr.all_fonts());
    // "Nonexistent" is skipped; TestSans and TestMono match.
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].family, "TestSans");
    assert_eq!(resolved[1].family, "TestMono");
}

#[test]
fn fallback_resolve_prefers_regular() {
    let mgr = make_test_manager();
    let chain = FallbackChain::new(vec!["TestSans".into()]);
    let resolved = FontFallback::resolve(&chain, mgr.all_fonts());
    assert_eq!(resolved.len(), 1);
    // Should pick Regular (400) over Bold (700).
    assert_eq!(resolved[0].weight, FontWeight::Regular);
}

#[test]
fn fallback_resolve_first() {
    let mgr = make_test_manager();
    let chain = FallbackChain::new(vec!["Missing".into(), "TestMono".into()]);
    let first = FontFallback::resolve_first(&chain, mgr.all_fonts());
    assert!(first.is_some());
    assert_eq!(first.unwrap().family, "TestMono");
}

#[test]
fn fallback_resolve_first_none() {
    let mgr = make_test_manager();
    let chain = FallbackChain::new(vec!["Missing".into()]);
    assert!(FontFallback::resolve_first(&chain, mgr.all_fonts()).is_none());
}

// ── Platform ─────────────────────────────────────────────────────────

#[test]
fn platform_system_dirs_nonempty() {
    let dirs = crate::platform::system_font_dirs();
    assert!(!dirs.is_empty(), "should return at least one font directory");
}

#[test]
fn platform_user_dir_some() {
    // This may return None in CI without a home dir, but on most dev
    // machines it should return Some.
    let _dir = crate::platform::user_font_dir();
    // No assertion — just make sure it doesn't panic.
}

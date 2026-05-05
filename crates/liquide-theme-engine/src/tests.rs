#[cfg(test)]
mod tests {
    use crate::builtin::*;
    use crate::color::Color;
    use crate::definition::*;
    use crate::manager::{ThemeError, ThemeManager};
    use crate::palette::ColorPalette;
    use crate::parser::{ParseError, parse_theme, parse_theme_source};
    use crate::transition::ThemeTransition;

    // ═══════════════════════════════════════════════════════
    //  Color
    // ═══════════════════════════════════════════════════════

    #[test]
    fn color_from_hex_6() {
        let c = Color::from_hex("#0a84ff").unwrap();
        assert_eq!(c, Color::rgb(10, 132, 255));
    }

    #[test]
    fn color_from_hex_8() {
        let c = Color::from_hex("#ff4500cc").unwrap();
        assert_eq!(c, Color::rgba(255, 69, 0, 204));
    }

    #[test]
    fn color_from_hex_3() {
        let c = Color::from_hex("#abc").unwrap();
        assert_eq!(c, Color::rgb(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn color_from_hex_4() {
        let c = Color::from_hex("#abcf").unwrap();
        assert_eq!(c, Color::rgba(0xaa, 0xbb, 0xcc, 0xff));
    }

    #[test]
    fn color_from_hex_no_hash() {
        let c = Color::from_hex("ff0000").unwrap();
        assert_eq!(c, Color::rgb(255, 0, 0));
    }

    #[test]
    fn color_from_hex_invalid() {
        assert!(Color::from_hex("xyz").is_none());
        assert!(Color::from_hex("#12345").is_none());
    }

    #[test]
    fn color_to_hex_opaque() {
        assert_eq!(Color::rgb(255, 0, 128).to_hex(), "#ff0080");
    }

    #[test]
    fn color_to_hex_transparent() {
        assert_eq!(Color::rgba(255, 0, 128, 200).to_hex(), "#ff0080c8");
    }

    #[test]
    fn color_lerp_extremes() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 50);
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
    }

    #[test]
    fn color_lerp_midpoint() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 50);
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.r, 50);
        assert_eq!(mid.g, 100);
        assert_eq!(mid.b, 25);
    }

    #[test]
    fn color_with_alpha() {
        let c = Color::rgb(10, 20, 30).with_alpha(128);
        assert_eq!(c, Color::rgba(10, 20, 30, 128));
    }

    #[test]
    fn color_lighten() {
        let c = Color::rgb(100, 100, 100);
        let lighter = c.lighten(0.5);
        assert!(lighter.r > 100);
        assert_eq!(lighter.a, 255);
    }

    #[test]
    fn color_darken() {
        let c = Color::rgb(100, 100, 100);
        let darker = c.darken(0.5);
        assert_eq!(darker.r, 50);
        assert_eq!(darker.a, 255);
    }

    #[test]
    fn color_lighten_clamp() {
        let c = Color::rgb(200, 200, 200);
        let lighter = c.lighten(2.0); // clamped to 1.0
        assert_eq!(lighter, Color::rgb(255, 255, 255));
    }

    #[test]
    fn color_darken_clamp() {
        let c = Color::rgb(200, 200, 200);
        let darker = c.darken(-1.0); // clamped to 0.0
        assert_eq!(darker, c);
    }

    #[test]
    fn color_css_rgba_roundtrip() {
        let c = Color::rgba(10, 132, 255, 128);
        let css = c.to_css_rgba();
        assert!(css.starts_with("rgba("));
        let parsed = Color::from_css_rgba(&css).unwrap();
        // Alpha might lose a tiny bit of precision in the float roundtrip.
        assert_eq!(parsed.r, 10);
        assert_eq!(parsed.g, 132);
        assert_eq!(parsed.b, 255);
        assert!((parsed.a as i16 - 128).unsigned_abs() <= 1);
    }

    #[test]
    fn color_css_rgb() {
        let c = Color::rgb(255, 128, 0);
        assert_eq!(c.to_css_rgba(), "rgb(255, 128, 0)");
        let parsed = Color::from_css_rgba("rgb(255, 128, 0)").unwrap();
        assert_eq!(parsed, c);
    }

    #[test]
    fn color_luminance() {
        assert!(Color::WHITE.luminance() > 0.9);
        assert!(Color::BLACK.luminance() < 0.01);
    }

    #[test]
    fn color_is_dark() {
        assert!(Color::BLACK.is_dark());
        assert!(!Color::WHITE.is_dark());
    }

    // ═══════════════════════════════════════════════════════
    //  Palette
    // ═══════════════════════════════════════════════════════

    #[test]
    fn palette_lerp_identity() {
        let p = ColorPalette::default();
        let result = p.lerp(&p, 0.5);
        assert_eq!(result, p);
    }

    // ═══════════════════════════════════════════════════════
    //  ThemeVariant
    // ═══════════════════════════════════════════════════════

    #[test]
    fn theme_variant_parse() {
        assert_eq!(
            ThemeVariant::from_str_loose("Light"),
            Some(ThemeVariant::Light)
        );
        assert_eq!(
            ThemeVariant::from_str_loose("DARK"),
            Some(ThemeVariant::Dark)
        );
        assert_eq!(
            ThemeVariant::from_str_loose("high-contrast"),
            Some(ThemeVariant::HighContrast)
        );
        assert_eq!(
            ThemeVariant::from_str_loose("auto"),
            Some(ThemeVariant::Auto)
        );
        assert_eq!(ThemeVariant::from_str_loose("unknown"), None);
    }

    // ═══════════════════════════════════════════════════════
    //  Built-in themes
    // ═══════════════════════════════════════════════════════

    #[test]
    fn builtin_night_valid() {
        let t = builtin_night();
        assert_eq!(t.metadata.id, "night");
        assert_eq!(t.metadata.variant, ThemeVariant::Dark);
        assert!(t.metadata.supports_glass);
        assert_eq!(t.palette.background, Color::rgb(0, 0, 0));
        assert_eq!(t.dock.height, 56.0);
        assert_eq!(t.statusbar.height, 34.0);
    }

    #[test]
    fn builtin_midday_valid() {
        let t = builtin_midday();
        assert_eq!(t.metadata.id, "midday");
        assert_eq!(t.metadata.variant, ThemeVariant::Light);
        assert_eq!(t.palette.background, Color::rgb(245, 240, 232));
    }

    #[test]
    fn builtin_sunset_valid() {
        let t = builtin_sunset();
        assert_eq!(t.metadata.id, "sunset");
        assert_eq!(t.palette.primary, Color::rgb(255, 159, 10));
    }

    #[test]
    fn builtin_liquid_glass_valid() {
        let t = builtin_liquid_glass();
        assert_eq!(t.metadata.id, "liquid-glass");
        assert!(t.glass.blur_radius > 20.0);
    }

    // ═══════════════════════════════════════════════════════
    //  ThemeManager
    // ═══════════════════════════════════════════════════════

    #[test]
    fn manager_register_and_list() {
        let mut mgr = ThemeManager::new();
        mgr.register_theme(builtin_night());
        mgr.register_theme(builtin_midday());
        let themes = mgr.available_themes();
        assert_eq!(themes.len(), 2);
        assert!(themes.iter().any(|m| m.id == "night"));
        assert!(themes.iter().any(|m| m.id == "midday"));
    }

    #[test]
    fn manager_set_active() {
        let mut mgr = ThemeManager::new();
        mgr.register_theme(builtin_night());
        assert!(mgr.set_active("night").is_ok());
        assert_eq!(mgr.active_theme().unwrap().metadata.id, "night");
    }

    #[test]
    fn manager_set_active_not_found() {
        let mut mgr = ThemeManager::new();
        assert_eq!(
            mgr.set_active("missing"),
            Err(ThemeError::NotFound("missing".into()))
        );
    }

    #[test]
    fn manager_no_active_theme() {
        let mgr = ThemeManager::new();
        assert_eq!(mgr.active_theme().unwrap_err(), ThemeError::NoActiveTheme);
    }

    #[test]
    fn manager_replace_theme() {
        let mut mgr = ThemeManager::new();
        mgr.register_theme(builtin_night());
        let mut modified = builtin_night();
        modified.dock.height = 80.0;
        mgr.register_theme(modified);
        assert_eq!(mgr.available_themes().len(), 1);
        let t = mgr.get_theme("night").unwrap();
        assert_eq!(t.dock.height, 80.0);
    }

    #[test]
    fn manager_with_builtins() {
        let mgr = ThemeManager::with_builtins();
        assert_eq!(mgr.available_themes().len(), 4);
        assert_eq!(mgr.active_theme().unwrap().metadata.id, "night");
    }

    #[test]
    fn manager_resolve_inheritance() {
        let mut mgr = ThemeManager::new();
        mgr.register_theme(builtin_night());

        let parsed = parse_theme_source(
            "[metadata]\nid = \"night-custom\"\nparent = \"night\"\n[dock]\nheight = 70\n",
        )
        .unwrap();
        let child = parsed.definition().clone();
        mgr.register_parsed_theme(parsed);

        let resolved = mgr.resolve_inheritance(&child).unwrap();
        assert!(resolved.metadata.parent.is_none());
        assert_eq!(resolved.dock.height, 70.0);
        assert_eq!(resolved.palette.primary, builtin_night().palette.primary);
    }

    #[test]
    fn manager_resolve_missing_parent() {
        let mgr = ThemeManager::new();
        let mut child = ThemeDefinition::default();
        child.metadata.id = "orphan".into();
        child.metadata.parent = Some("nonexistent".into());
        assert!(mgr.resolve_inheritance(&child).is_err());
    }

    #[test]
    fn manager_generate_css() {
        let theme = builtin_night();
        let css = ThemeManager::generate_css(&theme);
        assert!(css.starts_with(":root {"));
        assert!(css.contains("--color-primary:"));
        assert!(css.contains("--window-titlebar-height:"));
        assert!(css.contains("--dock-height:"));
        assert!(css.contains("--glass-blur-radius:"));
        assert!(css.ends_with("}\n"));
    }

    #[test]
    fn manager_register_theme_treats_materialized_defaults_as_explicit() {
        let mut mgr = ThemeManager::new();
        let mut parent = ThemeDefinition::default();
        parent.metadata.id = "parent".into();
        parent.metadata.name = "Parent".into();
        parent.window.titlebar_height = 99.0;
        mgr.register_theme(parent);

        let mut child = ThemeDefinition::default();
        child.metadata.id = "child".into();
        child.metadata.name = "Child".into();
        child.metadata.parent = Some("parent".into());
        mgr.register_theme(child.clone());

        let resolved = mgr.resolve_inheritance(&child).unwrap();
        assert!(
            (resolved.window.titlebar_height - WindowTheme::default().titlebar_height).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn manager_parsed_theme_preserves_omitted_parent_metadata() {
        let mut mgr = ThemeManager::new();
        let mut parent = ThemeDefinition::default();
        parent.metadata.id = "parent".into();
        parent.metadata.name = "Parent".into();
        parent.metadata.author = "Parent Author".into();
        parent.metadata.version = "9.9.9".into();
        parent.metadata.variant = ThemeVariant::Light;
        parent.metadata.supports_glass = false;
        parent.window.titlebar_height = 99.0;
        mgr.register_theme(parent);

        let parsed =
            parse_theme_source("[metadata]\nid = \"child\"\nparent = \"parent\"\n").unwrap();
        let child = parsed.definition().clone();
        mgr.register_parsed_theme(parsed);

        let resolved = mgr.resolve_inheritance(&child).unwrap();
        assert_eq!(resolved.metadata.name, "child");
        assert_eq!(resolved.metadata.author, "Parent Author");
        assert_eq!(resolved.metadata.version, "9.9.9");
        assert_eq!(resolved.metadata.variant, ThemeVariant::Light);
        assert!(!resolved.metadata.supports_glass);
        assert!((resolved.window.titlebar_height - 99.0).abs() < f32::EPSILON);
    }

    #[test]
    fn manager_parsed_theme_can_reset_parent_values_to_defaults() {
        let mut mgr = ThemeManager::new();
        let mut parent = builtin_night();
        parent.metadata.id = "parent".into();
        parent.metadata.name = "Parent".into();
        parent.metadata.supports_glass = false;
        parent.dock.spacing = 12.0;
        mgr.register_theme(parent);

        let parsed = parse_theme_source(
            "[metadata]\nid = \"child\"\nparent = \"parent\"\nsupports_glass = true\n[dock]\nspacing = 4\n",
        )
        .unwrap();
        let child = parsed.definition().clone();
        mgr.register_parsed_theme(parsed);

        let resolved = mgr.resolve_inheritance(&child).unwrap();
        assert_eq!(resolved.dock.spacing, DockTheme::default().spacing);
        assert!(resolved.metadata.supports_glass);
    }

    #[test]
    fn manager_on_theme_change_fires_on_set_active() {
        use std::sync::{Arc, Mutex};
        let mut mgr = ThemeManager::with_builtins();
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        mgr.on_theme_change(move |theme, css| {
            assert!(css.starts_with(":root {"));
            events_clone.lock().unwrap().push(theme.metadata.id.clone());
        });
        mgr.set_active("midday").unwrap();
        mgr.set_active("sunset").unwrap();
        let fired = events.lock().unwrap();
        assert_eq!(&fired[..], &["midday".to_string(), "sunset".to_string()]);
    }

    #[test]
    fn manager_set_active_uses_resolved_theme() {
        use std::sync::{Arc, Mutex};

        let mut mgr = ThemeManager::new();
        let mut parent = builtin_night();
        parent.metadata.id = "parent".into();
        parent.metadata.name = "Parent".into();
        parent.metadata.author = "Parent Author".into();
        let parent_primary = parent.palette.primary;
        mgr.register_theme(parent);

        let parsed = parse_theme_source(
            "[metadata]\nid = \"child\"\nparent = \"parent\"\n[dock]\nheight = 70\n",
        )
        .unwrap();
        mgr.register_parsed_theme(parsed);

        let events: Arc<Mutex<Vec<(String, f32, Color)>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        mgr.on_theme_change(move |theme, _css| {
            events_clone.lock().unwrap().push((
                theme.metadata.author.clone(),
                theme.dock.height,
                theme.palette.primary,
            ));
        });

        mgr.set_active("child").unwrap();

        let fired = events.lock().unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0, "Parent Author");
        assert_eq!(fired[0].1, 70.0);
        assert_eq!(fired[0].2, parent_primary);
    }

    #[test]
    fn manager_generate_active_css_emits_runtime_tokens_for_resolved_theme() {
        let mut mgr = ThemeManager::new();
        let mut parent = builtin_night();
        parent.metadata.id = "parent".into();
        parent.metadata.name = "Parent".into();
        parent.window.border_width = 5.0;
        parent.statusbar.padding_horizontal = 18.0;
        parent.dock.spacing = 9.0;
        parent.menu.disabled_color = Color::rgb(1, 2, 3);
        parent.tooltip.padding_horizontal = 11.0;
        parent.notification.action_bg = Color::rgb(4, 5, 6);
        mgr.register_theme(parent);

        let parsed = parse_theme_source(
            "[metadata]\nid = \"child\"\nparent = \"parent\"\n[window]\nclose_button_bg = #010203\n[dock]\nitem_hover_bg = #112233\n",
        )
        .unwrap();
        mgr.register_parsed_theme(parsed);
        mgr.set_active("child").unwrap();

        let css = mgr.generate_active_css().unwrap();
        assert!(css.contains("--window-border-width: 5px"));
        assert!(css.contains("--window-close-button-bg: rgb(1, 2, 3)"));
        assert!(css.contains("--statusbar-padding-horizontal: 18px"));
        assert!(css.contains("--dock-spacing: 9px"));
        assert!(css.contains("--dock-item-hover-bg: rgb(17, 34, 51)"));
        assert!(css.contains("--menu-disabled-color: rgb(1, 2, 3)"));
        assert!(css.contains("--tooltip-padding-horizontal: 11px"));
        assert!(css.contains("--notification-action-bg: rgb(4, 5, 6)"));
    }

    #[test]
    fn manager_reregister_active_theme_refreshes_callbacks() {
        use std::sync::{Arc, Mutex};

        let mut mgr = ThemeManager::new();
        mgr.register_theme(builtin_night());

        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        mgr.on_theme_change(move |_theme, css| {
            events_clone.lock().unwrap().push(css.to_string());
        });

        mgr.set_active("night").unwrap();

        let mut updated = builtin_night();
        updated.dock.height = 80.0;
        mgr.register_theme(updated);

        let fired = events.lock().unwrap();
        assert_eq!(fired.len(), 2);
        assert!(fired[1].contains("--dock-height: 80px"));
    }

    #[test]
    fn manager_resolves_auto_variant_against_system_theme_preference() {
        let mut mgr = ThemeManager::new();
        let mut theme = builtin_night();
        theme.metadata.id = "auto-night".into();
        theme.metadata.name = "Auto Night".into();
        theme.metadata.variant = ThemeVariant::Auto;
        mgr.register_theme(theme);

        mgr.set_system_theme_variant(ThemeVariant::Light);
        mgr.set_active("auto-night").unwrap();
        assert_eq!(
            mgr.resolved_active_theme().unwrap().metadata.variant,
            ThemeVariant::Light
        );

        mgr.set_system_theme_variant(ThemeVariant::HighContrast);
        assert_eq!(
            mgr.resolved_active_theme().unwrap().metadata.variant,
            ThemeVariant::HighContrast
        );
    }

    // ═══════════════════════════════════════════════════════
    //  ThemeTransition
    // ═══════════════════════════════════════════════════════

    #[test]
    fn transition_progress() {
        let from = builtin_night().palette;
        let to = builtin_midday().palette;
        let mut trans = ThemeTransition::new(from.clone(), to.clone(), 500);
        assert!(!trans.is_complete());
        assert_eq!(trans.progress, 0.0);

        trans.tick(250);
        assert!(!trans.is_complete());
        assert!((trans.progress - 0.5).abs() < 0.01);

        trans.tick(250);
        assert!(trans.is_complete());
    }

    #[test]
    fn transition_interpolate_endpoints() {
        let from = builtin_night().palette;
        let to = builtin_midday().palette;
        let trans = ThemeTransition::new(from.clone(), to.clone(), 500);

        let at_start = trans.interpolate_at(0.0);
        assert_eq!(at_start.background, from.background);

        let at_end = trans.interpolate_at(1.0);
        assert_eq!(at_end.background, to.background);
    }

    #[test]
    fn transition_reset() {
        let from = builtin_night().palette;
        let to = builtin_midday().palette;
        let mut trans = ThemeTransition::new(from, to, 200);
        trans.tick(200);
        assert!(trans.is_complete());
        trans.reset();
        assert_eq!(trans.progress, 0.0);
        assert!(!trans.is_complete());
    }

    #[test]
    fn transition_retarget() {
        let night = builtin_night().palette;
        let midday = builtin_midday().palette;
        let sunset = builtin_sunset().palette;

        let mut trans = ThemeTransition::new(night, midday, 200);
        trans.tick(100); // halfway
        let snapshot = trans.interpolate();
        trans.retarget(sunset.clone());
        assert_eq!(trans.progress, 0.0);
        // The new "from" should be close to the snapshot.
        assert_eq!(trans.from.background, snapshot.background);
        // "to" should be sunset.
        assert_eq!(trans.to.background, sunset.background);
    }

    // ═══════════════════════════════════════════════════════
    //  Parser
    // ═══════════════════════════════════════════════════════

    const SAMPLE_THEME: &str = r#"
# A custom theme
[metadata]
id = "my-dark"
name = "My Dark Theme"
author = "Test Author"
version = "2.0.0"
description = "A custom dark theme"
variant = "dark"
supports_glass = true

[palette]
primary = #ff5500
background = #111111
surface = #222222
text_primary = #ffffff

[window]
titlebar_height = 40
border_radius = 12

[statusbar]
height = 30

[dock]
height = 60
item_size = 48

[menu]
item_height = 32

[tooltip]
delay_ms = 300
max_width = 250

[notification]
width = 350

[glass]
blur_radius = 20
opacity = 0.75
"#;

    #[test]
    fn parse_sample_theme() {
        let theme = parse_theme(SAMPLE_THEME).unwrap();
        assert_eq!(theme.metadata.id, "my-dark");
        assert_eq!(theme.metadata.name, "My Dark Theme");
        assert_eq!(theme.metadata.author, "Test Author");
        assert_eq!(theme.metadata.version, "2.0.0");
        assert_eq!(theme.metadata.variant, ThemeVariant::Dark);
        assert!(theme.metadata.supports_glass);
    }

    #[test]
    fn parse_palette_colors() {
        let theme = parse_theme(SAMPLE_THEME).unwrap();
        assert_eq!(theme.palette.primary, Color::rgb(255, 85, 0));
        assert_eq!(theme.palette.background, Color::rgb(17, 17, 17));
    }

    #[test]
    fn parse_component_values() {
        let theme = parse_theme(SAMPLE_THEME).unwrap();
        assert_eq!(theme.window.titlebar_height, 40.0);
        assert_eq!(theme.window.border_radius, 12.0);
        assert_eq!(theme.statusbar.height, 30.0);
        assert_eq!(theme.dock.height, 60.0);
        assert_eq!(theme.dock.item_size, 48.0);
        assert_eq!(theme.menu.item_height, 32.0);
        assert_eq!(theme.tooltip.delay_ms, 300);
        assert_eq!(theme.tooltip.max_width, 250.0);
        assert_eq!(theme.notification.width, 350.0);
        assert_eq!(theme.glass.blur_radius, 20.0);
        assert!((theme.glass.opacity - 0.75).abs() < 0.001);
    }

    #[test]
    fn parse_missing_id() {
        let input = "[metadata]\nname = \"Test\"\n";
        assert_eq!(
            parse_theme(input).unwrap_err(),
            ParseError::MissingField("metadata.id".into())
        );
    }

    #[test]
    fn parse_unknown_section() {
        let input = "[metadata]\nid = \"x\"\n[bogus]\nfoo = bar\n";
        assert!(matches!(
            parse_theme(input).unwrap_err(),
            ParseError::UnknownSection(_, _)
        ));
    }

    #[test]
    fn parse_bad_color() {
        let input = "[metadata]\nid = \"x\"\n[palette]\nprimary = not_a_color\n";
        assert!(matches!(
            parse_theme(input).unwrap_err(),
            ParseError::BadColor(_, _)
        ));
    }

    #[test]
    fn parse_bad_number() {
        let input = "[metadata]\nid = \"x\"\n[dock]\nheight = abc\n";
        assert!(matches!(
            parse_theme(input).unwrap_err(),
            ParseError::BadNumber(_, _)
        ));
    }

    #[test]
    fn parse_comments_and_blanks() {
        let input = "# comment\n\n[metadata]\n// also a comment\nid = \"test\"\n";
        let theme = parse_theme(input).unwrap();
        assert_eq!(theme.metadata.id, "test");
    }

    #[test]
    fn parse_parent_theme() {
        let input = "[metadata]\nid = \"child\"\nparent = \"night\"\n";
        let theme = parse_theme(input).unwrap();
        assert_eq!(theme.metadata.parent, Some("night".to_string()));
    }

    #[test]
    fn parse_rgba_color_value() {
        let input = "[metadata]\nid = \"x\"\n[palette]\nprimary = rgba(10, 132, 255, 1.0)\n";
        let theme = parse_theme(input).unwrap();
        assert_eq!(theme.palette.primary, Color::rgba(10, 132, 255, 255));
    }

    #[test]
    fn parse_name_defaults_to_id() {
        let input = "[metadata]\nid = \"auto-name\"\n";
        let theme = parse_theme(input).unwrap();
        assert_eq!(theme.metadata.name, "auto-name");
    }

    #[test]
    fn parse_and_use_full_roundtrip() {
        // Parse a theme file, register it, set active, generate CSS.
        let mut mgr = ThemeManager::with_builtins();
        let custom = parse_theme_source(SAMPLE_THEME).unwrap();
        mgr.register_parsed_theme(custom);
        mgr.set_active("my-dark").unwrap();
        let css = ThemeManager::generate_css(mgr.active_theme().unwrap());
        assert!(css.contains("--dock-height: 60px"));
        assert!(css.contains("--color-primary:"));
    }
}

//! Tests for accessibility preference detection, contrast utilities,
//! high-contrast theme generation, reduced-motion overrides, and
//! preference change detection.

#[cfg(test)]
mod contrast_tests {
    use crate::contrast::*;

    #[test]
    fn luminance_black() {
        let l = luminance(0, 0, 0);
        assert!((l - 0.0).abs() < 1e-10, "black luminance should be 0, got {l}");
    }

    #[test]
    fn luminance_white() {
        let l = luminance(255, 255, 255);
        assert!(
            (l - 1.0).abs() < 1e-6,
            "white luminance should be 1.0, got {l}"
        );
    }

    #[test]
    fn luminance_red() {
        let l = luminance(255, 0, 0);
        assert!(
            (l - 0.2126).abs() < 0.001,
            "pure red luminance should be ~0.2126, got {l}"
        );
    }

    #[test]
    fn luminance_green() {
        let l = luminance(0, 255, 0);
        assert!(
            (l - 0.7152).abs() < 0.001,
            "pure green luminance should be ~0.7152, got {l}"
        );
    }

    #[test]
    fn luminance_blue() {
        let l = luminance(0, 0, 255);
        assert!(
            (l - 0.0722).abs() < 0.001,
            "pure blue luminance should be ~0.0722, got {l}"
        );
    }

    #[test]
    fn luminance_mid_grey() {
        // sRGB 128 -> linear ~0.216
        let l = luminance(128, 128, 128);
        assert!(l > 0.2 && l < 0.25, "mid grey luminance got {l}");
    }

    #[test]
    fn contrast_black_on_white() {
        let r = contrast_ratio((0, 0, 0), (255, 255, 255));
        assert!(
            (r - 21.0).abs() < 0.1,
            "black on white should be ~21:1, got {r}"
        );
    }

    #[test]
    fn contrast_white_on_black() {
        // Order shouldn't matter.
        let r = contrast_ratio((255, 255, 255), (0, 0, 0));
        assert!(
            (r - 21.0).abs() < 0.1,
            "white on black should be ~21:1, got {r}"
        );
    }

    #[test]
    fn contrast_same_color() {
        let r = contrast_ratio((100, 100, 100), (100, 100, 100));
        assert!(
            (r - 1.0).abs() < 0.01,
            "same color should be 1:1, got {r}"
        );
    }

    #[test]
    fn contrast_grey_on_white() {
        // #767676 on white is the boundary for AA (4.54:1).
        let r = contrast_ratio((118, 118, 118), (255, 255, 255));
        assert!(r >= 4.5, "#767676 on white should meet AA, got {r}");
    }

    #[test]
    fn meets_aa_passes() {
        assert!(meets_aa(4.5));
        assert!(meets_aa(7.0));
        assert!(meets_aa(21.0));
    }

    #[test]
    fn meets_aa_fails() {
        assert!(!meets_aa(4.49));
        assert!(!meets_aa(1.0));
    }

    #[test]
    fn meets_aaa_passes() {
        assert!(meets_aaa(7.0));
        assert!(meets_aaa(21.0));
    }

    #[test]
    fn meets_aaa_fails() {
        assert!(!meets_aaa(6.99));
        assert!(!meets_aaa(4.5));
    }

    #[test]
    fn meets_aa_large_passes() {
        assert!(meets_aa_large(3.0));
        assert!(meets_aa_large(4.5));
    }

    #[test]
    fn meets_aa_large_fails() {
        assert!(!meets_aa_large(2.99));
    }

    #[test]
    fn suggest_color_already_good() {
        // Black on white already has 21:1 — should return black unchanged.
        let result = suggest_color((0, 0, 0), (255, 255, 255), 7.0);
        assert_eq!(result, (0, 0, 0));
    }

    #[test]
    fn suggest_color_darken_for_contrast() {
        // Light grey on white — needs to get darker.
        let fg = (200, 200, 200);
        let bg = (255, 255, 255);
        let result = suggest_color(fg, bg, 4.5);
        let ratio = contrast_ratio(result, bg);
        assert!(
            ratio >= 4.5,
            "suggested color should meet 4.5:1, got {ratio} for {result:?}"
        );
    }

    #[test]
    fn suggest_color_lighten_for_contrast() {
        // Dark grey on black — needs to get lighter.
        let fg = (50, 50, 50);
        let bg = (0, 0, 0);
        let result = suggest_color(fg, bg, 4.5);
        let ratio = contrast_ratio(result, bg);
        assert!(
            ratio >= 4.5,
            "suggested color should meet 4.5:1, got {ratio} for {result:?}"
        );
    }

    #[test]
    fn suggest_color_preserves_hue_roughly() {
        // Red on white — darkened red should still be reddish.
        let fg = (255, 100, 100);
        let bg = (255, 255, 255);
        let result = suggest_color(fg, bg, 4.5);
        // Red channel should be higher than green and blue.
        assert!(
            result.0 >= result.1 && result.0 >= result.2,
            "should preserve red dominance: {result:?}"
        );
    }

    #[test]
    fn suggest_color_extreme_target() {
        // Ask for 21:1 on a mid-grey background — should return black or white.
        let bg = (128, 128, 128);
        let fg = (130, 130, 130);
        let result = suggest_color(fg, bg, 21.0);
        // Either black or white (whichever direction was chosen).
        let ratio = contrast_ratio(result, bg);
        // It won't reach 21:1, but it should be the best possible.
        assert!(
            result == (0, 0, 0) || result == (255, 255, 255),
            "extreme target should snap to black or white, got {result:?} (ratio {ratio})"
        );
    }

    #[test]
    fn contrast_ratio_symmetry() {
        let a = (100, 150, 200);
        let b = (50, 75, 100);
        assert!(
            (contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-10,
            "contrast_ratio should be symmetric"
        );
    }

    #[test]
    fn luminance_linearization_threshold() {
        // Channel value 10 is below the 0.04045 threshold.
        let l = luminance(10, 10, 10);
        // 10/255 = 0.0392 < 0.04045 — should use linear branch.
        let expected = 0.0392 / 12.92;
        assert!(
            (l - expected).abs() < 0.002,
            "low value should use linear branch, got {l} expected ~{expected}"
        );
    }
}

#[cfg(test)]
mod high_contrast_tests {
    use crate::contrast::*;
    use crate::high_contrast::*;

    #[test]
    fn light_theme_meets_aaa() {
        let t = high_contrast_light();
        let ratio = t.fg_bg_contrast();
        assert!(
            meets_aaa(ratio),
            "light high-contrast fg/bg should meet AAA, got {ratio}"
        );
    }

    #[test]
    fn dark_theme_meets_aaa() {
        let t = high_contrast_dark();
        let ratio = t.fg_bg_contrast();
        assert!(
            meets_aaa(ratio),
            "dark high-contrast fg/bg should meet AAA, got {ratio}"
        );
    }

    #[test]
    fn light_theme_accent_meets_aa() {
        let t = high_contrast_light();
        let ratio = t.accent_bg_contrast();
        assert!(
            meets_aa(ratio),
            "light accent on bg should meet AA, got {ratio}"
        );
    }

    #[test]
    fn dark_theme_accent_meets_aa() {
        let t = high_contrast_dark();
        let ratio = t.accent_bg_contrast();
        assert!(
            meets_aa(ratio),
            "dark accent on bg should meet AA, got {ratio}"
        );
    }

    #[test]
    fn increase_contrast_boosts_low_ratio() {
        let base = ThemeOverrides {
            bg_color: (255, 255, 255),
            fg_color: (150, 150, 150),     // ~2.8:1 — fails AA
            accent_color: (100, 100, 255), // low contrast
            border_color: (200, 200, 200), // very low
            link_color: (100, 100, 255),
            disabled_color: (180, 180, 180),
            selection_bg: (0, 0, 200),
            selection_fg: (255, 255, 255),
        };
        let boosted = increase_contrast(&base, 7.0);
        let ratio = contrast_ratio(boosted.fg_color, boosted.bg_color);
        assert!(
            ratio >= 7.0,
            "boosted fg should meet 7:1, got {ratio}"
        );
    }

    #[test]
    fn increase_contrast_preserves_good_ratio() {
        let base = high_contrast_light();
        let boosted = increase_contrast(&base, 7.0);
        // fg was already 21:1 — should be unchanged.
        assert_eq!(
            base.fg_color, boosted.fg_color,
            "fg already good should be unchanged"
        );
    }

    #[test]
    fn selection_contrast_meets_target() {
        let t = high_contrast_dark();
        let sel_ratio = contrast_ratio(t.selection_fg, t.selection_bg);
        assert!(
            sel_ratio >= 7.0,
            "selection fg/bg should meet AAA, got {sel_ratio}"
        );
    }
}

#[cfg(test)]
mod reduced_motion_tests {
    use crate::reduced_motion::*;

    #[test]
    fn default_has_no_restrictions() {
        let o = AnimationOverrides::default();
        assert!(!o.has_restrictions());
    }

    #[test]
    fn reduced_disables_transitions() {
        let o = reduced_motion_overrides();
        assert!(o.disable_transitions);
        assert!(o.disable_window_animations);
        assert!(o.disable_parallax);
        assert!(o.disable_blur_animation);
    }

    #[test]
    fn essential_only_caps_duration() {
        let o = essential_motion_only();
        assert!(o.max_duration_ms > 0 && o.max_duration_ms <= 200);
    }

    #[test]
    fn clamp_duration_disabled() {
        let o = reduced_motion_overrides();
        assert_eq!(o.clamp_duration(500), 0, "disabled transitions -> 0ms");
    }

    #[test]
    fn clamp_duration_capped() {
        let mut o = AnimationOverrides::default();
        o.max_duration_ms = 200;
        assert_eq!(o.clamp_duration(500), 200);
        assert_eq!(o.clamp_duration(100), 100);
    }

    #[test]
    fn clamp_duration_no_cap() {
        let o = AnimationOverrides::default();
        assert_eq!(o.clamp_duration(1000), 1000);
    }

    #[test]
    fn should_skip_window_animation() {
        let o = reduced_motion_overrides();
        assert!(o.should_skip_window_animation());

        let d = AnimationOverrides::default();
        assert!(!d.should_skip_window_animation());
    }
}

#[cfg(test)]
mod prefs_tests {
    use crate::prefs::*;

    #[test]
    fn default_prefs_no_overrides() {
        let p = AccessibilityPreferences::default();
        assert!(!p.has_visual_overrides());
        assert!(!p.has_motion_overrides());
        assert!(!p.has_keyboard_overrides());
    }

    #[test]
    fn visual_overrides_detected() {
        let mut p = AccessibilityPreferences::default();
        assert!(!p.has_visual_overrides());
        p.high_contrast = true;
        assert!(p.has_visual_overrides());
    }

    #[test]
    fn motion_overrides_detected() {
        let mut p = AccessibilityPreferences::default();
        p.reduced_motion = true;
        assert!(p.has_motion_overrides());
    }

    #[test]
    fn keyboard_overrides_detected() {
        let mut p = AccessibilityPreferences::default();
        p.sticky_keys = true;
        assert!(p.has_keyboard_overrides());
    }

    #[test]
    fn text_scale_override() {
        let mut p = AccessibilityPreferences::default();
        p.text_scale_factor = 1.5;
        assert!(p.has_visual_overrides());
    }

    #[test]
    fn cursor_size_from_pixels() {
        assert_eq!(CursorSize::from_pixels(16), CursorSize::Small);
        assert_eq!(CursorSize::from_pixels(32), CursorSize::Normal);
        assert_eq!(CursorSize::from_pixels(48), CursorSize::Large);
        assert_eq!(CursorSize::from_pixels(64), CursorSize::ExtraLarge);
        assert_eq!(CursorSize::from_pixels(96), CursorSize::ExtraLarge);
    }

    #[test]
    fn cursor_size_to_pixels() {
        assert_eq!(CursorSize::Small.to_pixels(), 16);
        assert_eq!(CursorSize::Normal.to_pixels(), 32);
        assert_eq!(CursorSize::Large.to_pixels(), 48);
        assert_eq!(CursorSize::ExtraLarge.to_pixels(), 64);
    }

    #[test]
    fn cursor_size_default() {
        assert_eq!(CursorSize::default(), CursorSize::Normal);
    }
}

#[cfg(test)]
mod watcher_tests {
    use crate::prefs::*;
    use crate::watcher::*;

    #[test]
    fn no_changes_when_equal() {
        let a = AccessibilityPreferences::default();
        let b = a.clone();
        let changes = check_for_changes(&a, &b);
        assert!(changes.is_empty());
    }

    #[test]
    fn detects_high_contrast_change() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.high_contrast = true;
        let changes = check_for_changes(&a, &b);
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0], PreferenceChange::HighContrast(true)));
    }

    #[test]
    fn detects_multiple_changes() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.reduced_motion = true;
        b.sticky_keys = true;
        b.text_scale_factor = 2.0;
        let changes = check_for_changes(&a, &b);
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn has_changes_utility() {
        let a = AccessibilityPreferences::default();
        let b = a.clone();
        assert!(!has_changes(&a, &b));

        let mut c = a.clone();
        c.bounce_keys = true;
        assert!(has_changes(&a, &c));
    }

    #[test]
    fn has_visual_changes_utility() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.sticky_keys = true; // not visual
        assert!(!has_visual_changes(&a, &b));

        b.high_contrast = true; // visual
        assert!(has_visual_changes(&a, &b));
    }

    #[test]
    fn has_motion_changes_utility() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.high_contrast = true;
        assert!(!has_motion_changes(&a, &b));

        b.reduced_motion = true;
        assert!(has_motion_changes(&a, &b));
    }

    #[test]
    fn change_label() {
        let c = PreferenceChange::HighContrast(true);
        assert_eq!(c.label(), "high-contrast");
    }

    #[test]
    fn change_classification() {
        let visual = PreferenceChange::InvertedColors(true);
        assert!(visual.is_visual());
        assert!(!visual.is_motion());
        assert!(!visual.is_keyboard());

        let motion = PreferenceChange::ReducedMotion(true);
        assert!(!motion.is_visual());
        assert!(motion.is_motion());

        let keyboard = PreferenceChange::StickyKeys(true);
        assert!(!keyboard.is_visual());
        assert!(keyboard.is_keyboard());
    }

    #[test]
    fn text_scale_epsilon() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        // Tiny change within epsilon — should NOT register.
        b.text_scale_factor = 1.0 + f32::EPSILON * 0.5;
        let changes = check_for_changes(&a, &b);
        assert!(changes.is_empty(), "tiny scale change within epsilon should be ignored");
    }

    #[test]
    fn text_scale_real_change() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.text_scale_factor = 1.25;
        let changes = check_for_changes(&a, &b);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes[0],
            PreferenceChange::TextScaleFactor(f) if (f - 1.25).abs() < 0.001
        ));
    }

    #[test]
    fn cursor_size_change() {
        let a = AccessibilityPreferences::default();
        let mut b = a.clone();
        b.cursor_size = CursorSize::ExtraLarge;
        let changes = check_for_changes(&a, &b);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes[0],
            PreferenceChange::CursorSize(CursorSize::ExtraLarge)
        ));
    }
}

#[cfg(test)]
mod platform_tests {
    use crate::platform::detect;
    use crate::prefs::AccessibilityPreferences;

    #[test]
    fn detect_returns_valid_prefs() {
        // Just verify it doesn't panic and returns reasonable defaults.
        let prefs = detect();
        assert!(prefs.text_scale_factor > 0.0);
        assert!(prefs.text_scale_factor < 10.0);
    }

    #[test]
    fn detect_is_deterministic() {
        // Two consecutive calls should return the same result
        // (system state shouldn't change between them).
        let a = detect();
        let b = detect();
        assert_eq!(a, b);
    }

    #[test]
    fn default_prefs_equal_themselves() {
        let a = AccessibilityPreferences::default();
        let b = AccessibilityPreferences::default();
        assert_eq!(a, b);
    }
}

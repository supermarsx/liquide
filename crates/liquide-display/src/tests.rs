#[cfg(test)]
mod tests {
    use crate::arrangement::DisplayArrangement;
    use crate::display::{DisplayId, DisplayInfo, Resolution, Rotation};
    use crate::night_light::{
        color_temperature_matrix, NightLight, NightLightSchedule,
    };
    use crate::profile::{detect_matching_profile, DisplayConfig, DisplayProfile};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_display(
        id: DisplayId,
        connector: &str,
        w: u32,
        h: u32,
        x: i32,
        y: i32,
        primary: bool,
    ) -> DisplayInfo {
        DisplayInfo {
            id,
            name: format!("Display {}", id),
            connector: connector.to_string(),
            resolution: Resolution::new(w, h),
            available_resolutions: vec![Resolution::new(w, h)],
            refresh_rate: 60.0,
            available_refresh_rates: vec![60.0],
            position: (x, y),
            rotation: Rotation::Normal,
            scale: 1.0,
            primary,
            enabled: true,
            physical_size_mm: Some((600, 340)),
            connected: true,
        }
    }

    // -----------------------------------------------------------------------
    // Resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolution_aspect_ratio_16_9() {
        assert_eq!(Resolution::FHD.aspect_ratio(), (16, 9));
    }

    #[test]
    fn resolution_aspect_ratio_16_10() {
        let r = Resolution::new(1920, 1200);
        assert_eq!(r.aspect_ratio(), (8, 5));
    }

    #[test]
    fn resolution_aspect_ratio_zero() {
        let r = Resolution::new(0, 1080);
        assert_eq!(r.aspect_ratio(), (0, 0));
    }

    #[test]
    fn resolution_pixel_count() {
        assert_eq!(Resolution::FHD.pixel_count(), 1920 * 1080);
    }

    #[test]
    fn resolution_dpi_calculation() {
        // 1920x1080 on a ~24" panel (530mm x 300mm).
        let dpi = Resolution::FHD.dpi(530, 300).unwrap();
        // Expected: ~93 DPI.
        assert!(dpi > 85.0 && dpi < 100.0, "dpi={}", dpi);
    }

    #[test]
    fn resolution_dpi_zero_physical() {
        assert!(Resolution::FHD.dpi(0, 0).is_none());
    }

    #[test]
    fn resolution_presets() {
        assert_eq!(Resolution::HD, Resolution::new(1280, 720));
        assert_eq!(Resolution::QHD, Resolution::new(2560, 1440));
        assert_eq!(Resolution::UHD_4K, Resolution::new(3840, 2160));
        assert_eq!(Resolution::UHD_5K, Resolution::new(5120, 2880));
    }

    #[test]
    fn resolution_display_format() {
        assert_eq!(format!("{}", Resolution::FHD), "1920x1080");
    }

    // -----------------------------------------------------------------------
    // Rotation tests
    // -----------------------------------------------------------------------

    #[test]
    fn rotation_degrees() {
        assert_eq!(Rotation::Normal.degrees(), 0);
        assert_eq!(Rotation::Right.degrees(), 90);
        assert_eq!(Rotation::Inverted.degrees(), 180);
        assert_eq!(Rotation::Left.degrees(), 270);
    }

    #[test]
    fn rotation_effective_resolution() {
        let res = Resolution::FHD;
        assert_eq!(
            Rotation::Left.effective_resolution(res),
            Resolution::new(1080, 1920)
        );
        assert_eq!(Rotation::Normal.effective_resolution(res), res);
        assert_eq!(Rotation::Inverted.effective_resolution(res), res);
    }

    // -----------------------------------------------------------------------
    // DisplayInfo tests
    // -----------------------------------------------------------------------

    #[test]
    fn display_info_bounds_no_scale() {
        let d = make_display(1, "DP-1", 1920, 1080, 0, 0, true);
        assert_eq!(d.bounds(), (0, 0, 1920, 1080));
    }

    #[test]
    fn display_info_bounds_with_scale() {
        let mut d = make_display(1, "DP-1", 3840, 2160, 100, 200, false);
        d.scale = 2.0;
        // Logical size = 3840/2 x 2160/2 = 1920 x 1080.
        assert_eq!(d.bounds(), (100, 200, 1920, 1080));
    }

    #[test]
    fn display_info_logical_dimensions() {
        let mut d = make_display(1, "DP-1", 3840, 2160, 0, 0, true);
        d.scale = 1.5;
        assert_eq!(d.logical_width(), 2560);
        assert_eq!(d.logical_height(), 1440);
    }

    // -----------------------------------------------------------------------
    // Arrangement tests
    // -----------------------------------------------------------------------

    #[test]
    fn arrangement_set_position() {
        let mut arr = DisplayArrangement::new(vec![make_display(1, "DP-1", 1920, 1080, 0, 0, true)]);
        assert!(arr.set_position(1, 500, 300));
        assert_eq!(arr.get(1).unwrap().position, (500, 300));
    }

    #[test]
    fn arrangement_set_position_missing() {
        let mut arr = DisplayArrangement::new(vec![]);
        assert!(!arr.set_position(99, 0, 0));
    }

    #[test]
    fn arrangement_set_primary() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        assert!(arr.set_primary(2));
        assert!(!arr.get(1).unwrap().primary);
        assert!(arr.get(2).unwrap().primary);
    }

    #[test]
    fn arrangement_align_horizontal() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ]);
        arr.align_horizontal(&[1, 2], 0, 0, 0);
        assert_eq!(arr.get(1).unwrap().position, (0, 0));
        assert_eq!(arr.get(2).unwrap().position, (1920, 0));
    }

    #[test]
    fn arrangement_align_horizontal_with_gap() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 0, 0, false),
        ]);
        arr.align_horizontal(&[1, 2], 10, 0, 0);
        assert_eq!(arr.get(2).unwrap().position, (1930, 0));
    }

    #[test]
    fn arrangement_align_vertical() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 0, 0, false),
        ]);
        arr.align_vertical(&[1, 2], 0, 0, 0);
        assert_eq!(arr.get(1).unwrap().position, (0, 0));
        assert_eq!(arr.get(2).unwrap().position, (0, 1080));
    }

    #[test]
    fn arrangement_bounds() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        assert_eq!(arr.bounds(), (0, 0, 3840, 1080));
    }

    #[test]
    fn arrangement_bounds_empty() {
        let arr = DisplayArrangement::new(vec![]);
        assert_eq!(arr.bounds(), (0, 0, 0, 0));
    }

    #[test]
    fn arrangement_display_at_point() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        assert_eq!(arr.display_at_point(100, 100), Some(1));
        assert_eq!(arr.display_at_point(2000, 500), Some(2));
        assert_eq!(arr.display_at_point(5000, 0), None);
    }

    #[test]
    fn arrangement_overlaps_detected() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1000, 0, false),
        ]);
        let overlaps = arr.overlaps();
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0], (1, 2));
    }

    #[test]
    fn arrangement_no_overlaps() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        assert!(arr.overlaps().is_empty());
    }

    #[test]
    fn arrangement_gaps_detected() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1970, 0, false),
        ]);
        let gaps = arr.gaps();
        // There should be a 50px wide gap between the two displays.
        assert!(!gaps.is_empty(), "expected gap between displays");
        let (gx, _gy, gw, _gh) = gaps[0];
        assert_eq!(gx, 1920);
        assert_eq!(gw, 50);
    }

    #[test]
    fn arrangement_no_gaps_adjacent() {
        let arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        assert!(arr.gaps().is_empty());
    }

    // -----------------------------------------------------------------------
    // Profile tests
    // -----------------------------------------------------------------------

    #[test]
    fn profile_save_and_restore() {
        let displays = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 2560, 1440, 1920, 0, false),
        ];
        let profile = DisplayProfile::save_current("Office", &displays);
        assert_eq!(profile.name, "Office");
        assert_eq!(profile.displays.len(), 2);
        assert_eq!(profile.displays[0].connector, "DP-1");
        assert_eq!(profile.displays[1].resolution, Resolution::new(2560, 1440));
    }

    #[test]
    fn profile_json_roundtrip() {
        let displays = vec![make_display(1, "DP-1", 1920, 1080, 0, 0, true)];
        let profile = DisplayProfile::save_current("Test", &displays);
        let json = profile.to_json().unwrap();
        let restored = DisplayProfile::from_json(&json).unwrap();
        assert_eq!(restored.name, "Test");
        assert_eq!(restored.displays[0].connector, "DP-1");
        assert_eq!(restored.displays[0].resolution, Resolution::new(1920, 1080));
    }

    #[test]
    fn profile_apply() {
        let mut displays = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ];
        let profile = DisplayProfile {
            name: "Custom".into(),
            displays: vec![DisplayConfig {
                connector: "HDMI-0".into(),
                resolution: Resolution::QHD,
                refresh_rate: 144.0,
                position: (0, 0),
                rotation: Rotation::Left,
                scale: 1.5,
                primary: true,
                enabled: true,
            }],
        };
        let matched = profile.apply(&mut displays);
        assert_eq!(matched, 1);
        assert_eq!(displays[1].resolution, Resolution::QHD);
        assert_eq!(displays[1].refresh_rate, 144.0);
        assert_eq!(displays[1].rotation, Rotation::Left);
        assert!(displays[1].primary);
    }

    #[test]
    fn profile_detect_matching() {
        let connected = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ];
        let profiles = vec![
            DisplayProfile {
                name: "Laptop".into(),
                displays: vec![DisplayConfig::from_display(&make_display(
                    1, "eDP-1", 1920, 1080, 0, 0, true,
                ))],
            },
            DisplayProfile {
                name: "Office".into(),
                displays: vec![
                    DisplayConfig::from_display(&connected[0]),
                    DisplayConfig::from_display(&connected[1]),
                ],
            },
        ];
        let matched = detect_matching_profile(&connected, &profiles);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "Office");
    }

    #[test]
    fn profile_detect_no_match() {
        let connected = vec![make_display(1, "VGA-1", 1024, 768, 0, 0, true)];
        let profiles = vec![DisplayProfile {
            name: "Other".into(),
            displays: vec![DisplayConfig::from_display(&make_display(
                1, "DP-1", 1920, 1080, 0, 0, true,
            ))],
        }];
        assert!(detect_matching_profile(&connected, &profiles).is_none());
    }

    // -----------------------------------------------------------------------
    // Night light tests
    // -----------------------------------------------------------------------

    #[test]
    fn night_light_default_disabled() {
        let nl = NightLight::default();
        assert!(!nl.enabled);
        assert_eq!(nl.color_matrix(), [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn night_light_warm_temperature() {
        let matrix = color_temperature_matrix(2700);
        // At 2700K, red should be ~1.0, green reduced, blue very reduced.
        assert!((matrix[0] - 1.0).abs() < 0.01, "r={}", matrix[0]);
        assert!(matrix[4] < 0.85, "g={}", matrix[4]);
        assert!(matrix[8] < 0.5, "b={}", matrix[8]);
    }

    #[test]
    fn night_light_daylight_near_identity() {
        let matrix = color_temperature_matrix(6500);
        // At 6500K (daylight), all channels should be close to 1.0.
        assert!((matrix[0] - 1.0).abs() < 0.05, "r={}", matrix[0]);
        assert!((matrix[4] - 1.0).abs() < 0.05, "g={}", matrix[4]);
        assert!((matrix[8] - 1.0).abs() < 0.05, "b={}", matrix[8]);
    }

    #[test]
    fn night_light_very_warm() {
        let matrix = color_temperature_matrix(1800);
        // Very warm — blue should be near 0.
        assert!(matrix[8] < 0.15, "b={}", matrix[8]);
    }

    #[test]
    fn night_light_custom_schedule_same_day() {
        let nl = NightLight {
            enabled: true,
            temperature_kelvin: 3000,
            schedule: NightLightSchedule::Custom {
                start_hour: 20,
                start_min: 0,
                end_hour: 23,
                end_min: 0,
            },
        };
        assert!(nl.is_active_at(21, 0));
        assert!(!nl.is_active_at(19, 0));
        assert!(!nl.is_active_at(23, 30));
    }

    #[test]
    fn night_light_custom_schedule_overnight() {
        let nl = NightLight {
            enabled: true,
            temperature_kelvin: 3000,
            schedule: NightLightSchedule::Custom {
                start_hour: 22,
                start_min: 0,
                end_hour: 6,
                end_min: 0,
            },
        };
        assert!(nl.is_active_at(23, 0));
        assert!(nl.is_active_at(3, 0));
        assert!(!nl.is_active_at(12, 0));
        assert!(!nl.is_active_at(7, 0));
    }

    #[test]
    fn night_light_manual_always_active() {
        let nl = NightLight {
            enabled: true,
            temperature_kelvin: 3400,
            schedule: NightLightSchedule::Manual,
        };
        assert!(nl.is_active_at(12, 0));
        assert!(nl.is_active_at(0, 0));
    }

    #[test]
    fn night_light_disabled_never_active() {
        let nl = NightLight {
            enabled: false,
            temperature_kelvin: 3400,
            schedule: NightLightSchedule::Manual,
        };
        assert!(!nl.is_active_at(12, 0));
    }

    #[test]
    fn night_light_sunset_sunrise_schedule() {
        let nl = NightLight {
            enabled: true,
            temperature_kelvin: 3000,
            schedule: NightLightSchedule::SunsetSunrise {
                latitude: 40.0,
                longitude: -74.0,
            },
        };
        // Night (midnight) should be active.
        assert!(nl.is_active_at(0, 0));
        // Midday should not be active.
        assert!(!nl.is_active_at(12, 0));
    }

    #[test]
    fn night_light_temperature_clamped() {
        let nl = NightLight::new(500); // Below minimum.
        assert_eq!(nl.temperature_kelvin, 1000);
        let nl2 = NightLight::new(99999);
        assert_eq!(nl2.temperature_kelvin, 10000);
    }

    #[test]
    fn color_matrix_is_diagonal() {
        // The color temperature matrix should always be diagonal.
        let m = color_temperature_matrix(4000);
        assert_eq!(m[1], 0.0);
        assert_eq!(m[2], 0.0);
        assert_eq!(m[3], 0.0);
        assert_eq!(m[5], 0.0);
        assert_eq!(m[6], 0.0);
        assert_eq!(m[7], 0.0);
    }

    // -----------------------------------------------------------------------
    // Xrandr parser test (Linux only at compile time, but we can test the
    // function signature and structure)
    // -----------------------------------------------------------------------

    #[test]
    fn resolution_serde_roundtrip() {
        let res = Resolution::UHD_4K;
        let json = serde_json::to_string(&res).unwrap();
        let restored: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, res);
    }

    #[test]
    fn rotation_serde_roundtrip() {
        let rot = Rotation::Left;
        let json = serde_json::to_string(&rot).unwrap();
        let restored: Rotation = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rot);
    }

    // ===================================================================
    // Arrangement policy & auto_arrange tests
    // ===================================================================

    use crate::arrangement::{
        auto_arrange, auto_arrange_default, fix_gaps, primary_monitor,
        snap_to_grid, ArrangementPolicy, MonitorArrangement,
        MonitorPosition,
    };

    #[test]
    fn auto_arrange_side_by_side() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        let result = auto_arrange(&monitors, &ArrangementPolicy::SideBySide);
        // DP-1 < HDMI-0 alphabetically, so DP-1 first at x=0.
        assert_eq!(result.position_of(1), Some((0, 0)));
        assert_eq!(result.position_of(2), Some((1920, 0)));
    }

    #[test]
    fn auto_arrange_stacked() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        let result = auto_arrange(&monitors, &ArrangementPolicy::Stacked);
        assert_eq!(result.position_of(1), Some((0, 0)));
        assert_eq!(result.position_of(2), Some((0, 1080)));
    }

    #[test]
    fn auto_arrange_mirror() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        let result = auto_arrange(&monitors, &ArrangementPolicy::Mirror);
        assert_eq!(result.position_of(1), Some((0, 0)));
        assert_eq!(result.position_of(2), Some((0, 0)));
    }

    #[test]
    fn auto_arrange_custom() {
        let policy = ArrangementPolicy::Custom(vec![
            MonitorPosition { id: 1, x: 100, y: 200 },
            MonitorPosition { id: 2, x: 300, y: 400 },
        ]);
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        let result = auto_arrange(&monitors, &policy);
        assert_eq!(result.position_of(1), Some((100, 200)));
        assert_eq!(result.position_of(2), Some((300, 400)));
    }

    #[test]
    fn auto_arrange_default_is_side_by_side() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        let result = auto_arrange_default(&monitors);
        assert_eq!(result.position_of(1), Some((0, 0)));
        assert_eq!(result.position_of(2), Some((1920, 0)));
    }

    #[test]
    fn auto_arrange_skips_disabled() {
        let mut monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ];
        monitors[1].enabled = false;
        let result = auto_arrange_default(&monitors);
        assert_eq!(result.positions.len(), 1);
        assert_eq!(result.position_of(1), Some((0, 0)));
        assert_eq!(result.position_of(2), None);
    }

    #[test]
    fn snap_to_grid_rounds_positions() {
        let mut arr = MonitorArrangement {
            positions: vec![(1, 17, 23), (2, 1930, 5)],
        };
        snap_to_grid(&mut arr, 10);
        assert_eq!(arr.position_of(1), Some((20, 20)));
        assert_eq!(arr.position_of(2), Some((1930, 10)));
    }

    #[test]
    fn snap_to_grid_noop_for_zero() {
        let mut arr = MonitorArrangement {
            positions: vec![(1, 17, 23)],
        };
        snap_to_grid(&mut arr, 0);
        assert_eq!(arr.position_of(1), Some((17, 23)));
    }

    #[test]
    fn snap_to_grid_noop_for_one() {
        let mut arr = MonitorArrangement {
            positions: vec![(1, 17, 23)],
        };
        snap_to_grid(&mut arr, 1);
        assert_eq!(arr.position_of(1), Some((17, 23)));
    }

    #[test]
    fn arrangement_apply_to() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 0, 0, false),
        ]);
        let ma = MonitorArrangement {
            positions: vec![(1, 100, 200), (2, 300, 400)],
        };
        ma.apply_to(&mut arr);
        assert_eq!(arr.get(1).unwrap().position, (100, 200));
        assert_eq!(arr.get(2).unwrap().position, (300, 400));
    }

    #[test]
    fn fix_gaps_closes_horizontal_gap() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1970, 0, false),
        ]);
        let gaps = fix_gaps(&mut arr);
        assert!(!gaps.is_empty());
        // After fix, display 2 should be shifted left by 50px.
        assert_eq!(arr.get(2).unwrap().position.0, 1920);
    }

    #[test]
    fn fix_gaps_no_change_when_adjacent() {
        let mut arr = DisplayArrangement::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0, false),
        ]);
        let gaps = fix_gaps(&mut arr);
        assert!(gaps.is_empty());
        assert_eq!(arr.get(2).unwrap().position, (1920, 0));
    }

    #[test]
    fn primary_monitor_returns_marked_primary() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 2560, 1440, 1920, 0, true),
        ];
        assert_eq!(primary_monitor(&monitors), Some(2));
    }

    #[test]
    fn primary_monitor_picks_highest_res() {
        let monitors = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, false),
            make_display(2, "HDMI-0", 3840, 2160, 1920, 0, false),
        ];
        assert_eq!(primary_monitor(&monitors), Some(2));
    }

    #[test]
    fn primary_monitor_empty() {
        let monitors: Vec<DisplayInfo> = vec![];
        assert_eq!(primary_monitor(&monitors), None);
    }

    #[test]
    fn arrangement_policy_default_is_side_by_side() {
        assert_eq!(ArrangementPolicy::default(), ArrangementPolicy::SideBySide);
    }

    // ===================================================================
    // Wallpaper tests
    // ===================================================================

    use crate::wallpaper::{
        compute_span_crop, compute_wallpaper_transform,
        SlideshowOrder, WallpaperConfig, WallpaperMode,
    };

    #[test]
    fn wallpaper_fill_wider_image() {
        // Image wider than monitor (2:1 vs 16:9).
        let t = compute_wallpaper_transform(1920, 1080, 3840, 1080, WallpaperMode::Fill);
        // Should crop horizontally, full height.
        assert!((t.src.2 - 3840.0).abs() < 1.0 || t.src.2 < 3840.0);
        assert_eq!(t.dst, (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn wallpaper_fill_exact_match() {
        let t = compute_wallpaper_transform(1920, 1080, 1920, 1080, WallpaperMode::Fill);
        assert!((t.src.0).abs() < 0.01);
        assert!((t.src.1).abs() < 0.01);
        assert!((t.src.2 - 1920.0).abs() < 0.01);
        assert!((t.src.3 - 1080.0).abs() < 0.01);
    }

    #[test]
    fn wallpaper_fit_letterbox_horizontal() {
        // Tall image on wide monitor -> letterbox left/right.
        let t = compute_wallpaper_transform(1920, 1080, 1080, 1920, WallpaperMode::Fit);
        // Entire source should be used.
        assert!((t.src.0).abs() < 0.01);
        assert!((t.src.1).abs() < 0.01);
        // Destination should be centered horizontally.
        assert!(t.dst.0 > 0.0, "expected left padding, got {}", t.dst.0);
        assert!((t.dst.1).abs() < 0.01);
    }

    #[test]
    fn wallpaper_stretch() {
        let t = compute_wallpaper_transform(1920, 1080, 800, 600, WallpaperMode::Stretch);
        assert_eq!(t.src, (0.0, 0.0, 800.0, 600.0));
        assert_eq!(t.dst, (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn wallpaper_center_small_image() {
        let t = compute_wallpaper_transform(1920, 1080, 640, 480, WallpaperMode::Center);
        // Source is the full image.
        assert_eq!(t.src, (0.0, 0.0, 640.0, 480.0));
        // Destination is centered.
        assert!((t.dst.0 - 640.0).abs() < 0.01); // (1920-640)/2
        assert!((t.dst.1 - 300.0).abs() < 0.01); // (1080-480)/2
    }

    #[test]
    fn wallpaper_center_large_image() {
        let t = compute_wallpaper_transform(1920, 1080, 3840, 2160, WallpaperMode::Center);
        // Image is larger, so it should be cropped from center.
        assert!((t.src.0 - 960.0).abs() < 0.01); // (3840-1920)/2
        assert!((t.src.1 - 540.0).abs() < 0.01); // (2160-1080)/2
        assert!((t.src.2 - 1920.0).abs() < 0.01);
        assert!((t.src.3 - 1080.0).abs() < 0.01);
        assert!((t.dst.0).abs() < 0.01);
        assert!((t.dst.1).abs() < 0.01);
    }

    #[test]
    fn wallpaper_tile() {
        let t = compute_wallpaper_transform(1920, 1080, 256, 256, WallpaperMode::Tile);
        // Single tile at origin.
        assert_eq!(t.src, (0.0, 0.0, 256.0, 256.0));
        assert_eq!(t.dst, (0.0, 0.0, 256.0, 256.0));
    }

    #[test]
    fn wallpaper_span_crop() {
        // Two monitors side-by-side: 1920+1920 = 3840 total width.
        let t = compute_span_crop(3840, 1080, 1920, 0, 1920, 1080, 3840, 2160);
        // Should get the right half of the scaled image.
        assert!(t.src.0 > 0.0, "span crop should start past 0");
        assert_eq!(t.dst, (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn wallpaper_config_default() {
        let cfg = WallpaperConfig::default();
        assert_eq!(cfg.mode, WallpaperMode::Fill);
        assert!(cfg.path.is_empty());
        assert!(cfg.slideshow.is_none());
    }

    #[test]
    fn wallpaper_config_slideshow() {
        let cfg = WallpaperConfig::slideshow(
            "/pictures/wallpapers",
            WallpaperMode::Fill,
            300,
            SlideshowOrder::Random,
        );
        assert!(cfg.slideshow.is_some());
        let ss = cfg.slideshow.unwrap();
        assert_eq!(ss.interval_secs, 300);
        assert_eq!(ss.order, SlideshowOrder::Random);
    }

    #[test]
    fn wallpaper_mode_default_is_fill() {
        assert_eq!(WallpaperMode::default(), WallpaperMode::Fill);
    }

    #[test]
    fn wallpaper_zero_dimensions() {
        let t = compute_wallpaper_transform(0, 0, 100, 100, WallpaperMode::Fill);
        assert_eq!(t.dst, (0.0, 0.0, 0.0, 0.0));
    }

    // ===================================================================
    // Output profile tests
    // ===================================================================

    use crate::output_profile::{
        builtin_docked, builtin_laptop_only, builtin_presentation,
        OutputProfile, ProfileStore,
    };

    #[test]
    fn output_profile_builtin_laptop() {
        let op = builtin_laptop_only("eDP-1", Resolution::FHD);
        assert_eq!(op.profile.name, "laptop-only");
        assert!(op.has_tag("laptop"));
        assert!(op.auto_generated);
        assert_eq!(op.profile.displays.len(), 1);
    }

    #[test]
    fn output_profile_builtin_docked() {
        let op = builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD);
        assert_eq!(op.profile.name, "docked");
        assert!(op.has_tag("docked"));
        assert_eq!(op.profile.displays.len(), 2);
        // External monitor should be primary.
        assert!(op.profile.displays[1].primary);
    }

    #[test]
    fn output_profile_builtin_presentation() {
        let op = builtin_presentation("eDP-1", Resolution::FHD, "HDMI-0");
        assert_eq!(op.profile.name, "presentation");
        assert!(op.has_tag("mirror"));
        // Both at (0,0).
        assert_eq!(op.profile.displays[0].position, (0, 0));
        assert_eq!(op.profile.displays[1].position, (0, 0));
    }

    #[test]
    fn output_profile_json_roundtrip() {
        let op = builtin_laptop_only("eDP-1", Resolution::FHD);
        let json = op.to_json().unwrap();
        let restored = OutputProfile::from_json(&json).unwrap();
        assert_eq!(restored.profile.name, "laptop-only");
        assert_eq!(restored.tags, vec!["laptop"]);
    }

    #[test]
    fn profile_store_add_and_get() {
        let mut store = ProfileStore::new();
        let op = builtin_laptop_only("eDP-1", Resolution::FHD);
        store.add(op);
        assert_eq!(store.len(), 1);
        assert!(store.get("laptop-only").is_some());
    }

    #[test]
    fn profile_store_remove() {
        let mut store = ProfileStore::new();
        store.add(builtin_laptop_only("eDP-1", Resolution::FHD));
        assert!(store.remove("laptop-only"));
        assert_eq!(store.len(), 0);
        assert!(!store.remove("nonexistent"));
    }

    #[test]
    fn profile_store_detect_match() {
        let mut store = ProfileStore::new();
        store.add(builtin_laptop_only("eDP-1", Resolution::FHD));
        store.add(builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD));

        let connected = vec![
            make_display(1, "eDP-1", 1920, 1080, 0, 0, true),
            make_display(2, "DP-1", 2560, 1440, 1920, 0, false),
        ];
        let detected = store.detect(&connected);
        assert!(detected.is_some());
        assert_eq!(detected.unwrap().profile.name, "docked");
    }

    #[test]
    fn profile_store_detect_no_match() {
        let mut store = ProfileStore::new();
        store.add(builtin_laptop_only("eDP-1", Resolution::FHD));
        let connected = vec![make_display(1, "VGA-1", 1024, 768, 0, 0, true)];
        assert!(store.detect(&connected).is_none());
    }

    #[test]
    fn profile_store_save_current() {
        let mut store = ProfileStore::new();
        let displays = vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0, true),
            make_display(2, "HDMI-0", 2560, 1440, 1920, 0, false),
        ];
        store.save_current("my-setup", &displays, vec!["home".into()]);
        assert_eq!(store.len(), 1);
        let p = store.get("my-setup").unwrap();
        assert_eq!(p.profile.displays.len(), 2);
    }

    #[test]
    fn profile_store_json_roundtrip() {
        let mut store = ProfileStore::new();
        store.add(builtin_laptop_only("eDP-1", Resolution::FHD));
        store.add(builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD));
        let json = store.to_json().unwrap();
        let restored = ProfileStore::from_json(&json).unwrap();
        assert_eq!(restored.len(), 2);
    }

    #[test]
    fn profile_store_names() {
        let mut store = ProfileStore::new();
        store.add(builtin_laptop_only("eDP-1", Resolution::FHD));
        store.add(builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD));
        let names = store.names();
        assert!(names.contains(&"laptop-only"));
        assert!(names.contains(&"docked"));
    }

    #[test]
    fn profile_store_empty() {
        let store = ProfileStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn profile_store_priority_ordering() {
        let mut store = ProfileStore::new();
        // Add two profiles matching the same connector set, different priorities.
        let mut low = builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD);
        low.priority = 5;
        low.profile.name = "low-prio".to_string();
        let mut high = builtin_docked("eDP-1", Resolution::FHD, "DP-1", Resolution::QHD);
        high.priority = 100;
        high.profile.name = "high-prio".to_string();
        store.add(low);
        store.add(high);

        let connected = vec![
            make_display(1, "eDP-1", 1920, 1080, 0, 0, true),
            make_display(2, "DP-1", 2560, 1440, 1920, 0, false),
        ];
        let detected = store.detect(&connected).unwrap();
        assert_eq!(detected.profile.name, "high-prio");
    }

    // ===================================================================
    // DPMS tests
    // ===================================================================

    use crate::dpms::{DpmsController, DpmsPolicy, DpmsState};

    #[test]
    fn dpms_state_defaults_on() {
        assert_eq!(DpmsState::default(), DpmsState::On);
    }

    #[test]
    fn dpms_state_depth_ordering() {
        assert!(DpmsState::On.depth() < DpmsState::Standby.depth());
        assert!(DpmsState::Standby.depth() < DpmsState::Suspend.depth());
        assert!(DpmsState::Suspend.depth() < DpmsState::Off.depth());
    }

    #[test]
    fn dpms_state_is_power_save() {
        assert!(!DpmsState::On.is_power_save());
        assert!(DpmsState::Standby.is_power_save());
        assert!(DpmsState::Suspend.is_power_save());
        assert!(DpmsState::Off.is_power_save());
    }

    #[test]
    fn dpms_policy_default_timeouts() {
        let p = DpmsPolicy::default();
        assert!(p.enabled);
        assert_eq!(p.standby_timeout_secs, 300);
        assert_eq!(p.suspend_timeout_secs, 600);
        assert_eq!(p.off_timeout_secs, 900);
    }

    #[test]
    fn dpms_policy_target_state_progression() {
        let p = DpmsPolicy::default();
        assert_eq!(p.target_state(0), DpmsState::On);
        assert_eq!(p.target_state(299), DpmsState::On);
        assert_eq!(p.target_state(300), DpmsState::Standby);
        assert_eq!(p.target_state(599), DpmsState::Standby);
        assert_eq!(p.target_state(600), DpmsState::Suspend);
        assert_eq!(p.target_state(899), DpmsState::Suspend);
        assert_eq!(p.target_state(900), DpmsState::Off);
        assert_eq!(p.target_state(9999), DpmsState::Off);
    }

    #[test]
    fn dpms_policy_single_timeout() {
        let p = DpmsPolicy::single_timeout(120);
        assert_eq!(p.target_state(0), DpmsState::On);
        assert_eq!(p.target_state(119), DpmsState::On);
        assert_eq!(p.target_state(120), DpmsState::Off);
    }

    #[test]
    fn dpms_policy_disabled() {
        let p = DpmsPolicy::disabled();
        assert_eq!(p.target_state(99999), DpmsState::On);
    }

    #[test]
    fn dpms_controller_tick_transitions() {
        let policy = DpmsPolicy {
            standby_timeout_secs: 10,
            suspend_timeout_secs: 20,
            off_timeout_secs: 30,
            enabled: true,
        };
        let mut ctrl = DpmsController::new(policy);
        assert_eq!(ctrl.state(), DpmsState::On);

        // Tick 9 seconds — still on.
        assert_eq!(ctrl.tick(9), None);
        assert_eq!(ctrl.state(), DpmsState::On);

        // Tick 1 more — standby.
        assert_eq!(ctrl.tick(1), Some(DpmsState::Standby));
        assert_eq!(ctrl.state(), DpmsState::Standby);

        // Tick 10 more — suspend.
        assert_eq!(ctrl.tick(10), Some(DpmsState::Suspend));
        assert_eq!(ctrl.state(), DpmsState::Suspend);

        // Tick 10 more — off.
        assert_eq!(ctrl.tick(10), Some(DpmsState::Off));
        assert_eq!(ctrl.state(), DpmsState::Off);
    }

    #[test]
    fn dpms_controller_wake_on_input() {
        let policy = DpmsPolicy {
            standby_timeout_secs: 5,
            suspend_timeout_secs: 0,
            off_timeout_secs: 0,
            enabled: true,
        };
        let mut ctrl = DpmsController::new(policy);
        // Go to standby.
        ctrl.tick(5);
        assert_eq!(ctrl.state(), DpmsState::Standby);

        // User input.
        ctrl.notify_input();
        assert!(ctrl.has_wake_pending());
        assert_eq!(ctrl.idle_secs(), 0);

        // Next tick processes wake.
        assert_eq!(ctrl.tick(0), Some(DpmsState::On));
        assert_eq!(ctrl.state(), DpmsState::On);
        assert!(!ctrl.has_wake_pending());
    }

    #[test]
    fn dpms_controller_force_state() {
        let mut ctrl = DpmsController::new(DpmsPolicy::default());
        ctrl.force_state(DpmsState::Off);
        assert_eq!(ctrl.state(), DpmsState::Off);
        ctrl.force_state(DpmsState::On);
        assert_eq!(ctrl.state(), DpmsState::On);
        assert_eq!(ctrl.idle_secs(), 0);
    }

    #[test]
    fn dpms_controller_no_change_on_repeated_tick() {
        let mut ctrl = DpmsController::new(DpmsPolicy::single_timeout(10));
        ctrl.tick(10);
        assert_eq!(ctrl.state(), DpmsState::Off);
        // Further ticks should not re-trigger.
        assert_eq!(ctrl.tick(1), None);
        assert_eq!(ctrl.tick(100), None);
    }

    // ===================================================================
    // Color profile tests
    // ===================================================================

    use crate::color_profile::{ColorProfile, ColorSpace, IccProfileStore};

    #[test]
    fn color_profile_srgb_gamma_ramp() {
        let profile = ColorProfile::srgb();
        let ramp = profile.gamma_ramp();
        // ramp[0] should be 0.0 (black).
        assert!((ramp[0]).abs() < 1e-6);
        // ramp[255] should be ~1.0 (white).
        assert!((ramp[255] - 1.0).abs() < 0.01);
        // Monotonically increasing.
        for i in 1..256 {
            assert!(ramp[i] >= ramp[i - 1], "non-monotonic at {}", i);
        }
    }

    #[test]
    fn color_profile_inverse_ramp() {
        let profile = ColorProfile::srgb();
        let ramp = profile.inverse_gamma_ramp();
        assert!((ramp[0]).abs() < 1e-6);
        assert!((ramp[255] - 1.0).abs() < 0.01);
        for i in 1..256 {
            assert!(ramp[i] >= ramp[i - 1], "non-monotonic at {}", i);
        }
    }

    #[test]
    fn color_profile_display_p3() {
        let profile = ColorProfile::display_p3();
        assert_eq!(profile.color_space, ColorSpace::DisplayP3);
        assert_eq!(profile.gamma, 2.2);
        // P3 has wider red primary than sRGB.
        assert!(profile.red_primary.0 > ColorProfile::srgb().red_primary.0);
    }

    #[test]
    fn color_profile_adobe_rgb() {
        let profile = ColorProfile::adobe_rgb();
        assert_eq!(profile.color_space, ColorSpace::AdobeRgb);
        // Adobe RGB has different green primary than sRGB.
        assert!((profile.green_primary.0 - 0.21).abs() < 0.01);
    }

    #[test]
    fn color_profile_custom() {
        let profile = ColorProfile::custom(
            "Test",
            1.8,
            (0.3127, 0.3290),
            (0.64, 0.33),
            (0.30, 0.60),
            (0.15, 0.06),
        );
        assert_eq!(profile.color_space, ColorSpace::Custom);
        assert_eq!(profile.gamma, 1.8);
    }

    #[test]
    fn color_profile_xyz_matrix_srgb() {
        let profile = ColorProfile::srgb();
        let m = profile.xyz_to_rgb_matrix();
        // The XYZ->sRGB matrix should have positive diagonal entries.
        assert!(m[0] > 0.0, "m[0]={}", m[0]);
        assert!(m[4] > 0.0, "m[4]={}", m[4]);
        assert!(m[8] > 0.0, "m[8]={}", m[8]);
    }

    #[test]
    fn color_profile_xyz_matrix_identity_white() {
        // Multiplying the XYZ of D65 white through XYZ->sRGB should give ~(1,1,1).
        let profile = ColorProfile::srgb();
        let m = profile.xyz_to_rgb_matrix();
        // D65 white in XYZ: (0.9505, 1.0, 1.0890)
        let xw = 0.9505;
        let yw = 1.0;
        let zw = 1.0890;
        let r = m[0] * xw + m[1] * yw + m[2] * zw;
        let g = m[3] * xw + m[4] * yw + m[5] * zw;
        let b = m[6] * xw + m[7] * yw + m[8] * zw;
        assert!((r - 1.0).abs() < 0.05, "r={}", r);
        assert!((g - 1.0).abs() < 0.05, "g={}", g);
        assert!((b - 1.0).abs() < 0.05, "b={}", b);
    }

    #[test]
    fn icc_store_new_has_builtins() {
        let store = IccProfileStore::new();
        assert_eq!(store.profile_count(), 3);
        assert!(store.get_profile("sRGB IEC61966-2.1").is_some());
        assert!(store.get_profile("Display P3").is_some());
        assert!(store.get_profile("Adobe RGB (1998)").is_some());
    }

    #[test]
    fn icc_store_add_and_get_profile() {
        let mut store = IccProfileStore::new();
        let custom = ColorProfile::custom(
            "My Profile",
            2.0,
            (0.3127, 0.3290),
            (0.64, 0.33),
            (0.30, 0.60),
            (0.15, 0.06),
        );
        store.add_profile(custom);
        assert_eq!(store.profile_count(), 4);
        assert!(store.get_profile("My Profile").is_some());
    }

    #[test]
    fn icc_store_apply_and_get_monitor_profile() {
        let mut store = IccProfileStore::new();
        assert!(store.apply_profile(1, "Display P3"));
        let assigned = store.get_monitor_profile(1).unwrap();
        assert_eq!(assigned.color_space, ColorSpace::DisplayP3);
    }

    #[test]
    fn icc_store_apply_nonexistent_profile() {
        let mut store = IccProfileStore::new();
        assert!(!store.apply_profile(1, "Nonexistent"));
    }

    #[test]
    fn icc_store_remove_assignment() {
        let mut store = IccProfileStore::new();
        store.apply_profile(1, "Display P3");
        assert!(store.remove_assignment(1));
        assert!(store.get_monitor_profile(1).is_none());
        assert!(!store.remove_assignment(1));
    }

    #[test]
    fn icc_store_gamma_ramp_default() {
        let store = IccProfileStore::new();
        // No assignment — should return sRGB ramp.
        let ramp = store.gamma_ramp_for(1);
        let srgb_ramp = ColorProfile::srgb().gamma_ramp();
        assert_eq!(ramp, srgb_ramp);
    }

    #[test]
    fn icc_store_gamma_ramp_assigned() {
        let mut store = IccProfileStore::new();
        store.apply_profile(1, "Adobe RGB (1998)");
        let ramp = store.gamma_ramp_for(1);
        // Adobe RGB uses pure gamma, not sRGB piecewise.
        let adobe = ColorProfile::adobe_rgb();
        let expected = adobe.gamma_ramp();
        assert_eq!(ramp, expected);
    }

    #[test]
    fn icc_store_profile_names() {
        let store = IccProfileStore::new();
        let names = store.profile_names();
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn color_space_default_is_srgb() {
        assert_eq!(ColorSpace::default(), ColorSpace::Srgb);
    }
}

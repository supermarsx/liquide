use crate::schema::{SchemaRegistry, SettingSchema, SettingType};
use crate::value::SettingValue;

/// Register all built-in DE default settings into the schema registry.
pub fn register_defaults(schema: &mut SchemaRegistry) {
    // ── appearance ──────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "appearance.theme".into(),
        display_name: "Theme".into(),
        description: "Desktop theme".into(),
        setting_type: SettingType::Enum(vec![
            "liquid-glass".into(),
            "night".into(),
            "midday".into(),
            "sunset".into(),
        ]),
        default: SettingValue::Str("liquid-glass".into()),
        category: "appearance".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "appearance.accent-color".into(),
        display_name: "Accent Color".into(),
        description: "System accent color for highlights and focus indicators".into(),
        setting_type: SettingType::Color,
        default: SettingValue::Color(0, 122, 255, 255),
        category: "appearance".into(),
        subcategory: "colors".into(),
    });
    schema.register(SettingSchema {
        key: "appearance.font-family".into(),
        display_name: "Font Family".into(),
        description: "Default UI font family".into(),
        setting_type: SettingType::String,
        default: SettingValue::Str("Inter".into()),
        category: "appearance".into(),
        subcategory: "fonts".into(),
    });
    schema.register(SettingSchema {
        key: "appearance.font-size".into(),
        display_name: "Font Size".into(),
        description: "Base font size in pixels".into(),
        setting_type: SettingType::FloatRange(8.0, 32.0),
        default: SettingValue::Float(14.0),
        category: "appearance".into(),
        subcategory: "fonts".into(),
    });
    schema.register(SettingSchema {
        key: "appearance.icon-theme".into(),
        display_name: "Icon Theme".into(),
        description: "Icon theme for system and application icons".into(),
        setting_type: SettingType::String,
        default: SettingValue::Str("default".into()),
        category: "appearance".into(),
        subcategory: "icons".into(),
    });

    // ── desktop ─────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "desktop.wallpaper-path".into(),
        display_name: "Wallpaper".into(),
        description: "Path to the desktop wallpaper image".into(),
        setting_type: SettingType::Path,
        default: SettingValue::Path(String::new()),
        category: "desktop".into(),
        subcategory: "background".into(),
    });
    schema.register(SettingSchema {
        key: "desktop.wallpaper-mode".into(),
        display_name: "Wallpaper Mode".into(),
        description: "How the wallpaper image is displayed".into(),
        setting_type: SettingType::Enum(vec![
            "fill".into(),
            "fit".into(),
            "stretch".into(),
            "tile".into(),
            "center".into(),
        ]),
        default: SettingValue::Str("fill".into()),
        category: "desktop".into(),
        subcategory: "background".into(),
    });
    schema.register(SettingSchema {
        key: "desktop.show-desktop-icons".into(),
        display_name: "Show Desktop Icons".into(),
        description: "Display file and folder icons on the desktop".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(true),
        category: "desktop".into(),
        subcategory: "general".into(),
    });

    // ── dock ────────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "dock.position".into(),
        display_name: "Dock Position".into(),
        description: "Edge of the screen where the dock is placed".into(),
        setting_type: SettingType::Enum(vec![
            "bottom".into(),
            "left".into(),
            "right".into(),
        ]),
        default: SettingValue::Str("bottom".into()),
        category: "dock".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "dock.auto-hide".into(),
        display_name: "Auto-Hide Dock".into(),
        description: "Automatically hide the dock when not in use".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "dock".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "dock.icon-size".into(),
        display_name: "Dock Icon Size".into(),
        description: "Size of dock icons in pixels".into(),
        setting_type: SettingType::IntRange(24, 128),
        default: SettingValue::Int(48),
        category: "dock".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "dock.magnification".into(),
        display_name: "Dock Magnification".into(),
        description: "Magnify dock icons on hover".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "dock".into(),
        subcategory: "general".into(),
    });

    // ── input ───────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "input.mouse-speed".into(),
        display_name: "Mouse Speed".into(),
        description: "Mouse pointer acceleration multiplier".into(),
        setting_type: SettingType::FloatRange(0.1, 10.0),
        default: SettingValue::Float(1.0),
        category: "input".into(),
        subcategory: "mouse".into(),
    });
    schema.register(SettingSchema {
        key: "input.scroll-natural".into(),
        display_name: "Natural Scrolling".into(),
        description: "Reverse the scroll direction (content follows finger)".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "input".into(),
        subcategory: "mouse".into(),
    });
    schema.register(SettingSchema {
        key: "input.double-click-ms".into(),
        display_name: "Double Click Interval".into(),
        description: "Maximum milliseconds between clicks for a double-click".into(),
        setting_type: SettingType::IntRange(200, 1000),
        default: SettingValue::Int(400),
        category: "input".into(),
        subcategory: "mouse".into(),
    });
    schema.register(SettingSchema {
        key: "input.keyboard-repeat-delay".into(),
        display_name: "Keyboard Repeat Delay".into(),
        description: "Delay before key repeat starts (ms)".into(),
        setting_type: SettingType::IntRange(100, 2000),
        default: SettingValue::Int(400),
        category: "input".into(),
        subcategory: "keyboard".into(),
    });
    schema.register(SettingSchema {
        key: "input.keyboard-repeat-rate".into(),
        display_name: "Keyboard Repeat Rate".into(),
        description: "Key repeats per second once repeat starts".into(),
        setting_type: SettingType::IntRange(1, 100),
        default: SettingValue::Int(30),
        category: "input".into(),
        subcategory: "keyboard".into(),
    });

    // ── display ─────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "display.night-light".into(),
        display_name: "Night Light".into(),
        description: "Reduce blue light emission in the evening".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "display".into(),
        subcategory: "color".into(),
    });
    schema.register(SettingSchema {
        key: "display.night-light-temperature".into(),
        display_name: "Night Light Temperature".into(),
        description: "Color temperature in Kelvin when night light is active".into(),
        setting_type: SettingType::IntRange(1700, 6500),
        default: SettingValue::Int(4000),
        category: "display".into(),
        subcategory: "color".into(),
    });
    schema.register(SettingSchema {
        key: "display.scaling".into(),
        display_name: "Display Scaling".into(),
        description: "UI scaling factor (1.0 = 100%)".into(),
        setting_type: SettingType::FloatRange(0.5, 4.0),
        default: SettingValue::Float(1.0),
        category: "display".into(),
        subcategory: "general".into(),
    });

    // ── power ───────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "power.screen-blank-minutes".into(),
        display_name: "Turn Off Screen After".into(),
        description: "Minutes of inactivity before blanking the screen (0 = never)".into(),
        setting_type: SettingType::IntRange(0, 120),
        default: SettingValue::Int(5),
        category: "power".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "power.suspend-minutes".into(),
        display_name: "Automatic Suspend After".into(),
        description: "Minutes of inactivity before suspend (0 = never)".into(),
        setting_type: SettingType::IntRange(0, 480),
        default: SettingValue::Int(30),
        category: "power".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "power.lid-close-action".into(),
        display_name: "Lid Close Action".into(),
        description: "Action when the laptop lid is closed".into(),
        setting_type: SettingType::Enum(vec![
            "suspend".into(),
            "hibernate".into(),
            "shutdown".into(),
            "nothing".into(),
        ]),
        default: SettingValue::Str("suspend".into()),
        category: "power".into(),
        subcategory: "general".into(),
    });

    // ── notifications ───────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "notifications.do-not-disturb".into(),
        display_name: "Do Not Disturb".into(),
        description: "Suppress all notification popups".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "notifications".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "notifications.show-previews".into(),
        display_name: "Show Previews".into(),
        description: "Show notification content in popups".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(true),
        category: "notifications".into(),
        subcategory: "general".into(),
    });
    schema.register(SettingSchema {
        key: "notifications.sound".into(),
        display_name: "Notification Sound".into(),
        description: "Play a sound when notifications arrive".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(true),
        category: "notifications".into(),
        subcategory: "general".into(),
    });

    // ── privacy ─────────────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "privacy.screen-lock-enabled".into(),
        display_name: "Screen Lock".into(),
        description: "Require authentication after inactivity".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(true),
        category: "privacy".into(),
        subcategory: "lock".into(),
    });
    schema.register(SettingSchema {
        key: "privacy.screen-lock-delay-seconds".into(),
        display_name: "Screen Lock Delay".into(),
        description: "Seconds of inactivity before the screen locks".into(),
        setting_type: SettingType::IntRange(0, 3600),
        default: SettingValue::Int(300),
        category: "privacy".into(),
        subcategory: "lock".into(),
    });

    // ── accessibility ───────────────────────────────────────────────
    schema.register(SettingSchema {
        key: "accessibility.large-text".into(),
        display_name: "Large Text".into(),
        description: "Increase text size throughout the interface".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "accessibility".into(),
        subcategory: "visual".into(),
    });
    schema.register(SettingSchema {
        key: "accessibility.high-contrast".into(),
        display_name: "High Contrast".into(),
        description: "Increase contrast for better visibility".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "accessibility".into(),
        subcategory: "visual".into(),
    });
    schema.register(SettingSchema {
        key: "accessibility.reduce-motion".into(),
        display_name: "Reduce Motion".into(),
        description: "Minimize animations and transitions".into(),
        setting_type: SettingType::Bool,
        default: SettingValue::Bool(false),
        category: "accessibility".into(),
        subcategory: "visual".into(),
    });
    schema.register(SettingSchema {
        key: "accessibility.cursor-size".into(),
        display_name: "Cursor Size".into(),
        description: "Size of the mouse cursor".into(),
        setting_type: SettingType::Enum(vec![
            "small".into(),
            "medium".into(),
            "large".into(),
            "extra-large".into(),
        ]),
        default: SettingValue::Str("medium".into()),
        category: "accessibility".into(),
        subcategory: "pointer".into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_defaults_populates_schema() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        // Should have registered all the settings above
        assert!(schema.len() >= 30, "expected at least 30 defaults, got {}", schema.len());
    }

    #[test]
    fn all_defaults_validate() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        for (key, s) in schema.iter() {
            let result = schema.validate(key, &s.default);
            assert!(
                result.is_ok(),
                "default for '{}' failed validation: {:?}",
                key,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn appearance_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("appearance");
        assert!(keys.contains(&"appearance.theme"));
        assert!(keys.contains(&"appearance.accent-color"));
        assert!(keys.contains(&"appearance.font-family"));
        assert!(keys.contains(&"appearance.font-size"));
        assert!(keys.contains(&"appearance.icon-theme"));
        assert_eq!(keys.len(), 5);
    }

    #[test]
    fn desktop_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("desktop");
        assert!(keys.contains(&"desktop.wallpaper-path"));
        assert!(keys.contains(&"desktop.wallpaper-mode"));
        assert!(keys.contains(&"desktop.show-desktop-icons"));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn dock_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("dock");
        assert!(keys.contains(&"dock.position"));
        assert!(keys.contains(&"dock.auto-hide"));
        assert!(keys.contains(&"dock.icon-size"));
        assert!(keys.contains(&"dock.magnification"));
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn input_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("input");
        assert!(keys.contains(&"input.mouse-speed"));
        assert!(keys.contains(&"input.scroll-natural"));
        assert!(keys.contains(&"input.double-click-ms"));
        assert!(keys.contains(&"input.keyboard-repeat-delay"));
        assert!(keys.contains(&"input.keyboard-repeat-rate"));
        assert_eq!(keys.len(), 5);
    }

    #[test]
    fn display_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("display");
        assert!(keys.contains(&"display.night-light"));
        assert!(keys.contains(&"display.night-light-temperature"));
        assert!(keys.contains(&"display.scaling"));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn power_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("power");
        assert!(keys.contains(&"power.screen-blank-minutes"));
        assert!(keys.contains(&"power.suspend-minutes"));
        assert!(keys.contains(&"power.lid-close-action"));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn notifications_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("notifications");
        assert!(keys.contains(&"notifications.do-not-disturb"));
        assert!(keys.contains(&"notifications.show-previews"));
        assert!(keys.contains(&"notifications.sound"));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn privacy_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("privacy");
        assert!(keys.contains(&"privacy.screen-lock-enabled"));
        assert!(keys.contains(&"privacy.screen-lock-delay-seconds"));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn accessibility_category_keys() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let keys = schema.keys_in_category("accessibility");
        assert!(keys.contains(&"accessibility.large-text"));
        assert!(keys.contains(&"accessibility.high-contrast"));
        assert!(keys.contains(&"accessibility.reduce-motion"));
        assert!(keys.contains(&"accessibility.cursor-size"));
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn all_categories_present() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);
        let cats = schema.categories();
        for expected in &[
            "appearance", "desktop", "dock", "input", "display",
            "power", "notifications", "privacy", "accessibility",
        ] {
            assert!(
                cats.contains(expected),
                "missing category: {}",
                expected
            );
        }
    }

    #[test]
    fn default_values_are_expected() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);

        assert_eq!(schema.get("appearance.theme").unwrap().default, SettingValue::Str("liquid-glass".into()));
        assert_eq!(schema.get("appearance.font-size").unwrap().default, SettingValue::Float(14.0));
        assert_eq!(schema.get("dock.icon-size").unwrap().default, SettingValue::Int(48));
        assert_eq!(schema.get("input.mouse-speed").unwrap().default, SettingValue::Float(1.0));
        assert_eq!(schema.get("display.scaling").unwrap().default, SettingValue::Float(1.0));
        assert_eq!(schema.get("power.screen-blank-minutes").unwrap().default, SettingValue::Int(5));
        assert_eq!(schema.get("notifications.do-not-disturb").unwrap().default, SettingValue::Bool(false));
        assert_eq!(schema.get("privacy.screen-lock-enabled").unwrap().default, SettingValue::Bool(true));
        assert_eq!(schema.get("accessibility.reduce-motion").unwrap().default, SettingValue::Bool(false));
        assert_eq!(schema.get("accessibility.cursor-size").unwrap().default, SettingValue::Str("medium".into()));
    }

    #[test]
    fn theme_enum_validates_correctly() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);

        assert!(schema.validate("appearance.theme", &SettingValue::Str("liquid-glass".into())).is_ok());
        assert!(schema.validate("appearance.theme", &SettingValue::Str("night".into())).is_ok());
        assert!(schema.validate("appearance.theme", &SettingValue::Str("midday".into())).is_ok());
        assert!(schema.validate("appearance.theme", &SettingValue::Str("sunset".into())).is_ok());
        assert!(schema.validate("appearance.theme", &SettingValue::Str("invalid".into())).is_err());
    }

    #[test]
    fn wallpaper_mode_enum_validates() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);

        for mode in &["fill", "fit", "stretch", "tile", "center"] {
            assert!(
                schema.validate("desktop.wallpaper-mode", &SettingValue::Str(mode.to_string())).is_ok(),
                "mode '{}' should be valid", mode
            );
        }
        assert!(schema.validate("desktop.wallpaper-mode", &SettingValue::Str("zoom".into())).is_err());
    }

    #[test]
    fn night_light_temp_range() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);

        assert!(schema.validate("display.night-light-temperature", &SettingValue::Int(1700)).is_ok());
        assert!(schema.validate("display.night-light-temperature", &SettingValue::Int(6500)).is_ok());
        assert!(schema.validate("display.night-light-temperature", &SettingValue::Int(1699)).is_err());
        assert!(schema.validate("display.night-light-temperature", &SettingValue::Int(6501)).is_err());
    }

    #[test]
    fn dock_icon_size_range() {
        let mut schema = SchemaRegistry::new();
        register_defaults(&mut schema);

        assert!(schema.validate("dock.icon-size", &SettingValue::Int(24)).is_ok());
        assert!(schema.validate("dock.icon-size", &SettingValue::Int(128)).is_ok());
        assert!(schema.validate("dock.icon-size", &SettingValue::Int(23)).is_err());
        assert!(schema.validate("dock.icon-size", &SettingValue::Int(129)).is_err());
    }
}

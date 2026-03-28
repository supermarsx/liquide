use crate::schema::SettingCategory;

/// A settings panel groups related settings
#[derive(Debug, Clone)]
pub struct SettingsPanel {
    pub category: SettingCategory,
    pub sections: Vec<PanelSection>,
}

#[derive(Debug, Clone)]
pub struct PanelSection {
    pub title: String,
    pub description: Option<String>,
    pub setting_keys: Vec<String>,  // keys within the store
}

/// Build the default panel layout
pub fn default_panels() -> Vec<SettingsPanel> {
    vec![
        SettingsPanel {
            category: SettingCategory::Appearance,
            sections: vec![
                PanelSection { title: "Theme".into(), description: None, setting_keys: vec!["appearance.theme".into()] },
                PanelSection { title: "Fonts".into(), description: None, setting_keys: vec!["appearance.font_size".into()] },
                PanelSection { title: "Colors".into(), description: None, setting_keys: vec!["appearance.accent_color".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Desktop,
            sections: vec![
                PanelSection { title: "Background".into(), description: None, setting_keys: vec!["desktop.wallpaper".into(), "desktop.show_icons".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::WindowManagement,
            sections: vec![
                PanelSection { title: "Focus".into(), description: None, setting_keys: vec!["wm.focus_policy".into()] },
                PanelSection { title: "Tiling".into(), description: None, setting_keys: vec!["wm.tiling_gap".into(), "wm.snap_enabled".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Input,
            sections: vec![
                PanelSection { title: "Mouse".into(), description: None, setting_keys: vec!["input.mouse_speed".into(), "input.natural_scroll".into()] },
                PanelSection { title: "Keyboard".into(), description: None, setting_keys: vec!["input.key_repeat_delay".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Display,
            sections: vec![
                PanelSection { title: "Scale".into(), description: None, setting_keys: vec!["display.dpi_scale".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Power,
            sections: vec![
                PanelSection { title: "Power Saving".into(), description: None, setting_keys: vec!["power.screen_blank_minutes".into(), "power.auto_suspend_minutes".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Notifications,
            sections: vec![
                PanelSection { title: "Notifications".into(), description: None, setting_keys: vec!["notifications.dnd".into(), "notifications.show_on_lockscreen".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Accessibility,
            sections: vec![
                PanelSection { title: "Visual".into(), description: None, setting_keys: vec!["a11y.high_contrast".into(), "a11y.large_text".into(), "a11y.reduce_motion".into()] },
                PanelSection { title: "Assistive".into(), description: None, setting_keys: vec!["a11y.screen_reader".into()] },
            ],
        },
        SettingsPanel {
            category: SettingCategory::Privacy,
            sections: vec![
                PanelSection { title: "Screen Lock".into(), description: None, setting_keys: vec!["privacy.lock_on_suspend".into(), "privacy.auto_lock_minutes".into()] },
            ],
        },
    ]
}

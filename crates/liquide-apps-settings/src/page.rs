//! Settings pages composed of sections and entries.

use crate::category::Category;
use crate::entry::SettingEntry;

/// A section within a settings page (e.g., "Resolution" within Display).
#[derive(Debug, Clone)]
pub struct Section {
    /// Section title.
    pub title: String,
    /// Entry keys belonging to this section.
    pub entry_keys: Vec<String>,
}

impl Section {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entry_keys: Vec::new(),
        }
    }

    pub fn add_key(&mut self, key: impl Into<String>) {
        self.entry_keys.push(key.into());
    }
}

/// A settings page for a single category.
#[derive(Debug, Clone)]
pub struct SettingsPage {
    pub category: Category,
    pub sections: Vec<Section>,
}

impl SettingsPage {
    #[must_use]
    pub fn new(category: Category) -> Self {
        Self {
            category,
            sections: Vec::new(),
        }
    }

    pub fn add_section(&mut self, section: Section) {
        self.sections.push(section);
    }

    /// Total number of entry keys across all sections.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.sections.iter().map(|s| s.entry_keys.len()).sum()
    }

    /// Gather all entry keys.
    #[must_use]
    pub fn all_keys(&self) -> Vec<&str> {
        self.sections
            .iter()
            .flat_map(|s| s.entry_keys.iter().map(String::as_str))
            .collect()
    }
}

/// Build the default pages and entries for all categories.
#[must_use]
pub fn default_pages() -> (Vec<SettingsPage>, Vec<SettingEntry>) {
    let mut entries = Vec::new();
    let mut pages = Vec::new();

    // ---- Display ----
    {
        let cat = Category::Display;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Resolution & Scaling");
        let e = SettingEntry::choice(
            "display.resolution",
            "Resolution",
            "Screen resolution",
            cat,
            "Resolution & Scaling",
            vec!["1920x1080".into(), "2560x1440".into(), "3840x2160".into()],
            "1920x1080",
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::slider(
            "display.scale",
            "UI Scale",
            "Interface scaling factor",
            cat,
            "Resolution & Scaling",
            1.0,
            3.0,
            0.25,
            1.0,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Refresh");
        let e = SettingEntry::choice(
            "display.refresh_rate",
            "Refresh Rate",
            "Monitor refresh rate",
            cat,
            "Refresh",
            vec!["60".into(), "75".into(), "120".into(), "144".into()],
            "60",
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Night Light");
        let e = SettingEntry::toggle(
            "display.night_light",
            "Night Light",
            "Reduce blue light at night",
            cat,
            "Night Light",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Input ----
    {
        let cat = Category::Input;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Keyboard");
        let e = SettingEntry::choice(
            "input.keyboard_layout",
            "Keyboard Layout",
            "Active keyboard layout",
            cat,
            "Keyboard",
            vec![
                "us".into(),
                "gb".into(),
                "de".into(),
                "fr".into(),
                "es".into(),
            ],
            "us",
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::slider(
            "input.repeat_delay",
            "Repeat Delay",
            "Key repeat delay in ms",
            cat,
            "Keyboard",
            100.0,
            1000.0,
            50.0,
            400.0,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Mouse");
        let e = SettingEntry::slider(
            "input.mouse_speed",
            "Mouse Speed",
            "Pointer acceleration",
            cat,
            "Mouse",
            0.1,
            3.0,
            0.1,
            1.0,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::toggle(
            "input.natural_scroll",
            "Natural Scrolling",
            "Reverse scroll direction",
            cat,
            "Mouse",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Audio ----
    {
        let cat = Category::Audio;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Output");
        let e = SettingEntry::slider(
            "audio.volume",
            "Volume",
            "Master output volume",
            cat,
            "Output",
            0.0,
            100.0,
            1.0,
            50.0,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::toggle(
            "audio.mute",
            "Mute",
            "Mute all audio output",
            cat,
            "Output",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Input");
        let e = SettingEntry::slider(
            "audio.input_volume",
            "Input Volume",
            "Microphone input level",
            cat,
            "Input",
            0.0,
            100.0,
            1.0,
            80.0,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Effects");
        let e = SettingEntry::toggle(
            "audio.system_sounds",
            "System Sounds",
            "Play sounds for notifications and events",
            cat,
            "Effects",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Network ----
    {
        let cat = Category::Network;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Connection");
        let e = SettingEntry::text(
            "network.hostname",
            "Hostname",
            "System hostname",
            cat,
            "Connection",
            64,
            "liquide-desktop",
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Proxy");
        let e = SettingEntry::toggle(
            "network.proxy_enabled",
            "Use Proxy",
            "Route traffic through proxy",
            cat,
            "Proxy",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::text(
            "network.proxy_address",
            "Proxy Address",
            "HTTP proxy address",
            cat,
            "Proxy",
            256,
            "",
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Appearance ----
    {
        let cat = Category::Appearance;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Theme");
        let e = SettingEntry::choice(
            "appearance.theme",
            "Theme",
            "Color theme for the desktop",
            cat,
            "Theme",
            vec!["Light".into(), "Dark".into(), "Auto".into()],
            "Auto",
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::choice(
            "appearance.accent_color",
            "Accent Color",
            "Primary accent color",
            cat,
            "Theme",
            vec![
                "Blue".into(),
                "Teal".into(),
                "Green".into(),
                "Orange".into(),
                "Purple".into(),
                "Red".into(),
            ],
            "Blue",
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Fonts");
        let e = SettingEntry::text(
            "appearance.font_family",
            "Font Family",
            "Default UI font",
            cat,
            "Fonts",
            128,
            "Inter",
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::slider(
            "appearance.font_size",
            "Font Size",
            "Default font size in points",
            cat,
            "Fonts",
            8.0,
            32.0,
            1.0,
            13.0,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Privacy ----
    {
        let cat = Category::Privacy;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Permissions");
        let e = SettingEntry::toggle(
            "privacy.location",
            "Location Services",
            "Allow apps to use location",
            cat,
            "Permissions",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::toggle(
            "privacy.camera",
            "Camera",
            "Allow apps to access camera",
            cat,
            "Permissions",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::toggle(
            "privacy.microphone",
            "Microphone",
            "Allow apps to access microphone",
            cat,
            "Permissions",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Screen");
        let e = SettingEntry::toggle(
            "privacy.screen_sharing",
            "Screen Sharing",
            "Allow screen sharing",
            cat,
            "Screen",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- Users ----
    {
        let cat = Category::Users;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Login");
        let e = SettingEntry::toggle(
            "users.auto_login",
            "Automatic Login",
            "Skip login screen on boot",
            cat,
            "Login",
            false,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    // ---- System ----
    {
        let cat = Category::System;
        let mut page = SettingsPage::new(cat);

        let mut sec = Section::new("Date & Time");
        let e = SettingEntry::toggle(
            "system.auto_timezone",
            "Automatic Time Zone",
            "Set time zone automatically",
            cat,
            "Date & Time",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);

        let e = SettingEntry::toggle(
            "system.24h_clock",
            "24-Hour Clock",
            "Use 24-hour time format",
            cat,
            "Date & Time",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Language");
        let e = SettingEntry::choice(
            "system.language",
            "Language",
            "System language",
            cat,
            "Language",
            vec![
                "en_US".into(),
                "en_GB".into(),
                "de_DE".into(),
                "fr_FR".into(),
                "es_ES".into(),
                "ja_JP".into(),
            ],
            "en_US",
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        let mut sec = Section::new("Updates");
        let e = SettingEntry::toggle(
            "system.auto_updates",
            "Automatic Updates",
            "Install updates automatically",
            cat,
            "Updates",
            true,
        );
        sec.add_key(&e.key);
        entries.push(e);
        page.add_section(sec);

        pages.push(page);
    }

    (pages, entries)
}

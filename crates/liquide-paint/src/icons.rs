//! Icon name → numeric ID mapping for the paint pipeline.
//!
//! Maps named icon identifiers (from DOM `data-icon` attributes) to the
//! numeric IDs used by the renderer's built-in vector icon system.
//! This is the single source of truth for icon name resolution.

/// Map a named icon string to the numeric icon ID used by the renderer.
///
/// Returns 0 for unrecognised names (the renderer draws a simple filled
/// rect as a fallback for unknown IDs).
#[must_use]
pub fn icon_id_for_name(name: &str) -> u32 {
    match name {
        "folder" | "file-manager" => 1,
        "terminal" | "console" | "utilities-terminal" => 2,
        "web-browser" | "browser" | "internet-web-browser" => 3,
        "preferences-system" | "settings" | "system-preferences" => 4,
        "calculator" | "accessories-calculator" => 5,
        "text-editor" | "accessories-text-editor" => 6,
        "audio-x-generic" | "music" | "multimedia-audio-player" => 7,
        "camera" | "camera-photo" => 8,
        "mail" | "internet-mail" => 9,
        "calendar" | "office-calendar" => 10,
        "clock" | "preferences-clock" => 11,
        "network-wireless" | "wifi" => 12,
        "battery" | "battery-full" => 13,
        "notification" | "preferences-desktop-notification" => 14,
        "search" | "system-search" | "edit-find" => 15,
        "power" | "system-shutdown" => 16,
        "audio-volume-high" | "volume" => 17,
        "user-trash" | "trash" => 18,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_icons_resolve() {
        assert_eq!(icon_id_for_name("folder"), 1);
        assert_eq!(icon_id_for_name("terminal"), 2);
        assert_eq!(icon_id_for_name("browser"), 3);
        assert_eq!(icon_id_for_name("settings"), 4);
        assert_eq!(icon_id_for_name("trash"), 18);
    }

    #[test]
    fn unknown_icon_returns_zero() {
        assert_eq!(icon_id_for_name("nonexistent"), 0);
        assert_eq!(icon_id_for_name(""), 0);
    }
}

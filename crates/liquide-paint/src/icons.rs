//! Icon name → numeric ID mapping for the paint pipeline.
//!
//! Maps named icon identifiers (from DOM `data-icon` attributes) to the
//! numeric IDs used by the renderer's built-in vector icon system.
//! This is the **single source of truth** for icon name resolution: the
//! duplicate helper in `liquide-shell/src/scene_builder.rs` delegates here so
//! the two cannot drift.
//!
//! The numeric IDs form the shared contract with the renderer's glyph table
//! (`liquide-renderer-cpu/src/icons.rs`): `icon_id_from_u32` must decode every
//! non-zero ID produced here, and every ID must have vector art. Keep the two
//! tables in sync — the id/glyph coverage test in the renderer asserts it.
//!
//! ## Fallback policy
//! An unrecognised (but non-empty) name resolves to `0`. The painter still
//! emits an icon display item for id `0`, and the renderer draws a visible
//! **placeholder** glyph (a bordered box with a dot) so a future unmapped icon
//! is visible and debuggable rather than silently blank. Empty names are not
//! icons and are skipped by the painter.

/// Map a named icon string to the numeric icon ID used by the renderer.
///
/// Returns 0 for unrecognised names. The renderer draws a visible placeholder
/// glyph for id 0 (see module docs), so an unmapped name is never blank.
///
/// The numeric IDs correspond 1:1 with `IconId` in
/// `liquide-renderer-cpu/src/icons.rs`.
#[must_use]
pub fn icon_id_for_name(name: &str) -> u32 {
    match name {
        // ── 1: Folder / file manager (generic folder glyph) ──
        "folder" | "file-manager" | "system-file-manager" | "folder-documents"
        | "folder-download" | "folder-downloads" | "folder-music" | "folder-pictures"
        | "folder-videos" | "folder-desktop" | "user-desktop" | "folder-code"
        | "folder-temp" => 1,

        // ── 2: Terminal / console ──
        "terminal" | "console" | "utilities-terminal" | "system-run" => 2,

        // ── 3: Web browser / globe ──
        "web-browser" | "browser" | "internet-web-browser" | "globe" => 3,

        // ── 4: System settings / gear (generic preferences) ──
        "preferences-system" | "settings" | "system-preferences" | "gear" | "system-ui"
        | "preferences-other" | "preferences-system-windows" | "preferences-system-privacy"
        | "preferences-desktop-peripherals" | "preferences-desktop-accessibility"
        | "preferences-desktop-locale" => 4,

        // ── 5: Calculator ──
        "calculator" | "accessories-calculator" => 5,

        // ── 6: Text editor / document (page glyph) ──
        "text-editor" | "accessories-text-editor" | "text-x-generic" | "text-plain"
        | "text-x-uri" | "text-x-source" | "text-x-markup" | "document-open"
        | "document-copy" | "document-multiple" | "document-properties"
        | "application-pdf" | "file-text" => 6,

        // ── 7: Music / audio file ──
        "audio-x-generic" | "music" | "multimedia-audio-player" | "file-audio" => 7,

        // ── 8: Camera / photo ──
        "camera" | "camera-photo" => 8,

        // ── 9: Mail / envelope ──
        "mail" | "internet-mail" => 9,

        // ── 10: Calendar ──
        "calendar" | "office-calendar" => 10,

        // ── 11: Clock / time ──
        "clock" | "preferences-clock" | "preferences-system-time" => 11,

        // ── 12: Wi-Fi / wireless ──
        "network-wireless" | "wifi" => 12,

        // ── 13: Battery ──
        "battery" | "battery-full" | "battery-low" | "battery-critical" => 13,

        // ── 14: Notification / bell (also generic info) ──
        "notification" | "preferences-desktop-notification"
        | "preferences-desktop-notifications" | "dialog-information" => 14,

        // ── 15: Search / find ──
        "search" | "system-search" | "edit-find" => 15,

        // ── 16: Power / shutdown ──
        "power" | "system-shutdown" | "system-reboot" | "system-suspend"
        | "preferences-system-power" => 16,

        // ── 17: Volume / sound ──
        "audio-volume-high" | "volume" | "audio-volume" | "audio-volume-change"
        | "preferences-desktop-sound" | "speaker" | "audio-speakers"
        | "audio-headphones" => 17,

        // ── 18: Trash / delete ──
        "user-trash" | "trash" | "edit-delete" => 18,

        // ── 19: Home folder ──
        "folder-home" => 19,

        // ── 20: Open / new folder ──
        "folder-open" | "folder-new" => 20,

        // ── 21: Starred / favourite ──
        "starred" | "bookmark" => 21,

        // ── 22: Recent documents ──
        "document-open-recent" => 22,

        // ── 23: Network server / wired ──
        "network-server" | "network-wired" | "network-workgroup" | "network-manager"
        | "network-monitor" | "preferences-system-network" => 23,

        // ── 24: Lock / password ──
        "lock" | "dialog-password" => 24,

        // ── 25: Warning / error ──
        "warning" | "dialog-warning" | "dialog-error" => 25,

        // ── 26: Edit / pencil (generic edit) ──
        "edit" | "document-edit" => 26,

        // ── 27: Package / archive ──
        "package-x-generic" | "package-install" | "package-remove" | "package-upgrade"
        | "system-update" => 27,

        // ── 28: Window minimize ──
        "window-minimize" | "minus" => 28,

        // ── 29: Window maximize ──
        "window-maximize" | "maximize" => 29,

        // ── 30: Wallpaper / picture (also generic image files) ──
        "preferences-desktop-wallpaper" | "preferences-desktop-theme"
        | "image-x-generic" | "image-viewer" | "file-image" => 30,

        // ── 31: Display / monitor ──
        "preferences-desktop-display" => 31,

        // ── 32: User / person ──
        "user-avatar" | "system-users" => 32,

        // ── 33: Disk drive ──
        "drive-harddisk" | "drive-removable" | "drive-removable-media"
        | "media-eject" => 33,

        // ── 34: Window close / X ──
        "window-close" | "x" => 34,

        // ── 35: Cut (scissors) ──
        "edit-cut" | "cut" => 35,

        // ── 36: Copy (two overlapping pages) ──
        "edit-copy" | "copy" => 36,

        // ── 37: Paste (clipboard) ──
        "edit-paste" | "paste" => 37,

        // ── 38: Undo (curved arrow left) ──
        "edit-undo" | "undo" => 38,

        // ── 39: Redo (curved arrow right) ──
        "edit-redo" | "redo" => 39,

        // ── 40: Process-stop / forbidden ──
        "process-stop" | "stop" | "action-unavailable" => 40,

        // ── 41: Video / movie file ──
        "video-x-generic" | "file-video" | "video" | "multimedia-video-player" => 41,

        // ── 42: Generic file / document ──
        "application-x-generic" | "file" | "file-generic" | "unknown"
        | "text-x-preview" => 42,

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

    /// Cut / Copy / Paste / Undo / Redo must each resolve to a DISTINCT
    /// non-zero id so they render distinguishable glyphs (they used to all
    /// collapse onto id 26, a single pencil). Teeth: mapping any two of these
    /// edit ops to the same id turns this RED.
    #[test]
    fn edit_ops_resolve_to_distinct_ids() {
        let names = ["edit-cut", "edit-copy", "edit-paste", "edit-undo", "edit-redo"];
        let ids: Vec<u32> = names.iter().map(|n| icon_id_for_name(n)).collect();
        for (name, &id) in names.iter().zip(&ids) {
            assert_ne!(id, 0, "`{name}` must map to a real glyph, not the placeholder");
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(
                    ids[i], ids[j],
                    "`{}` and `{}` must map to DISTINCT ids (both = {})",
                    names[i], names[j], ids[i]
                );
            }
        }
        // The generic `edit` pencil is deliberately unchanged (still id 26) and
        // must differ from every specific edit op.
        let edit = icon_id_for_name("edit");
        assert_eq!(edit, 26);
        assert!(!ids.contains(&edit), "specific edit ops must not reuse the pencil id");
    }

    /// `process-stop` (authorization / forbidden action) must resolve to a real
    /// glyph rather than the id-0 placeholder box. Teeth: dropping the mapping
    /// turns this RED.
    #[test]
    fn process_stop_resolves() {
        assert_ne!(icon_id_for_name("process-stop"), 0);
    }

    /// File-type names an embed peer requests via `data-icon` must all resolve
    /// to real glyphs so embedded files show a type icon, not a placeholder.
    #[test]
    fn file_type_names_resolve() {
        for name in [
            "text-x-generic",
            "file-text",
            "image-x-generic",
            "file-image",
            "audio-x-generic",
            "file-audio",
            "video-x-generic",
            "file-video",
            "application-x-generic",
            "file",
            "folder",
        ] {
            assert_ne!(icon_id_for_name(name), 0, "file-type name `{name}` must resolve");
        }
    }

    /// The previously-blank names named in the visual-bug hunt (symptom 1)
    /// must now resolve to non-zero IDs. Teeth: any of these mapping back to 0
    /// (a regression) turns this test RED.
    #[test]
    fn previously_blank_names_now_resolve() {
        for name in [
            "preferences-desktop-wallpaper",
            "folder-home",
            "starred",
            "document-open-recent",
            "network-server",
            "globe",
            "gear",
            "lock",
            "warning",
            "battery-low",
            "folder-open",
            "folder-new",
            "edit-cut",
            "edit-copy",
            "edit-paste",
            "package-x-generic",
            "window-minimize",
            "window-maximize",
        ] {
            assert_ne!(icon_id_for_name(name), 0, "`{name}` must map to a glyph");
        }
    }

    /// Every `data-icon` name that is actually produced somewhere in the
    /// codebase must resolve to a non-zero ID (so it renders a real glyph, not
    /// the placeholder). Enumerated from the icon producers: context-menu
    /// presets, Files sidebar/places/favorites, settings schema, session menu,
    /// dialogs, notifications, tray, dock, launcher. Teeth: a used name mapping
    /// to 0 turns this RED.
    #[test]
    fn all_used_names_resolve() {
        const USED: &[&str] = &[
            // context-menu presets
            "camera", "terminal", "folder", "web-browser", "preferences-system",
            "edit-paste", "folder-new", "edit-cut", "edit-copy", "package-x-generic",
            "document-properties", "edit-undo", "edit-redo", "window-minimize",
            "window-maximize", "edit-delete", "document-open",
            // Files sidebar / places / favorites / file picker
            "folder-home", "user-trash", "document-open-recent", "starred",
            "system-search", "network-server", "folder-documents", "folder-download",
            "folder-downloads", "folder-music", "folder-pictures", "folder-videos",
            "folder-desktop",
            // settings schema categories
            "preferences-desktop-theme", "preferences-desktop-wallpaper",
            "preferences-desktop-display", "preferences-system-windows",
            "preferences-desktop-peripherals", "preferences-system-power",
            "preferences-desktop-notifications", "preferences-desktop-accessibility",
            "preferences-system-privacy", "preferences-desktop-sound",
            "preferences-system-network", "preferences-desktop-locale",
            "preferences-other",
            // session / status / dialogs / notifications / launcher
            "power", "lock", "clock", "battery", "volume", "search", "globe",
            "gear", "warning", "battery-low", "mail", "folder-open",
            // window-decoration button glyphs (asset template)
            "minus", "maximize", "x",
        ];
        for name in USED {
            assert_ne!(
                icon_id_for_name(name),
                0,
                "used data-icon name `{name}` resolves to blank (id 0)"
            );
        }
    }
}

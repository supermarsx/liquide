//! Places sidebar model.
//!
//! Aggregates favourites, mounted devices, network shares, and special
//! virtual folders (Trash, Recent) into a single ordered list suitable for
//! rendering in a sidebar.  Modelled after the GNOME Files / Nautilus places
//! sidebar.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PlaceType
// ---------------------------------------------------------------------------

/// Category of a place item in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceType {
    /// User bookmark / favourite.
    Bookmark,
    /// Mounted block device or removable media.
    Device,
    /// Network share (SMB, NFS, SFTP, etc.).
    Network,
    /// The trash virtual folder.
    Trash,
    /// The recent-files virtual folder.
    Recent,
    /// Visual separator between sections (not clickable).
    Separator,
}

impl std::fmt::Display for PlaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bookmark => write!(f, "bookmark"),
            Self::Device => write!(f, "device"),
            Self::Network => write!(f, "network"),
            Self::Trash => write!(f, "trash"),
            Self::Recent => write!(f, "recent"),
            Self::Separator => write!(f, "separator"),
        }
    }
}

// ---------------------------------------------------------------------------
// PlaceItem
// ---------------------------------------------------------------------------

/// A single entry in the places sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceItem {
    /// Display label.
    pub label: String,
    /// Icon name (freedesktop icon-naming-spec).
    pub icon: String,
    /// URI (e.g. `file:///home/user`, `trash:///`).
    pub uri: String,
    /// Category of this place.
    pub place_type: PlaceType,
    /// Whether the device can be ejected (only relevant for removable media).
    pub is_ejectable: bool,
    /// Free space in bytes, if known.
    pub free_space: Option<u64>,
}

impl PlaceItem {
    /// Create a bookmark place.
    #[must_use]
    pub fn bookmark(label: &str, icon: &str, uri: &str) -> Self {
        Self {
            label: label.to_string(),
            icon: icon.to_string(),
            uri: uri.to_string(),
            place_type: PlaceType::Bookmark,
            is_ejectable: false,
            free_space: None,
        }
    }

    /// Create a device place.
    #[must_use]
    pub fn device(
        label: &str,
        icon: &str,
        uri: &str,
        ejectable: bool,
        free_space: Option<u64>,
    ) -> Self {
        Self {
            label: label.to_string(),
            icon: icon.to_string(),
            uri: uri.to_string(),
            place_type: PlaceType::Device,
            is_ejectable: ejectable,
            free_space,
        }
    }

    /// Create a network place.
    #[must_use]
    pub fn network(label: &str, uri: &str) -> Self {
        Self {
            label: label.to_string(),
            icon: "network-server".to_string(),
            uri: uri.to_string(),
            place_type: PlaceType::Network,
            is_ejectable: false,
            free_space: None,
        }
    }

    /// Create a separator item.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            icon: String::new(),
            uri: String::new(),
            place_type: PlaceType::Separator,
            is_ejectable: false,
            free_space: None,
        }
    }

    /// Whether this item is a visual separator.
    #[must_use]
    pub fn is_separator(&self) -> bool {
        self.place_type == PlaceType::Separator
    }
}

// ---------------------------------------------------------------------------
// PlacesModel
// ---------------------------------------------------------------------------

/// Aggregated places model for the sidebar.
pub struct PlacesModel {
    items: Vec<PlaceItem>,
    /// Mounted devices.
    devices: Vec<PlaceItem>,
    /// Network shares.
    network_shares: Vec<PlaceItem>,
    /// Whether to show the Trash item.
    show_trash: bool,
    /// Whether to show the Recent item.
    show_recent: bool,
}

impl PlacesModel {
    /// Create a new model with default bookmarks, Trash, and Recent.
    #[must_use]
    pub fn new() -> Self {
        let mut model = Self {
            items: Vec::new(),
            devices: Vec::new(),
            network_shares: Vec::new(),
            show_trash: true,
            show_recent: true,
        };
        model.refresh();
        model
    }

    /// Create an empty model.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            devices: Vec::new(),
            network_shares: Vec::new(),
            show_trash: true,
            show_recent: true,
        }
    }

    /// Rebuild the aggregated item list from all sources.
    ///
    /// Also probes the real filesystem for the existence of standard user
    /// directories and, on Windows, for available drive letters.
    pub fn refresh(&mut self) {
        self.items.clear();

        // Bookmarks section — only add directories that actually exist.
        let home = home_dir();
        let bookmarks = [
            ("Home", "folder-home", home.clone()),
            ("Documents", "folder-documents", format!("{home}/Documents")),
            ("Downloads", "folder-download", format!("{home}/Downloads")),
            ("Music", "folder-music", format!("{home}/Music")),
            ("Pictures", "folder-pictures", format!("{home}/Pictures")),
            ("Videos", "folder-videos", format!("{home}/Videos")),
        ];
        for (label, icon, path) in &bookmarks {
            // Home always shown; others only if they exist on disk.
            if *label == "Home" || std::path::Path::new(path).is_dir() {
                self.items
                    .push(PlaceItem::bookmark(label, icon, &format!("file://{path}")));
            }
        }

        // Auto-detect drives / root filesystem.
        let detected_devices = detect_system_devices();
        if !detected_devices.is_empty() || !self.devices.is_empty() {
            self.items.push(PlaceItem::separator());
            for dev in &detected_devices {
                self.items.push(dev.clone());
            }
            for dev in &self.devices {
                // Skip duplicates from auto-detection.
                if !detected_devices.iter().any(|d| d.uri == dev.uri) {
                    self.items.push(dev.clone());
                }
            }
        }

        // Separator before network shares.
        if !self.network_shares.is_empty() {
            self.items.push(PlaceItem::separator());
            for share in &self.network_shares {
                self.items.push(share.clone());
            }
        }

        // Separator before virtual folders.
        let has_virtual = self.show_trash || self.show_recent;
        if has_virtual {
            self.items.push(PlaceItem::separator());
        }
        if self.show_recent {
            self.items.push(PlaceItem {
                label: "Recent".to_string(),
                icon: "document-open-recent".to_string(),
                uri: "recent:///".to_string(),
                place_type: PlaceType::Recent,
                is_ejectable: false,
                free_space: None,
            });
        }
        if self.show_trash {
            self.items.push(PlaceItem {
                label: "Trash".to_string(),
                icon: "user-trash".to_string(),
                uri: "trash:///".to_string(),
                place_type: PlaceType::Trash,
                is_ejectable: false,
                free_space: None,
            });
        }
    }

    /// All items in sidebar order.
    #[must_use]
    pub fn items(&self) -> &[PlaceItem] {
        &self.items
    }

    /// Number of items (including separators).
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the model has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add a mounted device.
    pub fn mount_device(
        &mut self,
        label: &str,
        icon: &str,
        uri: &str,
        ejectable: bool,
        free_space: Option<u64>,
    ) {
        if self.devices.iter().any(|d| d.uri == uri) {
            return;
        }
        self.devices
            .push(PlaceItem::device(label, icon, uri, ejectable, free_space));
        self.refresh();
    }

    /// Remove a mounted device by URI.
    pub fn unmount_device(&mut self, uri: &str) {
        self.devices.retain(|d| d.uri != uri);
        self.refresh();
    }

    /// Eject a device (unmount + mark for safe removal).
    pub fn eject_device(&mut self, uri: &str) -> bool {
        let found = self.devices.iter().any(|d| d.uri == uri && d.is_ejectable);
        if found {
            self.unmount_device(uri);
        }
        found
    }

    /// Add a network share.
    pub fn add_network(&mut self, label: &str, uri: &str) {
        if self.network_shares.iter().any(|n| n.uri == uri) {
            return;
        }
        self.network_shares.push(PlaceItem::network(label, uri));
        self.refresh();
    }

    /// Remove a network share by URI.
    pub fn remove_network(&mut self, uri: &str) {
        self.network_shares.retain(|n| n.uri != uri);
        self.refresh();
    }

    /// Set whether the Trash virtual folder is shown.
    pub fn set_show_trash(&mut self, show: bool) {
        self.show_trash = show;
        self.refresh();
    }

    /// Set whether the Recent virtual folder is shown.
    pub fn set_show_recent(&mut self, show: bool) {
        self.show_recent = show;
        self.refresh();
    }

    /// Number of mounted devices.
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Number of network shares.
    #[must_use]
    pub fn network_count(&self) -> usize {
        self.network_shares.len()
    }

    /// Find a place item by URI.
    #[must_use]
    pub fn find(&self, uri: &str) -> Option<&PlaceItem> {
        self.items.iter().find(|p| p.uri == uri)
    }

    /// Count of non-separator items.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.items.iter().filter(|p| !p.is_separator()).count()
    }
}

impl Default for PlacesModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-independent home directory helper.
fn home_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return home;
    }
    #[cfg(target_os = "windows")]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return profile.replace('\\', "/");
    }
    "/home/user".to_string()
}

/// Detect system devices (drives on Windows, root on Unix).
fn detect_system_devices() -> Vec<PlaceItem> {
    let mut devices = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Probe A-Z drive letters.
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            if std::path::Path::new(&drive).exists() {
                let label = format!("{}: Drive", letter as char);
                let uri = format!("file:///{}", drive.replace('\\', "/"));
                devices.push(PlaceItem::device(
                    &label,
                    "drive-harddisk",
                    &uri,
                    false,
                    None,
                ));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Root filesystem.
        devices.push(PlaceItem::device(
            "Filesystem",
            "drive-harddisk",
            "file:///",
            false,
            None,
        ));
        // Scan /media/$USER and /mnt for mounted volumes.
        if let Ok(user) = std::env::var("USER") {
            let media = format!("/media/{user}");
            if let Ok(rd) = std::fs::read_dir(&media) {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let path = entry.path().to_string_lossy().to_string();
                    devices.push(PlaceItem::device(
                        &name,
                        "drive-removable-media",
                        &format!("file://{path}"),
                        true,
                        None,
                    ));
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Root filesystem.
        devices.push(PlaceItem::device(
            "Macintosh HD",
            "drive-harddisk",
            "file:///",
            false,
            None,
        ));
        // Scan /Volumes for mounted volumes.
        if let Ok(rd) = std::fs::read_dir("/Volumes") {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "Macintosh HD" {
                    continue; // already added above
                }
                let path = entry.path().to_string_lossy().to_string();
                devices.push(PlaceItem::device(
                    &name,
                    "drive-removable-media",
                    &format!("file://{path}"),
                    true,
                    None,
                ));
            }
        }
    }

    devices
}

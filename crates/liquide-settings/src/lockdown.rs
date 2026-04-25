//! Feature lockdown for kiosk, corporate, and education deployments.
//!
//! A lockdown profile disables or restricts specific desktop features.
//! Built-in profiles provide common configurations; custom profiles can be
//! composed by selecting individual feature restrictions.

use std::collections::HashMap;
use std::fmt;

/// A desktop feature that can be restricted by a lockdown profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    ChangeWallpaper,
    InstallApps,
    ModifyNetwork,
    AccessTerminal,
    ChangeDNS,
    AddPrinters,
    USBDevices,
    ExternalMedia,
    ScreenRecording,
    RemoteDesktop,
    DeveloperMode,
}

impl Feature {
    /// Returns all defined features.
    pub fn all() -> &'static [Feature] {
        &[
            Feature::ChangeWallpaper,
            Feature::InstallApps,
            Feature::ModifyNetwork,
            Feature::AccessTerminal,
            Feature::ChangeDNS,
            Feature::AddPrinters,
            Feature::USBDevices,
            Feature::ExternalMedia,
            Feature::ScreenRecording,
            Feature::RemoteDesktop,
            Feature::DeveloperMode,
        ]
    }

    /// Human-readable label for the feature.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ChangeWallpaper => "Change Wallpaper",
            Self::InstallApps => "Install Applications",
            Self::ModifyNetwork => "Modify Network Settings",
            Self::AccessTerminal => "Access Terminal",
            Self::ChangeDNS => "Change DNS Settings",
            Self::AddPrinters => "Add Printers",
            Self::USBDevices => "USB Devices",
            Self::ExternalMedia => "External Media",
            Self::ScreenRecording => "Screen Recording",
            Self::RemoteDesktop => "Remote Desktop",
            Self::DeveloperMode => "Developer Mode",
        }
    }

    /// Machine-readable identifier string.
    pub fn id(&self) -> &'static str {
        match self {
            Self::ChangeWallpaper => "change-wallpaper",
            Self::InstallApps => "install-apps",
            Self::ModifyNetwork => "modify-network",
            Self::AccessTerminal => "access-terminal",
            Self::ChangeDNS => "change-dns",
            Self::AddPrinters => "add-printers",
            Self::USBDevices => "usb-devices",
            Self::ExternalMedia => "external-media",
            Self::ScreenRecording => "screen-recording",
            Self::RemoteDesktop => "remote-desktop",
            Self::DeveloperMode => "developer-mode",
        }
    }

    /// Parse a feature from its string identifier.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "change-wallpaper" => Some(Self::ChangeWallpaper),
            "install-apps" => Some(Self::InstallApps),
            "modify-network" => Some(Self::ModifyNetwork),
            "access-terminal" => Some(Self::AccessTerminal),
            "change-dns" => Some(Self::ChangeDNS),
            "add-printers" => Some(Self::AddPrinters),
            "usb-devices" => Some(Self::USBDevices),
            "external-media" => Some(Self::ExternalMedia),
            "screen-recording" => Some(Self::ScreenRecording),
            "remote-desktop" => Some(Self::RemoteDesktop),
            "developer-mode" => Some(Self::DeveloperMode),
            _ => None,
        }
    }
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A restriction entry for a feature in a lockdown profile.
#[derive(Debug, Clone)]
struct FeatureRestriction {
    /// Whether the feature is disabled (true = blocked).
    disabled: bool,
    /// Optional message shown to the user when the feature is blocked.
    message: Option<String>,
}

/// A named lockdown profile that restricts a set of desktop features.
#[derive(Debug, Clone)]
pub struct LockdownProfile {
    /// Profile name (e.g. "kiosk", "corporate").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Per-feature restrictions.
    restrictions: HashMap<Feature, FeatureRestriction>,
}

impl LockdownProfile {
    /// Create a new empty profile (everything allowed).
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            restrictions: HashMap::new(),
        }
    }

    /// Disable a feature in this profile.
    pub fn disable(&mut self, feature: Feature) {
        self.restrictions.insert(
            feature,
            FeatureRestriction {
                disabled: true,
                message: None,
            },
        );
    }

    /// Disable a feature with a custom restriction message.
    pub fn disable_with_message(&mut self, feature: Feature, message: &str) {
        self.restrictions.insert(
            feature,
            FeatureRestriction {
                disabled: true,
                message: Some(message.to_string()),
            },
        );
    }

    /// Explicitly allow a feature (remove any restriction).
    pub fn allow(&mut self, feature: Feature) {
        self.restrictions.remove(&feature);
    }

    /// Check whether a feature is allowed in this profile.
    pub fn is_allowed(&self, feature: Feature) -> bool {
        match self.restrictions.get(&feature) {
            Some(r) => !r.disabled,
            None => true,
        }
    }

    /// Get the restriction message for a disabled feature, if any.
    pub fn restricted_message(&self, feature: Feature) -> Option<&str> {
        self.restrictions
            .get(&feature)
            .filter(|r| r.disabled)
            .and_then(|r| r.message.as_deref())
    }

    /// Return the list of all disabled features.
    pub fn disabled_features(&self) -> Vec<Feature> {
        self.restrictions
            .iter()
            .filter(|(_, r)| r.disabled)
            .map(|(f, _)| *f)
            .collect()
    }

    /// Return the number of restricted features.
    pub fn restriction_count(&self) -> usize {
        self.restrictions.values().filter(|r| r.disabled).count()
    }

    // ── Built-in profiles ──────────────────────────────────────────────

    /// "kiosk" profile: most features locked down. Only basic interaction
    /// is allowed — no system modification, no peripherals, no dev tools.
    pub fn kiosk() -> Self {
        let mut p = Self::new(
            "kiosk",
            "Kiosk mode — single-purpose display with all system access disabled",
        );
        for feature in Feature::all() {
            p.disable_with_message(*feature, "This feature is disabled in kiosk mode");
        }
        p
    }

    /// "corporate" profile: moderate restrictions. Users can customize
    /// appearance but cannot install software, access terminal, or change
    /// network/DNS settings.
    pub fn corporate() -> Self {
        let mut p = Self::new(
            "corporate",
            "Corporate managed desktop with moderate restrictions",
        );
        p.disable_with_message(
            Feature::InstallApps,
            "Software installation is managed by your IT department",
        );
        p.disable_with_message(
            Feature::AccessTerminal,
            "Terminal access is restricted by organization policy",
        );
        p.disable_with_message(
            Feature::ModifyNetwork,
            "Network settings are managed centrally",
        );
        p.disable_with_message(Feature::ChangeDNS, "DNS settings are managed centrally");
        p.disable_with_message(
            Feature::DeveloperMode,
            "Developer mode is not available on managed devices",
        );
        p.disable_with_message(
            Feature::USBDevices,
            "USB device access requires authorization",
        );
        p
    }

    /// "education" profile: some restrictions. Students cannot install
    /// apps, access developer tools, or record screens, but can change
    /// wallpaper and use peripherals.
    pub fn education() -> Self {
        let mut p = Self::new(
            "education",
            "Education environment with selective restrictions",
        );
        p.disable_with_message(
            Feature::InstallApps,
            "Application installation is managed by the school",
        );
        p.disable_with_message(
            Feature::DeveloperMode,
            "Developer mode is not available on school devices",
        );
        p.disable_with_message(
            Feature::ScreenRecording,
            "Screen recording is disabled on this device",
        );
        p.disable_with_message(Feature::RemoteDesktop, "Remote desktop is not available");
        p
    }

    /// "unrestricted" profile: all features allowed. This is the default
    /// for personal desktops.
    pub fn unrestricted() -> Self {
        Self::new("unrestricted", "No restrictions — all features available")
    }
}

/// Manages feature lockdown by evaluating the active profile.
pub struct LockdownManager {
    /// The currently active profile.
    active_profile: LockdownProfile,
}

impl LockdownManager {
    /// Create a manager with the given profile.
    pub fn new(profile: LockdownProfile) -> Self {
        Self {
            active_profile: profile,
        }
    }

    /// Create a manager with the unrestricted (default) profile.
    pub fn unrestricted() -> Self {
        Self::new(LockdownProfile::unrestricted())
    }

    /// Switch to a different profile.
    pub fn set_profile(&mut self, profile: LockdownProfile) {
        self.active_profile = profile;
    }

    /// Get a reference to the active profile.
    pub fn profile(&self) -> &LockdownProfile {
        &self.active_profile
    }

    /// Check whether a feature is allowed under the current profile.
    pub fn is_allowed(&self, feature: Feature) -> bool {
        self.active_profile.is_allowed(feature)
    }

    /// Get the user-facing restriction message for a blocked feature.
    pub fn restricted_message(&self, feature: Feature) -> Option<&str> {
        self.active_profile.restricted_message(feature)
    }

    /// Return all currently disabled features.
    pub fn disabled_features(&self) -> Vec<Feature> {
        self.active_profile.disabled_features()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_all_returns_11() {
        assert_eq!(Feature::all().len(), 11);
    }

    #[test]
    fn feature_labels() {
        assert_eq!(Feature::ChangeWallpaper.label(), "Change Wallpaper");
        assert_eq!(Feature::DeveloperMode.label(), "Developer Mode");
    }

    #[test]
    fn feature_id_roundtrip() {
        for feature in Feature::all() {
            let id = feature.id();
            let parsed = Feature::from_id(id);
            assert_eq!(parsed, Some(*feature), "roundtrip failed for {:?}", feature);
        }
    }

    #[test]
    fn feature_from_id_invalid() {
        assert_eq!(Feature::from_id("nonexistent"), None);
    }

    #[test]
    fn feature_display() {
        assert_eq!(format!("{}", Feature::InstallApps), "Install Applications");
    }

    #[test]
    fn empty_profile_allows_everything() {
        let profile = LockdownProfile::new("test", "test profile");
        for feature in Feature::all() {
            assert!(
                profile.is_allowed(*feature),
                "{:?} should be allowed",
                feature
            );
        }
    }

    #[test]
    fn disable_feature() {
        let mut profile = LockdownProfile::new("test", "test");
        profile.disable(Feature::InstallApps);
        assert!(!profile.is_allowed(Feature::InstallApps));
        assert!(profile.is_allowed(Feature::ChangeWallpaper));
    }

    #[test]
    fn disable_with_message() {
        let mut profile = LockdownProfile::new("test", "test");
        profile.disable_with_message(Feature::AccessTerminal, "Not available");
        assert!(!profile.is_allowed(Feature::AccessTerminal));
        assert_eq!(
            profile.restricted_message(Feature::AccessTerminal),
            Some("Not available")
        );
    }

    #[test]
    fn allow_re_enables_feature() {
        let mut profile = LockdownProfile::new("test", "test");
        profile.disable(Feature::ScreenRecording);
        assert!(!profile.is_allowed(Feature::ScreenRecording));
        profile.allow(Feature::ScreenRecording);
        assert!(profile.is_allowed(Feature::ScreenRecording));
    }

    #[test]
    fn restricted_message_none_for_allowed() {
        let profile = LockdownProfile::new("test", "test");
        assert_eq!(profile.restricted_message(Feature::ChangeWallpaper), None);
    }

    #[test]
    fn disabled_features_list() {
        let mut profile = LockdownProfile::new("test", "test");
        profile.disable(Feature::InstallApps);
        profile.disable(Feature::DeveloperMode);
        let disabled = profile.disabled_features();
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains(&Feature::InstallApps));
        assert!(disabled.contains(&Feature::DeveloperMode));
    }

    #[test]
    fn restriction_count() {
        let mut profile = LockdownProfile::new("test", "test");
        assert_eq!(profile.restriction_count(), 0);
        profile.disable(Feature::USBDevices);
        profile.disable(Feature::ExternalMedia);
        assert_eq!(profile.restriction_count(), 2);
    }

    // ── Built-in profile tests ─────────────────────────────────────

    #[test]
    fn kiosk_blocks_everything() {
        let profile = LockdownProfile::kiosk();
        assert_eq!(profile.name, "kiosk");
        for feature in Feature::all() {
            assert!(
                !profile.is_allowed(*feature),
                "kiosk should block {:?}",
                feature
            );
            assert!(profile.restricted_message(*feature).is_some());
        }
        assert_eq!(profile.restriction_count(), 11);
    }

    #[test]
    fn corporate_moderate_restrictions() {
        let profile = LockdownProfile::corporate();
        assert_eq!(profile.name, "corporate");
        assert!(!profile.is_allowed(Feature::InstallApps));
        assert!(!profile.is_allowed(Feature::AccessTerminal));
        assert!(!profile.is_allowed(Feature::ModifyNetwork));
        assert!(!profile.is_allowed(Feature::ChangeDNS));
        assert!(!profile.is_allowed(Feature::DeveloperMode));
        assert!(!profile.is_allowed(Feature::USBDevices));
        // Allowed in corporate
        assert!(profile.is_allowed(Feature::ChangeWallpaper));
        assert!(profile.is_allowed(Feature::ScreenRecording));
        assert!(profile.is_allowed(Feature::AddPrinters));
        assert_eq!(profile.restriction_count(), 6);
    }

    #[test]
    fn education_selective_restrictions() {
        let profile = LockdownProfile::education();
        assert_eq!(profile.name, "education");
        assert!(!profile.is_allowed(Feature::InstallApps));
        assert!(!profile.is_allowed(Feature::DeveloperMode));
        assert!(!profile.is_allowed(Feature::ScreenRecording));
        assert!(!profile.is_allowed(Feature::RemoteDesktop));
        // Allowed in education
        assert!(profile.is_allowed(Feature::ChangeWallpaper));
        assert!(profile.is_allowed(Feature::AccessTerminal));
        assert!(profile.is_allowed(Feature::ModifyNetwork));
        assert!(profile.is_allowed(Feature::USBDevices));
        assert_eq!(profile.restriction_count(), 4);
    }

    #[test]
    fn unrestricted_allows_everything() {
        let profile = LockdownProfile::unrestricted();
        assert_eq!(profile.name, "unrestricted");
        for feature in Feature::all() {
            assert!(profile.is_allowed(*feature));
        }
        assert_eq!(profile.restriction_count(), 0);
    }

    // ── LockdownManager tests ──────────────────────────────────────

    #[test]
    fn manager_unrestricted_default() {
        let mgr = LockdownManager::unrestricted();
        assert!(mgr.is_allowed(Feature::InstallApps));
        assert!(mgr.is_allowed(Feature::DeveloperMode));
        assert!(mgr.disabled_features().is_empty());
    }

    #[test]
    fn manager_with_kiosk_profile() {
        let mgr = LockdownManager::new(LockdownProfile::kiosk());
        assert!(!mgr.is_allowed(Feature::InstallApps));
        assert!(mgr.restricted_message(Feature::InstallApps).is_some());
        assert_eq!(mgr.disabled_features().len(), 11);
    }

    #[test]
    fn manager_switch_profile() {
        let mut mgr = LockdownManager::new(LockdownProfile::kiosk());
        assert!(!mgr.is_allowed(Feature::ChangeWallpaper));

        mgr.set_profile(LockdownProfile::unrestricted());
        assert!(mgr.is_allowed(Feature::ChangeWallpaper));
    }

    #[test]
    fn manager_profile_ref() {
        let mgr = LockdownManager::new(LockdownProfile::corporate());
        assert_eq!(mgr.profile().name, "corporate");
    }

    #[test]
    fn manager_restricted_message_none_when_allowed() {
        let mgr = LockdownManager::unrestricted();
        assert_eq!(mgr.restricted_message(Feature::ChangeWallpaper), None);
    }
}

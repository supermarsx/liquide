//! Power policy management.
//!
//! Defines power profiles (Performance, Balanced, PowerSaver, Custom) and
//! the [`PolicyManager`] that transitions between them, applying effective
//! settings for CPU governor, display brightness, timeouts, etc.
//!
//! Modelled after UPower power-profiles-daemon and TLP concepts.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Policy enum
// ---------------------------------------------------------------------------

/// A named power policy that determines system behaviour trade-offs between
/// performance and energy conservation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PowerPolicy {
    /// Maximum performance -- no throttling, display always bright.
    Performance,
    /// Balanced -- moderate timeouts, default brightness.
    Balanced,
    /// Power saver -- aggressive throttling, dim display, short timeouts.
    PowerSaver,
    /// A user-defined profile loaded from configuration.
    Custom(String),
}

impl std::fmt::Display for PowerPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Performance => write!(f, "performance"),
            Self::Balanced => write!(f, "balanced"),
            Self::PowerSaver => write!(f, "power-saver"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

// ---------------------------------------------------------------------------
// CPU governor hint
// ---------------------------------------------------------------------------

/// Hint for the kernel CPU frequency governor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuGovernor {
    Performance,
    Powersave,
    Schedutil,
    Ondemand,
    Conservative,
}

impl std::fmt::Display for CpuGovernor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Performance => "performance",
            Self::Powersave => "powersave",
            Self::Schedutil => "schedutil",
            Self::Ondemand => "ondemand",
            Self::Conservative => "conservative",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// PolicyConfig
// ---------------------------------------------------------------------------

/// Concrete settings produced by a [`PowerPolicy`].
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyConfig {
    /// Desired CPU frequency governor.
    pub cpu_governor: CpuGovernor,
    /// Display brightness (0-100) when on AC power.
    pub display_brightness_ac: u8,
    /// Display brightness (0-100) when on battery.
    pub display_brightness_battery: u8,
    /// How long before the display dims due to inactivity.
    pub dim_timeout: Duration,
    /// How long before the system suspends when on AC power.
    pub suspend_timeout_ac: Duration,
    /// How long before the system suspends when on battery.
    pub suspend_timeout_battery: Duration,
    /// Whether to automatically suspend when on battery and idle.
    pub auto_suspend_on_battery: bool,
}

impl PolicyConfig {
    /// Default configuration for a given built-in policy.
    pub fn for_policy(policy: &PowerPolicy) -> Self {
        match policy {
            PowerPolicy::Performance => Self {
                cpu_governor: CpuGovernor::Performance,
                display_brightness_ac: 100,
                display_brightness_battery: 80,
                dim_timeout: Duration::from_secs(600),
                suspend_timeout_ac: Duration::from_secs(0), // never
                suspend_timeout_battery: Duration::from_secs(1800),
                auto_suspend_on_battery: false,
            },
            PowerPolicy::Balanced => Self {
                cpu_governor: CpuGovernor::Schedutil,
                display_brightness_ac: 80,
                display_brightness_battery: 60,
                dim_timeout: Duration::from_secs(300),
                suspend_timeout_ac: Duration::from_secs(1800),
                suspend_timeout_battery: Duration::from_secs(900),
                auto_suspend_on_battery: true,
            },
            PowerPolicy::PowerSaver => Self {
                cpu_governor: CpuGovernor::Powersave,
                display_brightness_ac: 60,
                display_brightness_battery: 40,
                dim_timeout: Duration::from_secs(120),
                suspend_timeout_ac: Duration::from_secs(900),
                suspend_timeout_battery: Duration::from_secs(300),
                auto_suspend_on_battery: true,
            },
            PowerPolicy::Custom(_) => {
                // Custom profiles start from Balanced defaults; the caller is
                // expected to override individual fields.
                Self::for_policy(&PowerPolicy::Balanced)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Battery threshold triggers
// ---------------------------------------------------------------------------

/// Threshold at which the policy manager automatically switches to
/// [`PowerPolicy::PowerSaver`].
pub const BATTERY_THRESHOLD_POWER_SAVER: u8 = 20;

/// Threshold at which the policy manager emits a low-battery warning.
pub const BATTERY_THRESHOLD_LOW: u8 = 10;

/// Critical battery threshold.
pub const BATTERY_THRESHOLD_CRITICAL: u8 = 5;

// ---------------------------------------------------------------------------
// PolicyManager
// ---------------------------------------------------------------------------

/// Manages the active power policy and transitions between policies.
///
/// When battery level drops below configured thresholds, the manager can
/// automatically switch to a more conservative policy.
pub struct PolicyManager {
    active: PowerPolicy,
    config: PolicyConfig,
    /// The policy the user explicitly chose (restored when AC is plugged in).
    user_policy: PowerPolicy,
    /// Whether the manager forced a policy change due to low battery.
    forced_power_saver: bool,
    /// Custom policy configurations keyed by name.
    custom_configs: Vec<(String, PolicyConfig)>,
}

impl PolicyManager {
    /// Create a new manager with [`PowerPolicy::Balanced`] as default.
    pub fn new() -> Self {
        let policy = PowerPolicy::Balanced;
        let config = PolicyConfig::for_policy(&policy);
        Self {
            active: policy.clone(),
            config,
            user_policy: policy,
            forced_power_saver: false,
            custom_configs: Vec::new(),
        }
    }

    /// Returns the currently active policy.
    pub fn active_policy(&self) -> &PowerPolicy {
        &self.active
    }

    /// Returns the effective configuration for the active policy.
    pub fn active_config(&self) -> &PolicyConfig {
        &self.config
    }

    /// Returns whether the manager forced PowerSaver due to low battery.
    pub fn is_forced_power_saver(&self) -> bool {
        self.forced_power_saver
    }

    /// Explicitly set a new policy. This also updates `user_policy` so that
    /// it can be restored after battery recovery.
    pub fn set_policy(&mut self, policy: PowerPolicy) {
        self.config = self.resolve_config(&policy);
        self.active = policy.clone();
        self.user_policy = policy;
        self.forced_power_saver = false;
    }

    /// Register a custom policy configuration.
    pub fn register_custom(&mut self, name: String, config: PolicyConfig) {
        // Replace if already exists.
        if let Some(pos) = self.custom_configs.iter().position(|(n, _)| *n == name) {
            self.custom_configs[pos].1 = config;
        } else {
            self.custom_configs.push((name, config));
        }
    }

    /// Apply battery-level thresholds. Returns the policy that should be
    /// active after considering the current battery level. The manager will
    /// automatically switch to PowerSaver when below the threshold and
    /// restore the user's choice when battery recovers.
    pub fn apply_battery_threshold(&mut self, battery_percent: u8, is_charging: bool) -> &PowerPolicy {
        if is_charging {
            // Restore user policy when charging.
            if self.forced_power_saver {
                self.forced_power_saver = false;
                self.active = self.user_policy.clone();
                self.config = self.resolve_config(&self.active);
            }
        } else if battery_percent <= BATTERY_THRESHOLD_POWER_SAVER
            && self.active != PowerPolicy::PowerSaver
        {
            self.forced_power_saver = true;
            self.active = PowerPolicy::PowerSaver;
            self.config = PolicyConfig::for_policy(&PowerPolicy::PowerSaver);
        }
        &self.active
    }

    /// Compute effective settings for a policy, applying `apply_policy` logic.
    pub fn apply_policy(policy: &PowerPolicy) -> PolicyConfig {
        PolicyConfig::for_policy(policy)
    }

    // Internal: resolve config, checking custom registry first.
    fn resolve_config(&self, policy: &PowerPolicy) -> PolicyConfig {
        if let PowerPolicy::Custom(name) = policy {
            if let Some((_, cfg)) = self.custom_configs.iter().find(|(n, _)| n == name) {
                return cfg.clone();
            }
        }
        PolicyConfig::for_policy(policy)
    }
}

impl Default for PolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_balanced() {
        let pm = PolicyManager::new();
        assert_eq!(*pm.active_policy(), PowerPolicy::Balanced);
    }

    #[test]
    fn set_policy_updates_active() {
        let mut pm = PolicyManager::new();
        pm.set_policy(PowerPolicy::Performance);
        assert_eq!(*pm.active_policy(), PowerPolicy::Performance);
        assert_eq!(pm.active_config().cpu_governor, CpuGovernor::Performance);
    }

    #[test]
    fn power_saver_config() {
        let cfg = PolicyConfig::for_policy(&PowerPolicy::PowerSaver);
        assert_eq!(cfg.cpu_governor, CpuGovernor::Powersave);
        assert_eq!(cfg.display_brightness_battery, 40);
        assert!(cfg.auto_suspend_on_battery);
    }

    #[test]
    fn performance_config() {
        let cfg = PolicyConfig::for_policy(&PowerPolicy::Performance);
        assert_eq!(cfg.cpu_governor, CpuGovernor::Performance);
        assert_eq!(cfg.display_brightness_ac, 100);
        assert!(!cfg.auto_suspend_on_battery);
        // Never suspend on AC in performance mode.
        assert_eq!(cfg.suspend_timeout_ac, Duration::from_secs(0));
    }

    #[test]
    fn battery_threshold_forces_power_saver() {
        let mut pm = PolicyManager::new();
        pm.set_policy(PowerPolicy::Performance);
        pm.apply_battery_threshold(15, false);
        assert_eq!(*pm.active_policy(), PowerPolicy::PowerSaver);
        assert!(pm.is_forced_power_saver());
    }

    #[test]
    fn battery_threshold_restores_on_charge() {
        let mut pm = PolicyManager::new();
        pm.set_policy(PowerPolicy::Performance);
        pm.apply_battery_threshold(10, false);
        assert_eq!(*pm.active_policy(), PowerPolicy::PowerSaver);

        // Plugging in should restore the user's policy.
        pm.apply_battery_threshold(10, true);
        assert_eq!(*pm.active_policy(), PowerPolicy::Performance);
        assert!(!pm.is_forced_power_saver());
    }

    #[test]
    fn battery_above_threshold_no_change() {
        let mut pm = PolicyManager::new();
        pm.set_policy(PowerPolicy::Performance);
        pm.apply_battery_threshold(50, false);
        assert_eq!(*pm.active_policy(), PowerPolicy::Performance);
        assert!(!pm.is_forced_power_saver());
    }

    #[test]
    fn custom_policy_registered() {
        let mut pm = PolicyManager::new();
        let custom_cfg = PolicyConfig {
            cpu_governor: CpuGovernor::Conservative,
            display_brightness_ac: 70,
            display_brightness_battery: 50,
            dim_timeout: Duration::from_secs(180),
            suspend_timeout_ac: Duration::from_secs(600),
            suspend_timeout_battery: Duration::from_secs(300),
            auto_suspend_on_battery: true,
        };
        pm.register_custom("office".into(), custom_cfg.clone());
        pm.set_policy(PowerPolicy::Custom("office".into()));
        assert_eq!(pm.active_config().cpu_governor, CpuGovernor::Conservative);
        assert_eq!(pm.active_config().display_brightness_ac, 70);
    }

    #[test]
    fn custom_policy_unknown_falls_back_to_balanced() {
        let pm = PolicyManager::new();
        let cfg = pm.resolve_config(&PowerPolicy::Custom("unknown".into()));
        assert_eq!(cfg.cpu_governor, CpuGovernor::Schedutil);
    }

    #[test]
    fn register_custom_replaces_existing() {
        let mut pm = PolicyManager::new();
        let cfg1 = PolicyConfig::for_policy(&PowerPolicy::Performance);
        let cfg2 = PolicyConfig::for_policy(&PowerPolicy::PowerSaver);
        pm.register_custom("test".into(), cfg1);
        pm.register_custom("test".into(), cfg2.clone());
        pm.set_policy(PowerPolicy::Custom("test".into()));
        assert_eq!(pm.active_config().cpu_governor, cfg2.cpu_governor);
    }

    #[test]
    fn policy_display() {
        assert_eq!(PowerPolicy::Performance.to_string(), "performance");
        assert_eq!(PowerPolicy::Balanced.to_string(), "balanced");
        assert_eq!(PowerPolicy::PowerSaver.to_string(), "power-saver");
        assert_eq!(
            PowerPolicy::Custom("gaming".into()).to_string(),
            "custom:gaming"
        );
    }

    #[test]
    fn cpu_governor_display() {
        assert_eq!(CpuGovernor::Performance.to_string(), "performance");
        assert_eq!(CpuGovernor::Powersave.to_string(), "powersave");
        assert_eq!(CpuGovernor::Schedutil.to_string(), "schedutil");
        assert_eq!(CpuGovernor::Ondemand.to_string(), "ondemand");
        assert_eq!(CpuGovernor::Conservative.to_string(), "conservative");
    }

    #[test]
    fn apply_policy_static() {
        let cfg = PolicyManager::apply_policy(&PowerPolicy::Balanced);
        assert_eq!(cfg.cpu_governor, CpuGovernor::Schedutil);
    }

    #[test]
    fn balanced_config_values() {
        let cfg = PolicyConfig::for_policy(&PowerPolicy::Balanced);
        assert_eq!(cfg.display_brightness_ac, 80);
        assert_eq!(cfg.display_brightness_battery, 60);
        assert_eq!(cfg.dim_timeout, Duration::from_secs(300));
        assert_eq!(cfg.suspend_timeout_ac, Duration::from_secs(1800));
        assert_eq!(cfg.suspend_timeout_battery, Duration::from_secs(900));
        assert!(cfg.auto_suspend_on_battery);
    }
}

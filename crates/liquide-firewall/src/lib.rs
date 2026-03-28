mod platform;

use std::fmt;

// ---------------------------------------------------------------------------
// Core enums
// ---------------------------------------------------------------------------

/// Traffic direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Inbound,
    Outbound,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inbound => write!(f, "Inbound"),
            Self::Outbound => write!(f, "Outbound"),
        }
    }
}

/// What to do when a rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleAction {
    Allow,
    Block,
    Log,
}

impl fmt::Display for RuleAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "Allow"),
            Self::Block => write!(f, "Block"),
            Self::Log => write!(f, "Log"),
        }
    }
}

/// Network protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    TCP,
    UDP,
    ICMP,
    Any,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TCP => write!(f, "TCP"),
            Self::UDP => write!(f, "UDP"),
            Self::ICMP => write!(f, "ICMP"),
            Self::Any => write!(f, "Any"),
        }
    }
}

/// Port specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PortSpec {
    Single(u16),
    Range(u16, u16),
    Any,
}

impl PortSpec {
    /// Returns `true` if the given port matches this spec.
    pub fn matches(&self, port: u16) -> bool {
        match self {
            Self::Single(p) => port == *p,
            Self::Range(lo, hi) => port >= *lo && port <= *hi,
            Self::Any => true,
        }
    }
}

impl fmt::Display for PortSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(p) => write!(f, "{p}"),
            Self::Range(lo, hi) => write!(f, "{lo}-{hi}"),
            Self::Any => write!(f, "*"),
        }
    }
}

/// Remote address specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AddressSpec {
    /// Exact IP (v4 or v6) as string.
    Single(String),
    /// CIDR subnet: base address + prefix length.
    Subnet(String, u8),
    /// Match any address.
    Any,
}

impl AddressSpec {
    /// Returns `true` if `addr` matches this spec.
    ///
    /// For `Single`, an exact string comparison is performed.
    /// For `Subnet`, the base address and the candidate are parsed as IPv4
    /// dotted-quad and compared using the prefix length as a bit mask.
    /// Non-parseable addresses fall back to string-prefix comparison.
    pub fn matches(&self, addr: &str) -> bool {
        match self {
            Self::Single(a) => a == addr,
            Self::Subnet(base, prefix_len) => subnet_contains(base, *prefix_len, addr),
            Self::Any => true,
        }
    }
}

impl fmt::Display for AddressSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(a) => write!(f, "{a}"),
            Self::Subnet(a, p) => write!(f, "{a}/{p}"),
            Self::Any => write!(f, "*"),
        }
    }
}

/// Parse a dotted-quad IPv4 address into a `u32`.
fn parse_ipv4(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let a: u32 = parts[0].parse().ok()?;
    let b: u32 = parts[1].parse().ok()?;
    let c: u32 = parts[2].parse().ok()?;
    let d: u32 = parts[3].parse().ok()?;
    if a > 255 || b > 255 || c > 255 || d > 255 {
        return None;
    }
    Some((a << 24) | (b << 16) | (c << 8) | d)
}

/// Check whether `addr` falls within `base/prefix_len`.
fn subnet_contains(base: &str, prefix_len: u8, addr: &str) -> bool {
    if prefix_len == 0 {
        return true;
    }
    if let (Some(base_ip), Some(addr_ip)) = (parse_ipv4(base), parse_ipv4(addr)) {
        let mask = if prefix_len >= 32 {
            0xFFFF_FFFFu32
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        };
        return (base_ip & mask) == (addr_ip & mask);
    }
    // Fallback: string-prefix comparison (coarse but safe for non-IPv4)
    addr.starts_with(base)
}

// ---------------------------------------------------------------------------
// FirewallRule
// ---------------------------------------------------------------------------

/// A single firewall rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallRule {
    pub id: u32,
    pub name: String,
    pub enabled: bool,
    pub direction: Direction,
    pub action: RuleAction,
    pub protocol: Protocol,
    pub port: PortSpec,
    pub remote_address: AddressSpec,
    /// Restrict the rule to a specific application path (optional).
    pub application: Option<String>,
    /// Lower value = higher priority.
    pub priority: u32,
}

impl FirewallRule {
    /// Returns `true` if this rule matches the given traffic parameters.
    pub fn matches(
        &self,
        direction: Direction,
        protocol: Protocol,
        port: u16,
        address: &str,
        app: Option<&str>,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        if self.direction != direction {
            return false;
        }
        // Protocol: Any matches everything, else exact.
        if self.protocol != Protocol::Any && self.protocol != protocol {
            return false;
        }
        if !self.port.matches(port) {
            return false;
        }
        if !self.remote_address.matches(address) {
            return false;
        }
        // Application filter: if the rule specifies an app, require a match.
        if let Some(ref rule_app) = self.application {
            match app {
                Some(a) => {
                    if !paths_equal(rule_app, a) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

/// Case-insensitive, separator-normalised path comparison.
fn paths_equal(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").to_lowercase();
    norm(a) == norm(b)
}

// ---------------------------------------------------------------------------
// FirewallProfile
// ---------------------------------------------------------------------------

/// Named collection of rules with default actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallProfile {
    pub id: String,
    pub name: String,
    pub rules: Vec<FirewallRule>,
    pub default_inbound: RuleAction,
    pub default_outbound: RuleAction,
}

impl FirewallProfile {
    /// Permissive profile suitable for trusted home networks.
    pub fn home() -> Self {
        Self {
            id: "home".into(),
            name: "Home".into(),
            rules: vec![
                FirewallRule {
                    id: 1,
                    name: "Allow all outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 100,
                },
                FirewallRule {
                    id: 2,
                    name: "Allow LAN inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Subnet("192.168.0.0".into(), 16),
                    application: None,
                    priority: 100,
                },
                FirewallRule {
                    id: 3,
                    name: "Allow ICMP inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::ICMP,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 200,
                },
            ],
            default_inbound: RuleAction::Allow,
            default_outbound: RuleAction::Allow,
        }
    }

    /// Restrictive profile for untrusted public networks.
    pub fn public() -> Self {
        Self {
            id: "public".into(),
            name: "Public".into(),
            rules: vec![
                FirewallRule {
                    id: 1,
                    name: "Allow DNS outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::UDP,
                    port: PortSpec::Single(53),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 10,
                },
                FirewallRule {
                    id: 2,
                    name: "Allow HTTPS outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(443),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 20,
                },
                FirewallRule {
                    id: 3,
                    name: "Allow HTTP outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(80),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 30,
                },
                FirewallRule {
                    id: 4,
                    name: "Block all inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Block,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 1000,
                },
                FirewallRule {
                    id: 5,
                    name: "Log unknown outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Log,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 999,
                },
            ],
            default_inbound: RuleAction::Block,
            default_outbound: RuleAction::Block,
        }
    }

    /// Moderate profile suitable for managed work environments.
    pub fn work() -> Self {
        Self {
            id: "work".into(),
            name: "Work".into(),
            rules: vec![
                FirewallRule {
                    id: 1,
                    name: "Allow all outbound".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 100,
                },
                FirewallRule {
                    id: 2,
                    name: "Allow corporate subnet inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Subnet("10.0.0.0".into(), 8),
                    application: None,
                    priority: 50,
                },
                FirewallRule {
                    id: 3,
                    name: "Allow SSH inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(22),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 60,
                },
                FirewallRule {
                    id: 4,
                    name: "Allow RDP inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(3389),
                    remote_address: AddressSpec::Subnet("10.0.0.0".into(), 8),
                    application: None,
                    priority: 70,
                },
                FirewallRule {
                    id: 5,
                    name: "Log blocked inbound".into(),
                    enabled: true,
                    direction: Direction::Inbound,
                    action: RuleAction::Log,
                    protocol: Protocol::Any,
                    port: PortSpec::Any,
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 999,
                },
            ],
            default_inbound: RuleAction::Block,
            default_outbound: RuleAction::Allow,
        }
    }

    /// Returns the next available rule id (max existing + 1, or 1).
    fn next_rule_id(&self) -> u32 {
        self.rules.iter().map(|r| r.id).max().unwrap_or(0) + 1
    }
}

// ---------------------------------------------------------------------------
// ConnectionEvent
// ---------------------------------------------------------------------------

/// Record of a single evaluated connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEvent {
    /// Monotonic microsecond timestamp (caller-supplied or defaulting to 0).
    pub timestamp: u64,
    pub direction: Direction,
    pub protocol: Protocol,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub action: RuleAction,
    /// The id of the matching rule, or `None` if the default was used.
    pub rule_id: Option<u32>,
    /// Application that originated/received the traffic.
    pub app: Option<String>,
}

// ---------------------------------------------------------------------------
// FirewallError
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirewallError {
    ProfileNotFound,
    RuleNotFound,
    DuplicateRuleId,
    NotSupported,
    PermissionDenied,
    PlatformError(String),
}

impl fmt::Display for FirewallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileNotFound => write!(f, "profile not found"),
            Self::RuleNotFound => write!(f, "rule not found"),
            Self::DuplicateRuleId => write!(f, "duplicate rule id"),
            Self::NotSupported => write!(f, "not supported"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::PlatformError(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for FirewallError {}

// ---------------------------------------------------------------------------
// FirewallManager
// ---------------------------------------------------------------------------

/// Central firewall management object.
///
/// Holds a set of named profiles, an active profile, and a connection log.
pub struct FirewallManager {
    profiles: Vec<FirewallProfile>,
    active_profile_id: String,
    /// Bounded connection log — oldest entries are evicted when the capacity
    /// is exceeded.
    pub connection_log: Vec<ConnectionEvent>,
    log_capacity: usize,
}

impl FirewallManager {
    /// Maximum number of connection-log entries kept by default.
    const DEFAULT_LOG_CAPACITY: usize = 4096;

    /// Create a new `FirewallManager` pre-loaded with the three built-in
    /// profiles (`home`, `public`, `work`).  The `home` profile is active by
    /// default.
    pub fn new() -> Self {
        Self {
            profiles: vec![
                FirewallProfile::home(),
                FirewallProfile::public(),
                FirewallProfile::work(),
            ],
            active_profile_id: "home".into(),
            connection_log: Vec::new(),
            log_capacity: Self::DEFAULT_LOG_CAPACITY,
        }
    }

    /// Set the maximum number of connection-log entries.
    pub fn set_log_capacity(&mut self, cap: usize) {
        self.log_capacity = cap;
        if self.connection_log.len() > cap {
            let excess = self.connection_log.len() - cap;
            self.connection_log.drain(..excess);
        }
    }

    /// Return the log capacity.
    pub fn log_capacity(&self) -> usize {
        self.log_capacity
    }

    // -- profile accessors --------------------------------------------------

    /// Reference to the currently active profile.
    pub fn active_profile(&self) -> &FirewallProfile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
            .expect("active profile must exist")
    }

    /// Mutable reference to the currently active profile.
    fn active_profile_mut(&mut self) -> &mut FirewallProfile {
        let id = self.active_profile_id.clone();
        self.profiles
            .iter_mut()
            .find(|p| p.id == id)
            .expect("active profile must exist")
    }

    /// List all profile ids.
    pub fn profile_ids(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.id.as_str()).collect()
    }

    /// Switch to a different profile by id.
    pub fn set_profile(&mut self, id: &str) -> Result<(), FirewallError> {
        if !self.profiles.iter().any(|p| p.id == id) {
            return Err(FirewallError::ProfileNotFound);
        }
        self.active_profile_id = id.into();
        Ok(())
    }

    /// Add a custom profile.  Returns an error if a profile with the same id
    /// already exists.
    pub fn add_profile(&mut self, profile: FirewallProfile) -> Result<(), FirewallError> {
        if self.profiles.iter().any(|p| p.id == profile.id) {
            return Err(FirewallError::DuplicateRuleId);
        }
        self.profiles.push(profile);
        Ok(())
    }

    /// Remove a profile by id.  The active profile cannot be removed.
    pub fn remove_profile(&mut self, id: &str) -> Result<(), FirewallError> {
        if id == self.active_profile_id {
            return Err(FirewallError::ProfileNotFound);
        }
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        if self.profiles.len() == before {
            return Err(FirewallError::ProfileNotFound);
        }
        Ok(())
    }

    // -- rule management (operates on the active profile) -------------------

    /// Add a rule to the active profile.  If `rule.id` is 0 it will be
    /// auto-assigned.
    pub fn add_rule(&mut self, mut rule: FirewallRule) -> Result<(), FirewallError> {
        let profile = self.active_profile_mut();
        if rule.id == 0 {
            rule.id = profile.next_rule_id();
        } else if profile.rules.iter().any(|r| r.id == rule.id) {
            return Err(FirewallError::DuplicateRuleId);
        }
        profile.rules.push(rule);
        Ok(())
    }

    /// Remove a rule from the active profile by id.
    pub fn remove_rule(&mut self, id: u32) -> Result<(), FirewallError> {
        let profile = self.active_profile_mut();
        let before = profile.rules.len();
        profile.rules.retain(|r| r.id != id);
        if profile.rules.len() == before {
            Err(FirewallError::RuleNotFound)
        } else {
            Ok(())
        }
    }

    /// Enable a rule.
    pub fn enable_rule(&mut self, id: u32) -> Result<(), FirewallError> {
        let profile = self.active_profile_mut();
        let rule = profile
            .rules
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(FirewallError::RuleNotFound)?;
        rule.enabled = true;
        Ok(())
    }

    /// Disable a rule.
    pub fn disable_rule(&mut self, id: u32) -> Result<(), FirewallError> {
        let profile = self.active_profile_mut();
        let rule = profile
            .rules
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(FirewallError::RuleNotFound)?;
        rule.enabled = false;
        Ok(())
    }

    /// Retrieve a rule by id from the active profile.
    pub fn get_rule(&self, id: u32) -> Option<&FirewallRule> {
        self.active_profile().rules.iter().find(|r| r.id == id)
    }

    // -- evaluation ---------------------------------------------------------

    /// Evaluate traffic against the active profile's rules, returning the
    /// action of the highest-priority (lowest `priority` value) matching rule.
    /// Falls back to the profile's default action for the given direction.
    ///
    /// A `ConnectionEvent` is recorded in the log.
    pub fn evaluate(
        &mut self,
        direction: Direction,
        protocol: Protocol,
        port: u16,
        address: &str,
        app: Option<&str>,
    ) -> RuleAction {
        self.evaluate_at(0, direction, protocol, port, address, app)
    }

    /// Like [`evaluate`](Self::evaluate) but with an explicit timestamp.
    pub fn evaluate_at(
        &mut self,
        timestamp: u64,
        direction: Direction,
        protocol: Protocol,
        port: u16,
        address: &str,
        app: Option<&str>,
    ) -> RuleAction {
        let profile = self
            .profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
            .expect("active profile must exist");

        // Collect matching rules, sort by priority (ascending = higher prio).
        let mut matches: Vec<&FirewallRule> = profile
            .rules
            .iter()
            .filter(|r| r.matches(direction, protocol, port, address, app))
            .collect();
        matches.sort_by_key(|r| r.priority);

        let (action, rule_id) = if let Some(best) = matches.first() {
            (best.action, Some(best.id))
        } else {
            let default = match direction {
                Direction::Inbound => profile.default_inbound,
                Direction::Outbound => profile.default_outbound,
            };
            (default, None)
        };

        // Log the event.
        let event = ConnectionEvent {
            timestamp,
            direction,
            protocol,
            local_port: port,
            remote_addr: address.to_string(),
            remote_port: port,
            action,
            rule_id,
            app: app.map(|s| s.to_string()),
        };
        self.connection_log.push(event);
        if self.connection_log.len() > self.log_capacity {
            self.connection_log.remove(0);
        }

        action
    }

    /// Clear the connection log.
    pub fn clear_log(&mut self) {
        self.connection_log.clear();
    }
}

impl Default for FirewallManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Platform bridge trait
// ---------------------------------------------------------------------------

/// Trait for applying firewall rules to the operating system's native
/// firewall.  Each platform module provides an implementation.
pub trait FirewallBackend: Send {
    /// Apply (sync) all rules from the profile to the OS firewall.
    fn apply_profile(&mut self, profile: &FirewallProfile) -> Result<(), FirewallError>;

    /// Add a single rule to the OS firewall.
    fn add_rule(&mut self, rule: &FirewallRule) -> Result<(), FirewallError>;

    /// Remove a single rule from the OS firewall by name.
    fn remove_rule(&mut self, rule_name: &str) -> Result<(), FirewallError>;

    /// List rule names currently present in the OS firewall.
    fn list_rules(&self) -> Result<Vec<String>, FirewallError>;

    /// Check whether the OS firewall is enabled.
    fn is_enabled(&self) -> Result<bool, FirewallError>;

    /// Enable or disable the OS firewall.
    fn set_enabled(&mut self, enabled: bool) -> Result<(), FirewallError>;
}

pub use platform::PlatformFirewall;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PortSpec -----------------------------------------------------------

    #[test]
    fn port_spec_single_match() {
        let spec = PortSpec::Single(443);
        assert!(spec.matches(443));
        assert!(!spec.matches(80));
    }

    #[test]
    fn port_spec_range_match() {
        let spec = PortSpec::Range(8000, 9000);
        assert!(spec.matches(8000));
        assert!(spec.matches(8500));
        assert!(spec.matches(9000));
        assert!(!spec.matches(7999));
        assert!(!spec.matches(9001));
    }

    #[test]
    fn port_spec_any_match() {
        let spec = PortSpec::Any;
        assert!(spec.matches(0));
        assert!(spec.matches(65535));
    }

    // -- AddressSpec --------------------------------------------------------

    #[test]
    fn address_single_match() {
        let spec = AddressSpec::Single("10.0.0.1".into());
        assert!(spec.matches("10.0.0.1"));
        assert!(!spec.matches("10.0.0.2"));
    }

    #[test]
    fn address_subnet_match_24() {
        let spec = AddressSpec::Subnet("192.168.1.0".into(), 24);
        assert!(spec.matches("192.168.1.1"));
        assert!(spec.matches("192.168.1.254"));
        assert!(!spec.matches("192.168.2.1"));
    }

    #[test]
    fn address_subnet_match_16() {
        let spec = AddressSpec::Subnet("10.10.0.0".into(), 16);
        assert!(spec.matches("10.10.0.1"));
        assert!(spec.matches("10.10.255.255"));
        assert!(!spec.matches("10.11.0.1"));
    }

    #[test]
    fn address_subnet_match_8() {
        let spec = AddressSpec::Subnet("10.0.0.0".into(), 8);
        assert!(spec.matches("10.0.0.1"));
        assert!(spec.matches("10.255.255.255"));
        assert!(!spec.matches("11.0.0.1"));
    }

    #[test]
    fn address_subnet_match_32() {
        let spec = AddressSpec::Subnet("10.0.0.5".into(), 32);
        assert!(spec.matches("10.0.0.5"));
        assert!(!spec.matches("10.0.0.6"));
    }

    #[test]
    fn address_subnet_match_0() {
        let spec = AddressSpec::Subnet("0.0.0.0".into(), 0);
        assert!(spec.matches("1.2.3.4"));
        assert!(spec.matches("255.255.255.255"));
    }

    #[test]
    fn address_any_match() {
        let spec = AddressSpec::Any;
        assert!(spec.matches("anything"));
        assert!(spec.matches(""));
    }

    // -- FirewallRule matching ----------------------------------------------

    #[test]
    fn rule_matches_basic() {
        let rule = FirewallRule {
            id: 1,
            name: "Allow HTTPS out".into(),
            enabled: true,
            direction: Direction::Outbound,
            action: RuleAction::Allow,
            protocol: Protocol::TCP,
            port: PortSpec::Single(443),
            remote_address: AddressSpec::Any,
            application: None,
            priority: 10,
        };
        assert!(rule.matches(Direction::Outbound, Protocol::TCP, 443, "1.2.3.4", None));
        assert!(!rule.matches(Direction::Inbound, Protocol::TCP, 443, "1.2.3.4", None));
        assert!(!rule.matches(Direction::Outbound, Protocol::UDP, 443, "1.2.3.4", None));
        assert!(!rule.matches(Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None));
    }

    #[test]
    fn rule_disabled_does_not_match() {
        let rule = FirewallRule {
            id: 1,
            name: "Disabled".into(),
            enabled: false,
            direction: Direction::Outbound,
            action: RuleAction::Allow,
            protocol: Protocol::Any,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: None,
            priority: 1,
        };
        assert!(!rule.matches(Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None));
    }

    #[test]
    fn rule_protocol_any_matches_all() {
        let rule = FirewallRule {
            id: 1,
            name: "Any proto".into(),
            enabled: true,
            direction: Direction::Inbound,
            action: RuleAction::Block,
            protocol: Protocol::Any,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: None,
            priority: 1,
        };
        assert!(rule.matches(Direction::Inbound, Protocol::TCP, 80, "x", None));
        assert!(rule.matches(Direction::Inbound, Protocol::UDP, 53, "y", None));
        assert!(rule.matches(Direction::Inbound, Protocol::ICMP, 0, "z", None));
    }

    #[test]
    fn rule_application_filter() {
        let rule = FirewallRule {
            id: 1,
            name: "Chrome only".into(),
            enabled: true,
            direction: Direction::Outbound,
            action: RuleAction::Allow,
            protocol: Protocol::TCP,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: Some("/usr/bin/chrome".into()),
            priority: 1,
        };
        assert!(rule.matches(
            Direction::Outbound,
            Protocol::TCP,
            443,
            "1.2.3.4",
            Some("/usr/bin/chrome"),
        ));
        assert!(!rule.matches(
            Direction::Outbound,
            Protocol::TCP,
            443,
            "1.2.3.4",
            Some("/usr/bin/firefox"),
        ));
        // No app provided — should not match an app-specific rule.
        assert!(!rule.matches(Direction::Outbound, Protocol::TCP, 443, "1.2.3.4", None));
    }

    #[test]
    fn rule_application_filter_case_insensitive() {
        let rule = FirewallRule {
            id: 1,
            name: "App".into(),
            enabled: true,
            direction: Direction::Outbound,
            action: RuleAction::Allow,
            protocol: Protocol::Any,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: Some("C:\\Program Files\\App\\app.exe".into()),
            priority: 1,
        };
        assert!(rule.matches(
            Direction::Outbound,
            Protocol::TCP,
            443,
            "1.2.3.4",
            Some("c:/program files/app/app.exe"),
        ));
    }

    // -- FirewallProfile presets ---------------------------------------------

    #[test]
    fn home_profile_allows_lan_inbound() {
        let p = FirewallProfile::home();
        assert_eq!(p.id, "home");
        assert_eq!(p.default_inbound, RuleAction::Allow);
        assert_eq!(p.default_outbound, RuleAction::Allow);
        assert!(p.rules.iter().any(|r| r.name.contains("LAN")));
    }

    #[test]
    fn public_profile_blocks_inbound() {
        let p = FirewallProfile::public();
        assert_eq!(p.default_inbound, RuleAction::Block);
        assert_eq!(p.default_outbound, RuleAction::Block);
        // Has explicit allow for DNS and HTTPS outbound.
        assert!(p.rules.iter().any(|r| r.name.contains("DNS")));
        assert!(p.rules.iter().any(|r| r.name.contains("HTTPS")));
    }

    #[test]
    fn work_profile_allows_ssh_inbound() {
        let p = FirewallProfile::work();
        assert_eq!(p.default_inbound, RuleAction::Block);
        assert_eq!(p.default_outbound, RuleAction::Allow);
        assert!(p.rules.iter().any(|r| r.name.contains("SSH")));
        assert!(p.rules.iter().any(|r| r.name.contains("RDP")));
    }

    // -- FirewallManager ----------------------------------------------------

    #[test]
    fn manager_default_profile_is_home() {
        let mgr = FirewallManager::new();
        assert_eq!(mgr.active_profile().id, "home");
    }

    #[test]
    fn manager_switch_profile() {
        let mut mgr = FirewallManager::new();
        assert!(mgr.set_profile("public").is_ok());
        assert_eq!(mgr.active_profile().id, "public");
        assert!(mgr.set_profile("nonexistent").is_err());
    }

    #[test]
    fn manager_add_remove_rule() {
        let mut mgr = FirewallManager::new();
        let rule = FirewallRule {
            id: 0, // auto-assign
            name: "Custom".into(),
            enabled: true,
            direction: Direction::Inbound,
            action: RuleAction::Block,
            protocol: Protocol::TCP,
            port: PortSpec::Single(9999),
            remote_address: AddressSpec::Any,
            application: None,
            priority: 1,
        };
        mgr.add_rule(rule).unwrap();
        let assigned_id = mgr
            .active_profile()
            .rules
            .last()
            .unwrap()
            .id;
        assert!(assigned_id > 0);
        assert!(mgr.get_rule(assigned_id).is_some());
        assert!(mgr.remove_rule(assigned_id).is_ok());
        assert!(mgr.get_rule(assigned_id).is_none());
    }

    #[test]
    fn manager_duplicate_rule_id() {
        let mut mgr = FirewallManager::new();
        let rule = FirewallRule {
            id: 1, // home profile already has id=1
            name: "Dup".into(),
            enabled: true,
            direction: Direction::Inbound,
            action: RuleAction::Block,
            protocol: Protocol::Any,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: None,
            priority: 1,
        };
        assert_eq!(mgr.add_rule(rule), Err(FirewallError::DuplicateRuleId));
    }

    #[test]
    fn manager_enable_disable_rule() {
        let mut mgr = FirewallManager::new();
        mgr.disable_rule(1).unwrap();
        assert!(!mgr.get_rule(1).unwrap().enabled);
        mgr.enable_rule(1).unwrap();
        assert!(mgr.get_rule(1).unwrap().enabled);
        assert_eq!(mgr.enable_rule(9999), Err(FirewallError::RuleNotFound));
        assert_eq!(mgr.disable_rule(9999), Err(FirewallError::RuleNotFound));
    }

    #[test]
    fn manager_remove_nonexistent_rule() {
        let mut mgr = FirewallManager::new();
        assert_eq!(mgr.remove_rule(9999), Err(FirewallError::RuleNotFound));
    }

    // -- evaluate -----------------------------------------------------------

    #[test]
    fn evaluate_matches_highest_priority() {
        let mut mgr = FirewallManager::new();
        mgr.set_profile("public").unwrap();
        // Public profile allows DNS (UDP 53, prio 10) and HTTPS (TCP 443, prio 20).
        let action = mgr.evaluate(Direction::Outbound, Protocol::TCP, 443, "1.1.1.1", None);
        assert_eq!(action, RuleAction::Allow);
    }

    #[test]
    fn evaluate_falls_back_to_default() {
        let mut mgr = FirewallManager::new();
        mgr.set_profile("public").unwrap();
        // No rule for inbound ICMP on public — but "Block all inbound" at prio
        // 1000 does match.  The default would also be Block.
        let action = mgr.evaluate(Direction::Inbound, Protocol::ICMP, 0, "8.8.8.8", None);
        assert_eq!(action, RuleAction::Block);
    }

    #[test]
    fn evaluate_home_allows_outbound() {
        let mut mgr = FirewallManager::new();
        let action = mgr.evaluate(Direction::Outbound, Protocol::TCP, 443, "1.1.1.1", None);
        assert_eq!(action, RuleAction::Allow);
    }

    #[test]
    fn evaluate_home_allows_lan_inbound() {
        let mut mgr = FirewallManager::new();
        let action = mgr.evaluate(Direction::Inbound, Protocol::TCP, 8080, "192.168.1.5", None);
        assert_eq!(action, RuleAction::Allow);
    }

    #[test]
    fn evaluate_logs_event() {
        let mut mgr = FirewallManager::new();
        assert!(mgr.connection_log.is_empty());
        mgr.evaluate(Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None);
        assert_eq!(mgr.connection_log.len(), 1);
        let ev = &mgr.connection_log[0];
        assert_eq!(ev.direction, Direction::Outbound);
        assert_eq!(ev.protocol, Protocol::TCP);
        assert_eq!(ev.remote_addr, "1.2.3.4");
    }

    #[test]
    fn evaluate_log_capacity() {
        let mut mgr = FirewallManager::new();
        mgr.set_log_capacity(3);
        for i in 0..5 {
            mgr.evaluate_at(
                i as u64,
                Direction::Outbound,
                Protocol::TCP,
                80,
                "1.2.3.4",
                None,
            );
        }
        assert_eq!(mgr.connection_log.len(), 3);
        // Oldest entries should be evicted.
        assert_eq!(mgr.connection_log[0].timestamp, 2);
        assert_eq!(mgr.connection_log[1].timestamp, 3);
        assert_eq!(mgr.connection_log[2].timestamp, 4);
    }

    #[test]
    fn evaluate_with_timestamp() {
        let mut mgr = FirewallManager::new();
        mgr.evaluate_at(12345, Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None);
        assert_eq!(mgr.connection_log[0].timestamp, 12345);
    }

    #[test]
    fn clear_log() {
        let mut mgr = FirewallManager::new();
        mgr.evaluate(Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None);
        assert!(!mgr.connection_log.is_empty());
        mgr.clear_log();
        assert!(mgr.connection_log.is_empty());
    }

    // -- profile management -------------------------------------------------

    #[test]
    fn add_custom_profile() {
        let mut mgr = FirewallManager::new();
        let p = FirewallProfile {
            id: "custom".into(),
            name: "Custom".into(),
            rules: Vec::new(),
            default_inbound: RuleAction::Block,
            default_outbound: RuleAction::Block,
        };
        mgr.add_profile(p).unwrap();
        assert!(mgr.profile_ids().contains(&"custom"));
        mgr.set_profile("custom").unwrap();
        assert_eq!(mgr.active_profile().id, "custom");
    }

    #[test]
    fn add_duplicate_profile() {
        let mut mgr = FirewallManager::new();
        let p = FirewallProfile {
            id: "home".into(),
            name: "Home 2".into(),
            rules: Vec::new(),
            default_inbound: RuleAction::Allow,
            default_outbound: RuleAction::Allow,
        };
        assert!(mgr.add_profile(p).is_err());
    }

    #[test]
    fn remove_profile() {
        let mut mgr = FirewallManager::new();
        // Cannot remove active profile.
        assert!(mgr.remove_profile("home").is_err());
        mgr.set_profile("public").unwrap();
        assert!(mgr.remove_profile("home").is_ok());
        assert!(!mgr.profile_ids().contains(&"home"));
    }

    #[test]
    fn remove_nonexistent_profile() {
        let mut mgr = FirewallManager::new();
        assert_eq!(
            mgr.remove_profile("nope"),
            Err(FirewallError::ProfileNotFound),
        );
    }

    // -- Display impls ------------------------------------------------------

    #[test]
    fn display_direction() {
        assert_eq!(format!("{}", Direction::Inbound), "Inbound");
        assert_eq!(format!("{}", Direction::Outbound), "Outbound");
    }

    #[test]
    fn display_rule_action() {
        assert_eq!(format!("{}", RuleAction::Allow), "Allow");
        assert_eq!(format!("{}", RuleAction::Block), "Block");
        assert_eq!(format!("{}", RuleAction::Log), "Log");
    }

    #[test]
    fn display_protocol() {
        assert_eq!(format!("{}", Protocol::TCP), "TCP");
        assert_eq!(format!("{}", Protocol::UDP), "UDP");
        assert_eq!(format!("{}", Protocol::ICMP), "ICMP");
        assert_eq!(format!("{}", Protocol::Any), "Any");
    }

    #[test]
    fn display_port_spec() {
        assert_eq!(format!("{}", PortSpec::Single(443)), "443");
        assert_eq!(format!("{}", PortSpec::Range(8000, 9000)), "8000-9000");
        assert_eq!(format!("{}", PortSpec::Any), "*");
    }

    #[test]
    fn display_address_spec() {
        assert_eq!(
            format!("{}", AddressSpec::Single("10.0.0.1".into())),
            "10.0.0.1",
        );
        assert_eq!(
            format!("{}", AddressSpec::Subnet("192.168.0.0".into(), 24)),
            "192.168.0.0/24",
        );
        assert_eq!(format!("{}", AddressSpec::Any), "*");
    }

    #[test]
    fn display_firewall_error() {
        assert_eq!(
            format!("{}", FirewallError::ProfileNotFound),
            "profile not found",
        );
        assert_eq!(
            format!("{}", FirewallError::RuleNotFound),
            "rule not found",
        );
        assert_eq!(
            format!("{}", FirewallError::DuplicateRuleId),
            "duplicate rule id",
        );
        assert_eq!(format!("{}", FirewallError::NotSupported), "not supported");
        assert_eq!(
            format!("{}", FirewallError::PermissionDenied),
            "permission denied",
        );
        assert_eq!(
            format!("{}", FirewallError::PlatformError("oops".into())),
            "oops",
        );
    }

    // -- IPv4 parsing -------------------------------------------------------

    #[test]
    fn parse_ipv4_valid() {
        assert_eq!(parse_ipv4("10.0.0.1"), Some(0x0A000001));
        assert_eq!(parse_ipv4("192.168.1.0"), Some(0xC0A80100));
        assert_eq!(parse_ipv4("255.255.255.255"), Some(0xFFFFFFFF));
        assert_eq!(parse_ipv4("0.0.0.0"), Some(0));
    }

    #[test]
    fn parse_ipv4_invalid() {
        assert_eq!(parse_ipv4("not.an.ip"), None);
        assert_eq!(parse_ipv4("256.0.0.1"), None);
        assert_eq!(parse_ipv4("1.2.3"), None);
        assert_eq!(parse_ipv4(""), None);
    }

    // -- ConnectionEvent fields ---------------------------------------------

    #[test]
    fn connection_event_fields() {
        let ev = ConnectionEvent {
            timestamp: 1000,
            direction: Direction::Inbound,
            protocol: Protocol::UDP,
            local_port: 53,
            remote_addr: "8.8.8.8".into(),
            remote_port: 53,
            action: RuleAction::Allow,
            rule_id: Some(42),
            app: Some("/usr/bin/dns".into()),
        };
        assert_eq!(ev.timestamp, 1000);
        assert_eq!(ev.direction, Direction::Inbound);
        assert_eq!(ev.protocol, Protocol::UDP);
        assert_eq!(ev.local_port, 53);
        assert_eq!(ev.remote_addr, "8.8.8.8");
        assert_eq!(ev.remote_port, 53);
        assert_eq!(ev.action, RuleAction::Allow);
        assert_eq!(ev.rule_id, Some(42));
        assert_eq!(ev.app, Some("/usr/bin/dns".into()));
    }

    // -- Default impl -------------------------------------------------------

    #[test]
    fn manager_default_impl() {
        let mgr = FirewallManager::default();
        assert_eq!(mgr.active_profile().id, "home");
    }

    // -- priority ordering --------------------------------------------------

    #[test]
    fn evaluate_priority_ordering() {
        let mut mgr = FirewallManager::new();
        // Add two conflicting rules: low-prio allow, high-prio block.
        let profile = FirewallProfile {
            id: "test".into(),
            name: "Test".into(),
            rules: vec![
                FirewallRule {
                    id: 1,
                    name: "Allow (low prio)".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Allow,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(80),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 100,
                },
                FirewallRule {
                    id: 2,
                    name: "Block (high prio)".into(),
                    enabled: true,
                    direction: Direction::Outbound,
                    action: RuleAction::Block,
                    protocol: Protocol::TCP,
                    port: PortSpec::Single(80),
                    remote_address: AddressSpec::Any,
                    application: None,
                    priority: 10,
                },
            ],
            default_inbound: RuleAction::Block,
            default_outbound: RuleAction::Block,
        };
        mgr.add_profile(profile).unwrap();
        mgr.set_profile("test").unwrap();
        let action = mgr.evaluate(Direction::Outbound, Protocol::TCP, 80, "1.2.3.4", None);
        assert_eq!(action, RuleAction::Block);
    }

    // -- stub platform ------------------------------------------------------

    #[test]
    fn stub_platform_is_not_supported() {
        use platform::stub::StubFirewall;
        let fw = StubFirewall::new();
        assert!(matches!(fw.is_enabled(), Err(FirewallError::NotSupported)));
        assert!(matches!(fw.list_rules(), Err(FirewallError::NotSupported)));
    }

    #[test]
    fn stub_platform_add_remove_rule() {
        use platform::stub::StubFirewall;
        let mut fw = StubFirewall::new();
        let rule = FirewallRule {
            id: 1,
            name: "test".into(),
            enabled: true,
            direction: Direction::Outbound,
            action: RuleAction::Allow,
            protocol: Protocol::Any,
            port: PortSpec::Any,
            remote_address: AddressSpec::Any,
            application: None,
            priority: 1,
        };
        assert!(matches!(fw.add_rule(&rule), Err(FirewallError::NotSupported)));
        assert!(matches!(
            fw.remove_rule("test"),
            Err(FirewallError::NotSupported),
        ));
    }
}

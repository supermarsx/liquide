use std::process::Command;

use crate::{
    Direction, FirewallBackend, FirewallError, FirewallProfile, FirewallRule, PortSpec,
    Protocol, RuleAction,
};

/// Windows firewall backend using PowerShell `NetFirewallRule` cmdlets.
pub struct PlatformFirewall {
    /// Prefix added to all rule names managed by LiquiDE so they can be
    /// identified and cleaned up.
    rule_prefix: String,
}

impl PlatformFirewall {
    pub fn new() -> Self {
        Self {
            rule_prefix: "LiquiDE_".into(),
        }
    }

    fn prefixed_name(&self, name: &str) -> String {
        format!("{}{}", self.rule_prefix, name)
    }

    fn run_powershell(script: &str) -> Result<String, FirewallError> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| {
                FirewallError::PlatformError(format!("failed to run powershell: {e}"))
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("Access is denied")
                || stderr.contains("not have permission")
                || stderr.contains("requires elevation")
            {
                Err(FirewallError::PermissionDenied)
            } else {
                Err(FirewallError::PlatformError(stderr))
            }
        }
    }

    fn direction_str(d: Direction) -> &'static str {
        match d {
            Direction::Inbound => "Inbound",
            Direction::Outbound => "Outbound",
        }
    }

    fn action_str(a: RuleAction) -> &'static str {
        match a {
            RuleAction::Allow => "Allow",
            RuleAction::Block => "Block",
            // Windows firewall doesn't have a native "Log" action; we map it
            // to Allow and rely on the auditing policy to capture it.
            RuleAction::Log => "Allow",
        }
    }

    fn protocol_str(p: Protocol) -> &'static str {
        match p {
            Protocol::TCP => "TCP",
            Protocol::UDP => "UDP",
            Protocol::ICMP => "ICMPv4",
            Protocol::Any => "Any",
        }
    }

    fn port_str(p: &PortSpec) -> String {
        match p {
            PortSpec::Single(port) => port.to_string(),
            PortSpec::Range(lo, hi) => format!("{lo}-{hi}"),
            PortSpec::Any => "Any".into(),
        }
    }

    fn build_new_rule_script(&self, rule: &FirewallRule) -> String {
        let display_name = self.prefixed_name(&rule.name);
        let dir = Self::direction_str(rule.direction);
        let action = Self::action_str(rule.action);
        let proto = Self::protocol_str(rule.protocol);
        let port = Self::port_str(&rule.port);
        let enabled = if rule.enabled { "True" } else { "False" };

        let mut script = format!(
            "New-NetFirewallRule -DisplayName '{display_name}' \
             -Direction {dir} -Action {action} -Protocol {proto} \
             -Enabled {enabled}",
        );

        // Port (only valid for TCP/UDP).
        match rule.protocol {
            Protocol::TCP | Protocol::UDP => {
                match rule.direction {
                    Direction::Inbound => {
                        script.push_str(&format!(" -LocalPort {port}"));
                    }
                    Direction::Outbound => {
                        script.push_str(&format!(" -RemotePort {port}"));
                    }
                }
            }
            _ => {}
        }

        // Remote address.
        match &rule.remote_address {
            crate::AddressSpec::Single(addr) => {
                script.push_str(&format!(" -RemoteAddress '{addr}'"));
            }
            crate::AddressSpec::Subnet(base, prefix) => {
                script.push_str(&format!(" -RemoteAddress '{base}/{prefix}'"));
            }
            crate::AddressSpec::Any => {}
        }

        // Application.
        if let Some(ref app) = rule.application {
            script.push_str(&format!(" -Program '{app}'"));
        }

        script
    }
}

impl FirewallBackend for PlatformFirewall {
    fn apply_profile(&mut self, profile: &FirewallProfile) -> Result<(), FirewallError> {
        // Remove all existing LiquiDE rules first.
        let remove_script = format!(
            "Get-NetFirewallRule -DisplayName '{}*' -ErrorAction SilentlyContinue | \
             Remove-NetFirewallRule -ErrorAction SilentlyContinue",
            self.rule_prefix,
        );
        Self::run_powershell(&remove_script)?;

        // Add each rule.
        for rule in &profile.rules {
            let script = self.build_new_rule_script(rule);
            Self::run_powershell(&script)?;
        }

        Ok(())
    }

    fn add_rule(&mut self, rule: &FirewallRule) -> Result<(), FirewallError> {
        let script = self.build_new_rule_script(rule);
        Self::run_powershell(&script)?;
        Ok(())
    }

    fn remove_rule(&mut self, rule_name: &str) -> Result<(), FirewallError> {
        let display_name = self.prefixed_name(rule_name);
        let script = format!(
            "Remove-NetFirewallRule -DisplayName '{display_name}' -ErrorAction Stop",
        );
        Self::run_powershell(&script)?;
        Ok(())
    }

    fn list_rules(&self) -> Result<Vec<String>, FirewallError> {
        let script = format!(
            "Get-NetFirewallRule -DisplayName '{}*' -ErrorAction SilentlyContinue | \
             ForEach-Object {{ $_.DisplayName }}",
            self.rule_prefix,
        );
        let output = Self::run_powershell(&script)?;
        let names: Vec<String> = output
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        Ok(names)
    }

    fn is_enabled(&self) -> Result<bool, FirewallError> {
        let script =
            "(Get-NetFirewallProfile -Profile Domain,Public,Private | \
             Where-Object { $_.Enabled -eq 'True' }).Count";
        let output = Self::run_powershell(&script)?;
        let count: u32 = output.trim().parse().unwrap_or(0);
        Ok(count > 0)
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), FirewallError> {
        let state = if enabled { "True" } else { "False" };
        let script = format!(
            "Set-NetFirewallProfile -Profile Domain,Public,Private -Enabled {state}",
        );
        Self::run_powershell(&script)?;
        Ok(())
    }
}

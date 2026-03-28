use std::process::Command;

use crate::{
    AddressSpec, Direction, FirewallBackend, FirewallError, FirewallProfile, FirewallRule,
    PortSpec, Protocol, RuleAction,
};

/// macOS firewall backend using `pfctl` (Packet Filter).
///
/// Rules are placed in a dedicated anchor (`com.liquide/fw`) so they can be
/// managed without disturbing the system's own PF rules.
pub struct PlatformFirewall {
    anchor: String,
}

impl PlatformFirewall {
    pub fn new() -> Self {
        Self {
            anchor: "com.liquide/fw".into(),
        }
    }

    fn run(program: &str, args: &[&str]) -> Result<String, FirewallError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| {
                FirewallError::PlatformError(format!("failed to run {program}: {e}"))
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("Permission denied")
                || stderr.contains("must be root")
                || stderr.contains("Operation not permitted")
            {
                Err(FirewallError::PermissionDenied)
            } else {
                Err(FirewallError::PlatformError(stderr))
            }
        }
    }

    /// Convert a `FirewallRule` to a PF rule string.
    fn rule_to_pf(rule: &FirewallRule) -> String {
        let action = match rule.action {
            RuleAction::Allow => "pass",
            RuleAction::Block => "block",
            RuleAction::Log => "pass log",
        };

        let dir = match rule.direction {
            Direction::Inbound => "in",
            Direction::Outbound => "out",
        };

        let proto = match rule.protocol {
            Protocol::TCP => " proto tcp",
            Protocol::UDP => " proto udp",
            Protocol::ICMP => " proto icmp",
            Protocol::Any => "",
        };

        let addr = match &rule.remote_address {
            AddressSpec::Single(a) => {
                let kw = match rule.direction {
                    Direction::Inbound => "from",
                    Direction::Outbound => "to",
                };
                format!(" {kw} {a}")
            }
            AddressSpec::Subnet(base, prefix) => {
                let kw = match rule.direction {
                    Direction::Inbound => "from",
                    Direction::Outbound => "to",
                };
                format!(" {kw} {base}/{prefix}")
            }
            AddressSpec::Any => String::new(),
        };

        let port = match &rule.port {
            PortSpec::Single(p) => format!(" port {p}"),
            PortSpec::Range(lo, hi) => format!(" port {lo}:{hi}"),
            PortSpec::Any => String::new(),
        };

        // PF rule format: action [log] direction [quick] [proto] [from/to addr] [port N]
        format!(
            "{action} {dir} quick{proto}{addr}{port} # {name}",
            name = rule.name,
        )
    }

    /// Write rules to the anchor via stdin piped to `pfctl -a <anchor> -f -`.
    fn load_rules(&self, rules: &[String]) -> Result<(), FirewallError> {
        let ruleset = rules.join("\n");
        let mut child = Command::new("pfctl")
            .args(["-a", &self.anchor, "-f", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                FirewallError::PlatformError(format!("failed to spawn pfctl: {e}"))
            })?;

        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            let _ = stdin.write_all(ruleset.as_bytes());
        }

        let output = child.wait_with_output().map_err(|e| {
            FirewallError::PlatformError(format!("pfctl wait failed: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("Permission denied") || stderr.contains("must be root") {
                return Err(FirewallError::PermissionDenied);
            }
            return Err(FirewallError::PlatformError(stderr));
        }
        Ok(())
    }
}

impl FirewallBackend for PlatformFirewall {
    fn apply_profile(&mut self, profile: &FirewallProfile) -> Result<(), FirewallError> {
        let pf_rules: Vec<String> = profile
            .rules
            .iter()
            .filter(|r| r.enabled)
            .map(|r| Self::rule_to_pf(r))
            .collect();
        self.load_rules(&pf_rules)
    }

    fn add_rule(&mut self, rule: &FirewallRule) -> Result<(), FirewallError> {
        // Retrieve existing rules, append, reload.
        let existing = Self::run("pfctl", &["-a", &self.anchor, "-sr"])
            .unwrap_or_default();
        let mut rules: Vec<String> = existing
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect();
        rules.push(Self::rule_to_pf(rule));
        self.load_rules(&rules)
    }

    fn remove_rule(&mut self, rule_name: &str) -> Result<(), FirewallError> {
        let existing = Self::run("pfctl", &["-a", &self.anchor, "-sr"])
            .unwrap_or_default();
        let rules: Vec<String> = existing
            .lines()
            .filter(|l| !l.contains(&format!("# {rule_name}")))
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect();
        self.load_rules(&rules)
    }

    fn list_rules(&self) -> Result<Vec<String>, FirewallError> {
        let output = Self::run("pfctl", &["-a", &self.anchor, "-sr"])?;
        let names: Vec<String> = output
            .lines()
            .filter_map(|l| {
                l.find("# ").map(|idx| l[idx + 2..].trim().to_string())
            })
            .collect();
        Ok(names)
    }

    fn is_enabled(&self) -> Result<bool, FirewallError> {
        let output = Self::run("pfctl", &["-si"])?;
        Ok(output.contains("Status: Enabled"))
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), FirewallError> {
        let flag = if enabled { "-e" } else { "-d" };
        Self::run("pfctl", &[flag])?;
        Ok(())
    }
}

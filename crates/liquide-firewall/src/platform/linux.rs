#![cfg_attr(all(test, not(target_os = "linux")), allow(dead_code))]

use std::process::Command;

use crate::{
    AddressSpec, Direction, FirewallBackend, FirewallError, FirewallProfile, FirewallRule,
    PortSpec, Protocol, RuleAction,
};

/// Linux firewall backend.
///
/// Tries `nft` (nftables) first, falls back to `iptables`, and finally `ufw`.
/// `nft` and `iptables` profile replacement use LiquiDE-owned tables/chains.
/// UFW supports individual rule adds/removals only; full profile replacement
/// fails closed because UFW has no dedicated chain equivalent here.
pub struct PlatformFirewall {
    backend: LinuxBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxBackend {
    Nftables,
    Iptables,
    Ufw,
}

impl PlatformFirewall {
    pub fn new() -> Self {
        let backend = if Self::has_command("nft") {
            LinuxBackend::Nftables
        } else if Self::has_command("iptables") {
            LinuxBackend::Iptables
        } else {
            LinuxBackend::Ufw
        };
        Self { backend }
    }

    fn has_command(name: &str) -> bool {
        Command::new("which")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn run(program: &str, args: &[&str]) -> Result<String, FirewallError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| FirewallError::PlatformError(format!("failed to run {program}: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("Permission denied") || stderr.contains("Operation not permitted") {
                Err(FirewallError::PermissionDenied)
            } else {
                Err(FirewallError::PlatformError(stderr))
            }
        }
    }

    // -- nftables helpers ---------------------------------------------------

    fn nft_ensure_table(&self) -> Result<(), FirewallError> {
        // Create table+chain if they don't exist.  `add` is idempotent.
        Self::run("nft", &["add", "table", "inet", "liquide"])?;
        Self::run(
            "nft",
            &[
                "add",
                "chain",
                "inet",
                "liquide",
                "input",
                "{ type filter hook input priority 0; policy accept; }",
            ],
        )?;
        Self::run(
            "nft",
            &[
                "add",
                "chain",
                "inet",
                "liquide",
                "output",
                "{ type filter hook output priority 0; policy accept; }",
            ],
        )?;
        Ok(())
    }

    fn nft_flush(&self) -> Result<(), FirewallError> {
        Self::run("nft", &["flush", "table", "inet", "liquide"])?;
        Ok(())
    }

    fn nft_add_rule(&self, rule: &FirewallRule) -> Result<(), FirewallError> {
        let chain = match rule.direction {
            Direction::Inbound => "input",
            Direction::Outbound => "output",
        };
        let mut parts: Vec<String> = vec![
            "add".into(),
            "rule".into(),
            "inet".into(),
            "liquide".into(),
            chain.into(),
        ];

        // Protocol.
        match rule.protocol {
            Protocol::TCP => {
                parts.push("ip".into());
                parts.push("protocol".into());
                parts.push("tcp".into());
            }
            Protocol::UDP => {
                parts.push("ip".into());
                parts.push("protocol".into());
                parts.push("udp".into());
            }
            Protocol::ICMP => {
                parts.push("ip".into());
                parts.push("protocol".into());
                parts.push("icmp".into());
            }
            Protocol::Any => {}
        }

        // Port.
        match &rule.port {
            PortSpec::Single(p) => {
                let kw = match rule.direction {
                    Direction::Inbound => "dport",
                    Direction::Outbound => "dport",
                };
                let proto = match rule.protocol {
                    Protocol::TCP => "tcp",
                    Protocol::UDP => "udp",
                    _ => "tcp", // default for port rules
                };
                parts.push(proto.into());
                parts.push(kw.into());
                parts.push(p.to_string());
            }
            PortSpec::Range(lo, hi) => {
                let kw = "dport";
                let proto = match rule.protocol {
                    Protocol::TCP => "tcp",
                    Protocol::UDP => "udp",
                    _ => "tcp",
                };
                parts.push(proto.into());
                parts.push(kw.into());
                parts.push(format!("{lo}-{hi}"));
            }
            PortSpec::Any => {}
        }

        // Remote address.
        match &rule.remote_address {
            AddressSpec::Single(addr) => {
                let kw = match rule.direction {
                    Direction::Inbound => "saddr",
                    Direction::Outbound => "daddr",
                };
                parts.push("ip".into());
                parts.push(kw.into());
                parts.push(addr.clone());
            }
            AddressSpec::Subnet(base, prefix) => {
                let kw = match rule.direction {
                    Direction::Inbound => "saddr",
                    Direction::Outbound => "daddr",
                };
                parts.push("ip".into());
                parts.push(kw.into());
                parts.push(format!("{base}/{prefix}"));
            }
            AddressSpec::Any => {}
        }

        // Action.
        match rule.action {
            RuleAction::Allow => parts.push("accept".into()),
            RuleAction::Block => parts.push("drop".into()),
            RuleAction::Log => {
                parts.push("log".into());
                parts.push("prefix".into());
                parts.push(format!("\"liquide: {}\"", rule.name));
                parts.push("accept".into());
            }
        }

        // Comment with rule name for identification.
        parts.push("comment".into());
        parts.push(format!("\"{}\"", rule.name));

        let args: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        Self::run("nft", &args)?;
        Ok(())
    }

    // -- iptables helpers ---------------------------------------------------

    fn iptables_ensure_chain(&self) -> Result<(), FirewallError> {
        // Create custom chains (ignore error if already exists).
        let _ = Self::run("iptables", &["-N", "LIQUIDE_INPUT"]);
        let _ = Self::run("iptables", &["-N", "LIQUIDE_OUTPUT"]);
        // Jump from built-in chains (ignore duplicates).
        let _ = Self::run("iptables", &["-C", "INPUT", "-j", "LIQUIDE_INPUT"])
            .or_else(|_| Self::run("iptables", &["-A", "INPUT", "-j", "LIQUIDE_INPUT"]));
        let _ = Self::run("iptables", &["-C", "OUTPUT", "-j", "LIQUIDE_OUTPUT"])
            .or_else(|_| Self::run("iptables", &["-A", "OUTPUT", "-j", "LIQUIDE_OUTPUT"]));
        Ok(())
    }

    fn iptables_flush(&self) -> Result<(), FirewallError> {
        let _ = Self::run("iptables", &["-F", "LIQUIDE_INPUT"]);
        let _ = Self::run("iptables", &["-F", "LIQUIDE_OUTPUT"]);
        Ok(())
    }

    fn iptables_add_rule(&self, rule: &FirewallRule) -> Result<(), FirewallError> {
        let chain = match rule.direction {
            Direction::Inbound => "LIQUIDE_INPUT",
            Direction::Outbound => "LIQUIDE_OUTPUT",
        };

        let mut args: Vec<String> = vec!["-A".into(), chain.into()];

        // Protocol.
        match rule.protocol {
            Protocol::TCP => {
                args.push("-p".into());
                args.push("tcp".into());
            }
            Protocol::UDP => {
                args.push("-p".into());
                args.push("udp".into());
            }
            Protocol::ICMP => {
                args.push("-p".into());
                args.push("icmp".into());
            }
            Protocol::Any => {}
        }

        // Port.
        match &rule.port {
            PortSpec::Single(p) => {
                args.push("--dport".into());
                args.push(p.to_string());
            }
            PortSpec::Range(lo, hi) => {
                args.push("--dport".into());
                args.push(format!("{lo}:{hi}"));
            }
            PortSpec::Any => {}
        }

        // Remote address.
        match &rule.remote_address {
            AddressSpec::Single(addr) => {
                let flag = match rule.direction {
                    Direction::Inbound => "-s",
                    Direction::Outbound => "-d",
                };
                args.push(flag.into());
                args.push(addr.clone());
            }
            AddressSpec::Subnet(base, prefix) => {
                let flag = match rule.direction {
                    Direction::Inbound => "-s",
                    Direction::Outbound => "-d",
                };
                args.push(flag.into());
                args.push(format!("{base}/{prefix}"));
            }
            AddressSpec::Any => {}
        }

        // Action.
        args.push("-j".into());
        match rule.action {
            RuleAction::Allow => args.push("ACCEPT".into()),
            RuleAction::Block => args.push("DROP".into()),
            RuleAction::Log => {
                // For LOG we add two rules: LOG then ACCEPT.
                args.push("LOG".into());
                args.push("--log-prefix".into());
                args.push(format!("liquide/{}: ", rule.name));
                let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                Self::run("iptables", &a)?;
                // Now add the ACCEPT rule.
                // Replace LOG → ACCEPT and remove log-prefix.
                let idx = args.len();
                args[idx - 3] = "ACCEPT".into();
                args.truncate(idx - 2);
                let a2: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                Self::run("iptables", &a2)?;
                return Ok(());
            }
        }

        // Comment.
        args.push("-m".into());
        args.push("comment".into());
        args.push("--comment".into());
        args.push(rule.name.clone());

        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        Self::run("iptables", &a)?;
        Ok(())
    }

    // -- ufw helpers --------------------------------------------------------

    fn ufw_add_rule(&self, rule: &FirewallRule) -> Result<(), FirewallError> {
        let mut args: Vec<String> = Vec::new();

        // Direction.
        match rule.direction {
            Direction::Inbound => args.push("allow".into()),
            Direction::Outbound => args.push("allow".into()),
        }

        // Override with deny if blocking.
        if rule.action == RuleAction::Block {
            args[0] = "deny".into();
        }

        // Direction keyword.
        match rule.direction {
            Direction::Inbound => args.push("in".into()),
            Direction::Outbound => args.push("out".into()),
        }

        // Protocol.
        match rule.protocol {
            Protocol::TCP | Protocol::UDP => {
                args.push("proto".into());
                args.push(match rule.protocol {
                    Protocol::TCP => "tcp".into(),
                    Protocol::UDP => "udp".into(),
                    _ => unreachable!(),
                });
            }
            _ => {}
        }

        // Remote address.
        match &rule.remote_address {
            AddressSpec::Single(addr) => {
                args.push("from".into());
                args.push(addr.clone());
            }
            AddressSpec::Subnet(base, prefix) => {
                args.push("from".into());
                args.push(format!("{base}/{prefix}"));
            }
            AddressSpec::Any => {}
        }

        // Port.
        match &rule.port {
            PortSpec::Single(p) => {
                args.push("to".into());
                args.push("any".into());
                args.push("port".into());
                args.push(p.to_string());
            }
            PortSpec::Range(lo, hi) => {
                args.push("to".into());
                args.push("any".into());
                args.push("port".into());
                args.push(format!("{lo}:{hi}"));
            }
            PortSpec::Any => {}
        }

        // Comment.
        args.push("comment".into());
        args.push(rule.name.clone());

        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        Self::run("ufw", &a)?;
        Ok(())
    }
}

impl FirewallBackend for PlatformFirewall {
    fn apply_profile(&mut self, profile: &FirewallProfile) -> Result<(), FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                self.nft_ensure_table()?;
                self.nft_flush()?;
                for rule in &profile.rules {
                    if rule.enabled {
                        self.nft_add_rule(rule)?;
                    }
                }
            }
            LinuxBackend::Iptables => {
                self.iptables_ensure_chain()?;
                self.iptables_flush()?;
                for rule in &profile.rules {
                    if rule.enabled {
                        self.iptables_add_rule(rule)?;
                    }
                }
            }
            LinuxBackend::Ufw => {
                let _ = profile;
                return Err(FirewallError::NotSupported);
            }
        }
        Ok(())
    }

    fn add_rule(&mut self, rule: &FirewallRule) -> Result<(), FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                self.nft_ensure_table()?;
                self.nft_add_rule(rule)
            }
            LinuxBackend::Iptables => {
                self.iptables_ensure_chain()?;
                self.iptables_add_rule(rule)
            }
            LinuxBackend::Ufw => self.ufw_add_rule(rule),
        }
    }

    fn remove_rule(&mut self, rule_name: &str) -> Result<(), FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                // nft requires handle numbers to delete; list and grep.
                let output = Self::run("nft", &["-a", "list", "table", "inet", "liquide"])?;
                for line in output.lines() {
                    if line.contains(rule_name) {
                        // Extract handle number from "# handle N".
                        if let Some(handle) = line
                            .rsplit("# handle ")
                            .next()
                            .and_then(|s| s.trim().parse::<u64>().ok())
                        {
                            // Determine chain from rule text.
                            let chain = if line.contains("chain input") || line.contains("input") {
                                "input"
                            } else {
                                "output"
                            };
                            Self::run(
                                "nft",
                                &[
                                    "delete",
                                    "rule",
                                    "inet",
                                    "liquide",
                                    chain,
                                    "handle",
                                    &handle.to_string(),
                                ],
                            )?;
                        }
                    }
                }
                Ok(())
            }
            LinuxBackend::Iptables => {
                // Delete by comment match.
                for chain in &["LIQUIDE_INPUT", "LIQUIDE_OUTPUT"] {
                    let output = Self::run("iptables", &["-L", chain, "--line-numbers", "-n"])
                        .unwrap_or_default();
                    let mut line_nums: Vec<u32> = Vec::new();
                    for line in output.lines() {
                        if line.contains(rule_name) {
                            if let Some(num) = line.split_whitespace().next() {
                                if let Ok(n) = num.parse::<u32>() {
                                    line_nums.push(n);
                                }
                            }
                        }
                    }
                    // Delete in reverse order to keep line numbers stable.
                    line_nums.sort();
                    line_nums.reverse();
                    for num in line_nums {
                        Self::run("iptables", &["-D", chain, &num.to_string()])?;
                    }
                }
                Ok(())
            }
            LinuxBackend::Ufw => {
                // ufw delete by re-specifying the rule (best-effort).
                let _ = Self::run("ufw", &["delete", "allow", "comment", rule_name]);
                let _ = Self::run("ufw", &["delete", "deny", "comment", rule_name]);
                Ok(())
            }
        }
    }

    fn list_rules(&self) -> Result<Vec<String>, FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                let output = Self::run("nft", &["-a", "list", "table", "inet", "liquide"])?;
                let names: Vec<String> = output
                    .lines()
                    .filter(|l| l.contains("comment"))
                    .filter_map(|l| {
                        // Extract comment string between quotes.
                        let start = l.find("comment \"")? + 9;
                        let end = l[start..].find('"')? + start;
                        Some(l[start..end].to_string())
                    })
                    .collect();
                Ok(names)
            }
            LinuxBackend::Iptables => {
                let mut names = Vec::new();
                for chain in &["LIQUIDE_INPUT", "LIQUIDE_OUTPUT"] {
                    let output = Self::run("iptables", &["-L", chain, "-n", "--line-numbers"])
                        .unwrap_or_default();
                    for line in output.lines() {
                        if let Some(idx) = line.find("/* ") {
                            let rest = &line[idx + 3..];
                            if let Some(end) = rest.find(" */") {
                                names.push(rest[..end].to_string());
                            }
                        }
                    }
                }
                Ok(names)
            }
            LinuxBackend::Ufw => {
                let output = Self::run("ufw", &["status", "verbose"])?;
                let names: Vec<String> = output
                    .lines()
                    .filter(|l| l.contains("ALLOW") || l.contains("DENY") || l.contains("REJECT"))
                    .map(|l| l.trim().to_string())
                    .collect();
                Ok(names)
            }
        }
    }

    fn is_enabled(&self) -> Result<bool, FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                let output = Self::run("nft", &["list", "tables"])?;
                Ok(!output.trim().is_empty())
            }
            LinuxBackend::Iptables => {
                let output = Self::run("iptables", &["-L", "-n"])?;
                Ok(!output.trim().is_empty())
            }
            LinuxBackend::Ufw => {
                let output = Self::run("ufw", &["status"])?;
                Ok(output.contains("Status: active"))
            }
        }
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), FirewallError> {
        match self.backend {
            LinuxBackend::Nftables => {
                if enabled {
                    self.nft_ensure_table()?;
                } else {
                    let _ = Self::run("nft", &["delete", "table", "inet", "liquide"]);
                }
                Ok(())
            }
            LinuxBackend::Iptables => {
                if enabled {
                    self.iptables_ensure_chain()?;
                } else {
                    self.iptables_flush()?;
                    let _ = Self::run("iptables", &["-D", "INPUT", "-j", "LIQUIDE_INPUT"]);
                    let _ = Self::run("iptables", &["-D", "OUTPUT", "-j", "LIQUIDE_OUTPUT"]);
                    let _ = Self::run("iptables", &["-X", "LIQUIDE_INPUT"]);
                    let _ = Self::run("iptables", &["-X", "LIQUIDE_OUTPUT"]);
                }
                Ok(())
            }
            LinuxBackend::Ufw => {
                let cmd = if enabled { "enable" } else { "disable" };
                Self::run("ufw", &["--force", cmd])?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FirewallBackend, RuleAction};

    #[test]
    fn ufw_apply_profile_fails_closed_without_global_reset() {
        let mut firewall = PlatformFirewall {
            backend: LinuxBackend::Ufw,
        };
        let profile = FirewallProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            rules: Vec::new(),
            default_inbound: RuleAction::Block,
            default_outbound: RuleAction::Allow,
        };

        assert!(matches!(
            firewall.apply_profile(&profile),
            Err(FirewallError::NotSupported)
        ));
    }
}

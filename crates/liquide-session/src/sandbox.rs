//! Sandbox enforcement for session isolation.

use std::fmt;

use crate::Result;
use crate::config::{JailConfig, JailNetwork};

/// Type of jail to apply to the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JailType {
    /// No sandboxing.
    None,
    /// Linux namespace isolation.
    Namespace,
    /// Seccomp syscall filtering.
    Seccomp,
    /// Full container isolation.
    Container,
    /// Combined namespace + seccomp + additional restrictions.
    Combined,
}

impl fmt::Display for JailType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Namespace => write!(f, "Namespace"),
            Self::Seccomp => write!(f, "Seccomp"),
            Self::Container => write!(f, "Container"),
            Self::Combined => write!(f, "Combined"),
        }
    }
}

/// Configuration for Linux namespace isolation.
#[derive(Debug, Clone)]
pub struct NamespaceConfig {
    /// Enable user namespace.
    pub user_ns: bool,
    /// Enable mount namespace.
    pub mount_ns: bool,
    /// Enable PID namespace.
    pub pid_ns: bool,
    /// Enable network namespace.
    pub net_ns: bool,
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            user_ns: true,
            mount_ns: true,
            pid_ns: true,
            net_ns: false,
        }
    }
}

/// Enforces sandbox restrictions on a session.
pub struct SandboxEnforcer {
    config: JailConfig,
    namespace_config: NamespaceConfig,
    enforced: bool,
}

impl SandboxEnforcer {
    /// Create a new sandbox enforcer.
    #[must_use]
    pub fn new(config: JailConfig) -> Self {
        let namespace_config = match config.network {
            JailNetwork::Isolated | JailNetwork::None => NamespaceConfig {
                net_ns: true,
                ..NamespaceConfig::default()
            },
            JailNetwork::Host => NamespaceConfig::default(),
        };

        Self {
            config,
            namespace_config,
            enforced: false,
        }
    }

    /// Whether the sandbox is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.enforced && self.config.jail_type != JailType::None
    }

    /// The type of jail configured.
    #[must_use]
    pub fn jail_type(&self) -> JailType {
        self.config.jail_type
    }

    /// Paths allowed inside the sandboxed environment.
    #[must_use]
    pub fn allowed_paths(&self) -> &[String] {
        &self.config.allowed_paths
    }

    /// Check whether a path is allowed by the sandbox.
    #[must_use]
    pub fn is_path_allowed(&self, path: &str) -> bool {
        if self.config.jail_type == JailType::None {
            return true;
        }
        self.config
            .allowed_paths
            .iter()
            .any(|allowed| path.starts_with(allowed.as_str()))
    }

    /// Check whether a syscall name is allowed.
    #[must_use]
    pub fn is_syscall_allowed(&self, syscall: &str) -> bool {
        if self.config.jail_type == JailType::None {
            return true;
        }
        !self.config.denied_syscalls.contains(&syscall.to_string())
    }

    /// Activate the sandbox. In a real implementation this would configure
    /// namespaces, seccomp filters, and cgroup limits.
    pub fn enforce(&mut self) -> Result<()> {
        if self.config.jail_type == JailType::None {
            self.enforced = false;
            return Ok(());
        }

        // In production this would set up seccomp, namespaces, cgroups, etc.
        // For now, mark as enforced.
        self.enforced = true;
        Ok(())
    }

    /// The namespace configuration.
    #[must_use]
    pub fn namespace_config(&self) -> &NamespaceConfig {
        &self.namespace_config
    }

    /// The jail configuration.
    #[must_use]
    pub fn jail_config(&self) -> &JailConfig {
        &self.config
    }
}

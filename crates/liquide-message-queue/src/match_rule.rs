//! Signal matching and filtering for the IPC message bus.
//!
//! A [`MatchRule`] describes a pattern that incoming signals are tested against.
//! Only signals that satisfy *all* specified fields are delivered to the
//! subscriber.  Fields that are `None` act as wildcards (match anything).
//!
//! Inspired by the D-Bus match rule syntax (`type='signal',sender='...'`).

/// A pattern for filtering bus signals.
///
/// All fields are optional — an empty `MatchRule` matches every signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRule {
    /// Match only signals from this sender address.
    pub sender: Option<String>,
    /// Match only signals on this interface (e.g., `"org.liquide.Shell"`).
    pub interface: Option<String>,
    /// Match only signals with this member name (e.g., `"WindowOpened"`).
    pub member: Option<String>,
    /// Match only signals emitted on this object path (e.g., `"/desktop"`).
    pub path: Option<String>,
    /// Match only signals whose first body argument (if it is a string)
    /// equals this value.
    pub arg0: Option<String>,
}

impl MatchRule {
    /// Create an empty rule that matches everything.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sender: None,
            interface: None,
            member: None,
            path: None,
            arg0: None,
        }
    }

    /// Test whether a signal matches this rule.
    ///
    /// The caller provides the signal's sender, interface, member, path, and
    /// optional first string argument.  Each `Some` field in the rule must
    /// match the corresponding value; `None` fields are wildcards.
    #[must_use]
    pub fn matches(
        &self,
        sender: &str,
        interface: &str,
        member: &str,
        path: &str,
        arg0: Option<&str>,
    ) -> bool {
        if let Some(ref s) = self.sender {
            if s != sender {
                return false;
            }
        }
        if let Some(ref i) = self.interface {
            if i != interface {
                return false;
            }
        }
        if let Some(ref m) = self.member {
            if m != member {
                return false;
            }
        }
        if let Some(ref p) = self.path {
            if p != path {
                return false;
            }
        }
        if let Some(ref a0) = self.arg0 {
            match arg0 {
                Some(v) if v == a0.as_str() => {}
                _ => return false,
            }
        }
        true
    }

    /// Returns `true` if this rule has no filters (matches everything).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sender.is_none()
            && self.interface.is_none()
            && self.member.is_none()
            && self.path.is_none()
            && self.arg0.is_none()
    }

    /// Return a compact string representation of the rule (for diagnostics).
    ///
    /// Format: `"sender='x',interface='y',member='z',path='p',arg0='a'"`.
    #[must_use]
    pub fn to_rule_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref s) = self.sender {
            parts.push(format!("sender='{s}'"));
        }
        if let Some(ref i) = self.interface {
            parts.push(format!("interface='{i}'"));
        }
        if let Some(ref m) = self.member {
            parts.push(format!("member='{m}'"));
        }
        if let Some(ref p) = self.path {
            parts.push(format!("path='{p}'"));
        }
        if let Some(ref a) = self.arg0 {
            parts.push(format!("arg0='{a}'"));
        }
        parts.join(",")
    }
}

impl Default for MatchRule {
    fn default() -> Self {
        Self::new()
    }
}

// ── Builder ─────────────────────────────────────────────────────────────

/// Ergonomic builder for [`MatchRule`].
///
/// ```rust
/// use liquide_message_queue::match_rule::MatchRuleBuilder;
///
/// let rule = MatchRuleBuilder::new()
///     .sender("org.liquide.Shell")
///     .member("WindowOpened")
///     .build();
/// ```
pub struct MatchRuleBuilder {
    rule: MatchRule,
}

impl MatchRuleBuilder {
    /// Start building a new match rule.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rule: MatchRule::new(),
        }
    }

    /// Filter by sender address.
    #[must_use]
    pub fn sender(mut self, sender: &str) -> Self {
        self.rule.sender = Some(sender.to_owned());
        self
    }

    /// Filter by interface name.
    #[must_use]
    pub fn interface(mut self, interface: &str) -> Self {
        self.rule.interface = Some(interface.to_owned());
        self
    }

    /// Filter by member (method / signal) name.
    #[must_use]
    pub fn member(mut self, member: &str) -> Self {
        self.rule.member = Some(member.to_owned());
        self
    }

    /// Filter by object path.
    #[must_use]
    pub fn path(mut self, path: &str) -> Self {
        self.rule.path = Some(path.to_owned());
        self
    }

    /// Filter by first string argument.
    #[must_use]
    pub fn arg0(mut self, arg0: &str) -> Self {
        self.rule.arg0 = Some(arg0.to_owned());
        self
    }

    /// Consume the builder and return the finished [`MatchRule`].
    #[must_use]
    pub fn build(self) -> MatchRule {
        self.rule
    }
}

impl Default for MatchRuleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

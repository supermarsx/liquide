//! In-process IPC message bus for the LiquiDE desktop environment.
//!
//! The [`MessageBus`] is the central hub for inter-component communication.
//! It provides:
//!
//! - **Signals** — broadcast messages with no specific destination (pub/sub).
//! - **Method calls** — targeted request/response to a named service.
//! - **Name ownership** — components claim well-known addresses for routing.
//!
//! Inspired by the freedesktop.org D-Bus specification but implemented as a
//! lightweight, single-process, synchronous message bus with no system daemon.

use std::collections::HashMap;

use crate::match_rule::MatchRule;
use crate::serial::BusValue;
use crate::service::{BusError, MethodCall, Response, ServiceRegistry};

// ── Bus address ─────────────────────────────────────────────────────────

/// A reverse-DNS bus address identifying a component on the bus.
///
/// Examples: `"org.liquide.Shell"`, `"org.liquide.Settings"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BusAddress(pub String);

impl BusAddress {
    /// Create a new bus address.
    #[must_use]
    pub fn new(addr: impl Into<String>) -> Self {
        Self(addr.into())
    }

    /// The address string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate the address format.
    ///
    /// A valid address has at least two dot-separated segments, each
    /// containing only ASCII alphanumerics and underscores, with the first
    /// character of each segment being a letter or underscore.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let parts: Vec<&str> = self.0.split('.').collect();
        if parts.len() < 2 {
            return false;
        }
        for part in &parts {
            if part.is_empty() {
                return false;
            }
            let first = part.as_bytes()[0];
            if !first.is_ascii_alphabetic() && first != b'_' {
                return false;
            }
            if !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return false;
            }
        }
        true
    }
}

impl std::fmt::Display for BusAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for BusAddress {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ── Message types ───────────────────────────────────────────────────────

/// The kind of a bus message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusMessageType {
    /// A broadcast signal (no specific destination).
    Signal,
    /// A method call to a named service.
    MethodCall,
    /// A successful return from a method call.
    MethodReturn,
    /// An error return from a method call.
    Error,
}

/// A message on the bus.
#[derive(Debug, Clone)]
pub struct BusMessage {
    /// What kind of message this is.
    pub msg_type: BusMessageType,
    /// Bus address of the sender.
    pub sender: String,
    /// Bus address of the destination (empty for signals).
    pub destination: String,
    /// Object path (e.g., `"/desktop"`, `"/windows/1"`).
    pub path: String,
    /// Interface name (groups related methods/signals).
    pub interface: String,
    /// Method or signal name.
    pub member: String,
    /// Argument values.
    pub body: Vec<BusValue>,
    /// Monotonically increasing serial number.
    pub serial: u64,
    /// For MethodReturn/Error: the serial of the call being replied to.
    pub reply_serial: Option<u64>,
    /// Error name (only for `BusMessageType::Error`).
    pub error_name: Option<String>,
}

impl BusMessage {
    /// Create a new signal message.
    #[must_use]
    pub fn signal(
        sender: impl Into<String>,
        path: impl Into<String>,
        interface: impl Into<String>,
        member: impl Into<String>,
    ) -> Self {
        Self {
            msg_type: BusMessageType::Signal,
            sender: sender.into(),
            destination: String::new(),
            path: path.into(),
            interface: interface.into(),
            member: member.into(),
            body: Vec::new(),
            serial: 0,
            reply_serial: None,
            error_name: None,
        }
    }

    /// Create a method-call message.
    #[must_use]
    pub fn method_call(
        sender: impl Into<String>,
        destination: impl Into<String>,
        path: impl Into<String>,
        interface: impl Into<String>,
        member: impl Into<String>,
    ) -> Self {
        Self {
            msg_type: BusMessageType::MethodCall,
            sender: sender.into(),
            destination: destination.into(),
            path: path.into(),
            interface: interface.into(),
            member: member.into(),
            body: Vec::new(),
            serial: 0,
            reply_serial: None,
            error_name: None,
        }
    }

    /// Builder: set body arguments.
    #[must_use]
    pub fn with_body(mut self, body: Vec<BusValue>) -> Self {
        self.body = body;
        self
    }

    /// Builder: add a single argument.
    #[must_use]
    pub fn with_arg(mut self, arg: BusValue) -> Self {
        self.body.push(arg);
        self
    }
}

/// A broadcast signal (convenience alias used by the bus's pub/sub layer).
#[derive(Debug, Clone)]
pub struct Signal {
    /// Sender address.
    pub sender: String,
    /// Object path.
    pub path: String,
    /// Interface name.
    pub interface: String,
    /// Signal name.
    pub member: String,
    /// Arguments.
    pub args: Vec<BusValue>,
}

impl Signal {
    /// Create a new signal.
    #[must_use]
    pub fn new(
        sender: impl Into<String>,
        path: impl Into<String>,
        interface: impl Into<String>,
        member: impl Into<String>,
    ) -> Self {
        Self {
            sender: sender.into(),
            path: path.into(),
            interface: interface.into(),
            member: member.into(),
            args: Vec::new(),
        }
    }

    /// Builder: set arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<BusValue>) -> Self {
        self.args = args;
        self
    }

    /// Builder: add a single argument.
    #[must_use]
    pub fn with_arg(mut self, arg: BusValue) -> Self {
        self.args.push(arg);
        self
    }
}

// ── Subscriber record ───────────────────────────────────────────────────

/// Unique identifier for a subscription.
pub type SubscriptionId = u64;

/// Internal record of a signal subscriber.
struct Subscription {
    id: SubscriptionId,
    /// The bus address of the subscriber (for diagnostics / removal by owner).
    owner: String,
    /// Match rule that the signal must satisfy.
    rule: MatchRule,
    /// Accumulated signals delivered to this subscription that have not yet
    /// been drained by the subscriber.
    pending: Vec<Signal>,
}

// ── MessageBus ──────────────────────────────────────────────────────────

/// The central in-process message bus.
///
/// Components interact with the bus by:
/// 1. Claiming a name via [`request_name`](Self::request_name).
/// 2. Subscribing to signals via [`subscribe`](Self::subscribe).
/// 3. Publishing signals via [`publish`](Self::publish).
/// 4. Making method calls via [`call`](Self::call).
pub struct MessageBus {
    /// Next subscription id.
    next_sub_id: SubscriptionId,
    /// Next message serial number.
    next_serial: u64,
    /// Registered name owners: address -> owner string.
    names: HashMap<String, String>,
    /// Signal subscriptions.
    subscriptions: Vec<Subscription>,
    /// Service registry for method dispatch.
    services: ServiceRegistry,
    /// History of published signals (bounded ring buffer).
    signal_log: Vec<Signal>,
    /// Maximum number of signals to keep in the log.
    signal_log_capacity: usize,
}

impl MessageBus {
    /// Create a new, empty message bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_sub_id: 1,
            next_serial: 1,
            names: HashMap::new(),
            subscriptions: Vec::new(),
            services: ServiceRegistry::new(),
            signal_log: Vec::new(),
            signal_log_capacity: 256,
        }
    }

    /// Create a bus with a custom signal log capacity.
    #[must_use]
    pub fn with_log_capacity(capacity: usize) -> Self {
        Self {
            signal_log_capacity: capacity,
            ..Self::new()
        }
    }

    // ── Name ownership ──────────────────────────────────────────────────

    /// Request ownership of a well-known bus name.
    ///
    /// Returns `true` if the name was successfully claimed.  Returns `false`
    /// if the name is already owned by another entity.
    pub fn request_name(&mut self, address: &str, owner: &str) -> bool {
        if self.names.contains_key(address) {
            return false;
        }
        self.names.insert(address.to_owned(), owner.to_owned());
        true
    }

    /// Release a previously claimed name.
    ///
    /// Returns `true` if the name existed and was released.
    pub fn release_name(&mut self, address: &str) -> bool {
        self.names.remove(address).is_some()
    }

    /// Check whether a name is currently owned.
    #[must_use]
    pub fn has_name(&self, address: &str) -> bool {
        self.names.contains_key(address)
    }

    /// Get the owner of a name, if any.
    #[must_use]
    pub fn name_owner(&self, address: &str) -> Option<&str> {
        self.names.get(address).map(|s| s.as_str())
    }

    /// List all registered names.
    #[must_use]
    pub fn list_names(&self) -> Vec<String> {
        self.names.keys().cloned().collect()
    }

    // ── Subscriptions (pub/sub signals) ─────────────────────────────────

    /// Subscribe to signals matching `rule`.
    ///
    /// Returns a [`SubscriptionId`] that can be used to drain pending signals
    /// or to unsubscribe.
    pub fn subscribe(&mut self, owner: &str, rule: MatchRule) -> SubscriptionId {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscriptions.push(Subscription {
            id,
            owner: owner.to_owned(),
            rule,
            pending: Vec::new(),
        });
        id
    }

    /// Remove a subscription.  Returns `true` if it existed.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        let before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.id != id);
        self.subscriptions.len() != before
    }

    /// Remove all subscriptions owned by `owner`.
    pub fn unsubscribe_all(&mut self, owner: &str) {
        self.subscriptions.retain(|s| s.owner != owner);
    }

    /// Number of active subscriptions.
    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Publish a signal on the bus.
    ///
    /// The signal is delivered to every subscription whose match rule is
    /// satisfied.  A copy is also appended to the signal log.
    pub fn publish(&mut self, signal: Signal) {
        // Deliver to matching subscriptions.
        for sub in &mut self.subscriptions {
            let arg0 = signal.args.first().and_then(|v| v.as_str());
            if sub.rule.matches(
                &signal.sender,
                &signal.interface,
                &signal.member,
                &signal.path,
                arg0,
            ) {
                sub.pending.push(signal.clone());
            }
        }

        // Append to log (ring buffer).
        if self.signal_log.len() >= self.signal_log_capacity {
            self.signal_log.remove(0);
        }
        self.signal_log.push(signal);
    }

    /// Drain all pending signals for a subscription.
    ///
    /// Returns the signals accumulated since the last drain (or since
    /// subscription creation).
    pub fn drain_signals(&mut self, id: SubscriptionId) -> Vec<Signal> {
        for sub in &mut self.subscriptions {
            if sub.id == id {
                return std::mem::take(&mut sub.pending);
            }
        }
        Vec::new()
    }

    /// Number of pending (unread) signals for a subscription.
    #[must_use]
    pub fn pending_count(&self, id: SubscriptionId) -> usize {
        for sub in &self.subscriptions {
            if sub.id == id {
                return sub.pending.len();
            }
        }
        0
    }

    /// Read the signal log (most recent signals, up to `signal_log_capacity`).
    #[must_use]
    pub fn signal_log(&self) -> &[Signal] {
        &self.signal_log
    }

    // ── Method calls ────────────────────────────────────────────────────

    /// Access the underlying service registry (for registration).
    #[must_use]
    pub fn services(&self) -> &ServiceRegistry {
        &self.services
    }

    /// Mutable access to the service registry.
    pub fn services_mut(&mut self) -> &mut ServiceRegistry {
        &mut self.services
    }

    /// Make a synchronous method call to a named service.
    ///
    /// The bus looks up the service in its [`ServiceRegistry`] and dispatches
    /// the call.  Returns the response or an error.
    pub fn call(
        &mut self,
        sender: &str,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        args: Vec<BusValue>,
    ) -> Result<Response, BusError> {
        let method_call = MethodCall {
            sender: sender.to_owned(),
            interface: interface.to_owned(),
            member: member.to_owned(),
            path: path.to_owned(),
            args,
        };
        self.services.call(destination, &method_call)
    }

    /// Make a method call using a pre-built [`BusMessage`].
    pub fn call_message(&mut self, msg: &BusMessage) -> Result<Response, BusError> {
        self.call(
            &msg.sender,
            &msg.destination,
            &msg.path,
            &msg.interface,
            &msg.member,
            msg.body.clone(),
        )
    }

    // ── Serials ─────────────────────────────────────────────────────────

    /// Allocate the next serial number (for outgoing messages).
    pub fn next_serial(&mut self) -> u64 {
        let s = self.next_serial;
        self.next_serial += 1;
        s
    }

    // ── Introspection ───────────────────────────────────────────────────

    /// Introspect a service by name.
    ///
    /// Returns the service's metadata if it is registered.
    #[must_use]
    pub fn introspect(&self, service_name: &str) -> Option<crate::service::ServiceInfo> {
        self.services.introspect(service_name)
    }

    /// List all registered service names.
    #[must_use]
    pub fn list_services(&self) -> Vec<String> {
        self.services.list_names()
    }

    // ── Cleanup ─────────────────────────────────────────────────────────

    /// Remove everything related to an owner: release names, unsubscribe,
    /// and unregister their service.
    pub fn disconnect(&mut self, owner: &str) {
        // Release owned names.
        self.names.retain(|_, v| v != owner);
        // Remove subscriptions.
        self.unsubscribe_all(owner);
        // Unregister service (owner == service name by convention).
        self.services.unregister(owner);
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MessageBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageBus")
            .field("names", &self.names.keys().collect::<Vec<_>>())
            .field("subscriptions", &self.subscriptions.len())
            .field("services", &self.services.count())
            .field("signal_log_len", &self.signal_log.len())
            .finish()
    }
}

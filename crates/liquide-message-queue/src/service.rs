//! Service registration and introspection for the IPC message bus.
//!
//! A *service* is a named entity on the bus that can receive method calls and
//! return results.  Services declare their capabilities through [`Interface`]
//! descriptors so that callers can introspect available methods at runtime.

use std::collections::HashMap;

use crate::serial::BusValue;

// ── Error ───────────────────────────────────────────────────────────────

/// Error type returned by service method handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusError {
    /// Machine-readable error name (e.g., `"org.liquide.Error.NotFound"`).
    pub name: String,
    /// Human-readable description.
    pub message: String,
}

impl BusError {
    /// Create a new bus error.
    #[must_use]
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
        }
    }

    /// Convenience: "method not found" error.
    #[must_use]
    pub fn unknown_method(method: &str) -> Self {
        Self {
            name: "org.liquide.Error.UnknownMethod".into(),
            message: format!("no such method: {method}"),
        }
    }

    /// Convenience: "service not found" error.
    #[must_use]
    pub fn unknown_service(name: &str) -> Self {
        Self {
            name: "org.liquide.Error.ServiceUnknown".into(),
            message: format!("service not found: {name}"),
        }
    }

    /// Convenience: "invalid arguments" error.
    #[must_use]
    pub fn invalid_args(detail: &str) -> Self {
        Self {
            name: "org.liquide.Error.InvalidArgs".into(),
            message: detail.to_owned(),
        }
    }
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl std::error::Error for BusError {}

// ── MethodCall context ──────────────────────────────────────────────────

/// Incoming method call that a [`Service`] must handle.
#[derive(Debug, Clone)]
pub struct MethodCall {
    /// The caller's bus address (if known).
    pub sender: String,
    /// Interface the method belongs to.
    pub interface: String,
    /// Method name.
    pub member: String,
    /// Object path.
    pub path: String,
    /// Arguments.
    pub args: Vec<BusValue>,
}

/// Successful response to a method call.
#[derive(Debug, Clone)]
pub struct Response {
    /// Return values.
    pub values: Vec<BusValue>,
}

impl Response {
    /// Empty response (void return).
    #[must_use]
    pub fn empty() -> Self {
        Self { values: Vec::new() }
    }

    /// Single-value response.
    #[must_use]
    pub fn single(value: BusValue) -> Self {
        Self {
            values: vec![value],
        }
    }

    /// Multi-value response.
    #[must_use]
    pub fn many(values: Vec<BusValue>) -> Self {
        Self { values }
    }
}

// ── Service trait ───────────────────────────────────────────────────────

/// Trait implemented by any component that wants to receive method calls on
/// the bus.
pub trait Service {
    /// Handle an incoming method call and return a response or error.
    fn handle_method(&mut self, call: &MethodCall) -> Result<Response, BusError>;

    /// Return metadata about this service.
    fn info(&self) -> ServiceInfo;
}

// ── Interface / method descriptors ──────────────────────────────────────

/// A named method within an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    /// Method name.
    pub name: String,
    /// D-Bus-style signature of the input arguments (e.g., `"su"` for
    /// string + uint32).
    pub in_signature: String,
    /// D-Bus-style signature of the return values.
    pub out_signature: String,
}

impl MethodSignature {
    /// Create a new method signature.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        in_sig: impl Into<String>,
        out_sig: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            in_signature: in_sig.into(),
            out_signature: out_sig.into(),
        }
    }
}

/// An interface groups related methods under a single name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Interface name (e.g., `"org.liquide.Shell"`).
    pub name: String,
    /// Methods provided by this interface.
    pub methods: Vec<MethodSignature>,
}

impl Interface {
    /// Create a new interface.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            methods: Vec::new(),
        }
    }

    /// Builder: add a method.
    #[must_use]
    pub fn with_method(mut self, sig: MethodSignature) -> Self {
        self.methods.push(sig);
        self
    }

    /// Find a method by name.
    #[must_use]
    pub fn find_method(&self, name: &str) -> Option<&MethodSignature> {
        self.methods.iter().find(|m| m.name == name)
    }
}

/// Metadata about a registered service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    /// Well-known bus name.
    pub name: String,
    /// Version string (semver recommended).
    pub version: String,
    /// Interfaces provided.
    pub interfaces: Vec<Interface>,
}

impl ServiceInfo {
    /// Create new service info.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            interfaces: Vec::new(),
        }
    }

    /// Builder: add an interface.
    #[must_use]
    pub fn with_interface(mut self, iface: Interface) -> Self {
        self.interfaces.push(iface);
        self
    }

    /// Find an interface by name.
    #[must_use]
    pub fn find_interface(&self, name: &str) -> Option<&Interface> {
        self.interfaces.iter().find(|i| i.name == name)
    }
}

// ── Service Registry ────────────────────────────────────────────────────

/// Central registry of services.
///
/// Components register their [`Service`] implementation here so that other
/// components can discover and call them through the [`MessageBus`](crate::bus::MessageBus).
pub struct ServiceRegistry {
    services: HashMap<String, Box<dyn Service>>,
}

impl ServiceRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service under its well-known name.
    ///
    /// Returns `false` if a service with that name is already registered.
    pub fn register(&mut self, service: Box<dyn Service>) -> bool {
        let name = service.info().name.clone();
        if self.services.contains_key(&name) {
            return false;
        }
        self.services.insert(name, service);
        true
    }

    /// Unregister a service by name.
    ///
    /// Returns `true` if the service existed and was removed.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.services.remove(name).is_some()
    }

    /// Check whether a service is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    /// Number of registered services.
    #[must_use]
    pub fn count(&self) -> usize {
        self.services.len()
    }

    /// List the names of all registered services.
    #[must_use]
    pub fn list_names(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }

    /// Dispatch a method call to the named service.
    pub fn call(&mut self, service_name: &str, call: &MethodCall) -> Result<Response, BusError> {
        let svc = self
            .services
            .get_mut(service_name)
            .ok_or_else(|| BusError::unknown_service(service_name))?;
        svc.handle_method(call)
    }

    /// Introspect a service — return its [`ServiceInfo`] if registered.
    #[must_use]
    pub fn introspect(&self, service_name: &str) -> Option<ServiceInfo> {
        self.services.get(service_name).map(|s| s.info())
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("services", &self.services.keys().collect::<Vec<_>>())
            .finish()
    }
}

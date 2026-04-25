pub mod health;
pub mod inhibitor;
pub mod lifecycle;
pub mod registry;
pub mod service;
pub mod shutdown;
pub mod state;

pub use health::{HealthCheck, HealthStatus};
pub use inhibitor::{InhibitFlag, Inhibitor, InhibitorRegistry};
pub use lifecycle::LifecycleManager;
pub use registry::ServiceRegistry;
pub use service::{RestartPolicy, ServiceDescriptor, ServiceId, ServiceState};
pub use shutdown::{ShutdownKind, ShutdownManager, ShutdownPhase, ShutdownReason};
pub use state::{SessionError, SessionSnapshot, SessionState, SessionStore, SessionWindow};

#[cfg(test)]
mod tests;

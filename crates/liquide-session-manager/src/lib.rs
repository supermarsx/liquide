pub mod service;
pub mod registry;
pub mod lifecycle;
pub mod health;
pub mod state;
pub mod inhibitor;
pub mod shutdown;

pub use service::{ServiceId, ServiceDescriptor, ServiceState, RestartPolicy};
pub use registry::ServiceRegistry;
pub use lifecycle::LifecycleManager;
pub use health::{HealthCheck, HealthStatus};
pub use state::{SessionState, SessionWindow, SessionSnapshot, SessionStore, SessionError};
pub use inhibitor::{InhibitFlag, Inhibitor, InhibitorRegistry};
pub use shutdown::{ShutdownPhase, ShutdownReason, ShutdownKind, ShutdownManager};

#[cfg(test)]
mod tests;

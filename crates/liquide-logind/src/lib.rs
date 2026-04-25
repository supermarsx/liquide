//! Session and seat management for the LiquiDE standalone compositor.
//!
//! Provides logind D-Bus integration for session control, VT (virtual
//! terminal) allocation and switching, and seat device access management.
//!
//! When running as a standalone compositor from TTY, this crate handles:
//! - Registering with systemd-logind for session control
//! - Allocating and activating virtual terminals
//! - Managing DRM master and input device access
//! - Handling sleep/resume and VT switch signals
//!
//! For systems without systemd, a seatd backend is provided as fallback.

pub mod error;
pub mod privileges;
pub mod seat;
pub mod session;
pub mod vt;

pub use error::{LogindError, Result};
pub use privileges::Privileges;
pub use seat::{LogindSeat, SeatBackend, SeatInfo, SeatdSeat, StubSeat};
pub use session::{SessionInfo, SessionProvider, SessionState as LogindSessionState};
pub use vt::{VirtualTerminal, VtMode};

#[cfg(test)]
mod tests;

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
pub mod seat;
pub mod session;
pub mod vt;
pub mod privileges;

pub use error::{LogindError, Result};
pub use seat::{SeatBackend, SeatInfo, LogindSeat, SeatdSeat, StubSeat};
pub use session::{SessionInfo, SessionProvider, SessionState as LogindSessionState};
pub use vt::{VirtualTerminal, VtMode};
pub use privileges::Privileges;

#[cfg(test)]
mod tests;

//! Seat backends for device access management.

use crate::error::{LogindError, Result};

/// Information about a seat.
#[derive(Debug, Clone)]
pub struct SeatInfo {
    /// Seat identifier (e.g. "seat0").
    pub id: String,
    /// Whether the seat supports multiple sessions.
    pub can_multi_session: bool,
    /// Whether the seat has TTY capability.
    pub can_tty: bool,
    /// Whether the seat has graphical capability.
    pub can_graphical: bool,
}

/// Trait for seat device management backends.
pub trait SeatBackend: Send {
    /// Get information about this seat.
    fn seat_info(&self) -> Result<SeatInfo>;

    /// Take a device by major/minor number, returning an fd.
    fn take_device(&mut self, major: u32, minor: u32) -> Result<i32>;

    /// Release a previously taken device.
    fn release_device(&mut self, major: u32, minor: u32) -> Result<()>;
}

/// Logind-based seat backend (communicates via D-Bus).
pub struct LogindSeat {
    /// D-Bus object path for the session.
    session_path: String,
    /// D-Bus object path for the seat.
    bus_path: String,
    /// Whether the seat is currently active.
    active: bool,
}

impl LogindSeat {
    pub fn new(session_path: String, bus_path: String) -> Self {
        Self {
            session_path,
            bus_path,
            active: false,
        }
    }
}

impl SeatBackend for LogindSeat {
    fn seat_info(&self) -> Result<SeatInfo> {
        // Stub: in a real implementation this would query logind via D-Bus.
        Ok(SeatInfo {
            id: "seat0".to_string(),
            can_multi_session: true,
            can_tty: true,
            can_graphical: true,
        })
    }

    fn take_device(&mut self, _major: u32, _minor: u32) -> Result<i32> {
        if !self.active {
            return Err(LogindError::DeviceAccess {
                path: format!("session not active (path={})", self.session_path),
            });
        }
        // Stub: real implementation calls org.freedesktop.login1.Session.TakeDevice
        Err(LogindError::NotSupported)
    }

    fn release_device(&mut self, _major: u32, _minor: u32) -> Result<()> {
        // Stub: real implementation calls org.freedesktop.login1.Session.ReleaseDevice
        let _ = &self.bus_path;
        Err(LogindError::NotSupported)
    }
}

/// Seatd-based seat backend (communicates via Unix socket).
pub struct SeatdSeat {
    /// Path to the seatd socket.
    socket_path: String,
    /// Whether we are connected to seatd.
    connected: bool,
}

impl SeatdSeat {
    pub fn new(socket_path: String) -> Self {
        Self {
            socket_path,
            connected: false,
        }
    }

    /// Connect to the seatd daemon.
    pub fn connect(&mut self) -> Result<()> {
        // Stub: real implementation opens a Unix socket connection.
        let _ = &self.socket_path;
        self.connected = true;
        Ok(())
    }
}

impl SeatBackend for SeatdSeat {
    fn seat_info(&self) -> Result<SeatInfo> {
        if !self.connected {
            return Err(LogindError::SeatdConnection(
                "not connected".to_string(),
            ));
        }
        Ok(SeatInfo {
            id: "seat0".to_string(),
            can_multi_session: false,
            can_tty: true,
            can_graphical: true,
        })
    }

    fn take_device(&mut self, _major: u32, _minor: u32) -> Result<i32> {
        if !self.connected {
            return Err(LogindError::SeatdConnection(
                "not connected".to_string(),
            ));
        }
        // Stub: real implementation sends open_device to seatd
        Err(LogindError::NotSupported)
    }

    fn release_device(&mut self, _major: u32, _minor: u32) -> Result<()> {
        if !self.connected {
            return Err(LogindError::SeatdConnection(
                "not connected".to_string(),
            ));
        }
        // Stub: real implementation sends close_device to seatd
        Err(LogindError::NotSupported)
    }
}

/// Stub seat backend for testing and non-Linux platforms.
pub struct StubSeat;

impl StubSeat {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubSeat {
    fn default() -> Self {
        Self::new()
    }
}

impl SeatBackend for StubSeat {
    fn seat_info(&self) -> Result<SeatInfo> {
        Ok(SeatInfo {
            id: "seat0".to_string(),
            can_multi_session: false,
            can_tty: false,
            can_graphical: false,
        })
    }

    fn take_device(&mut self, _major: u32, _minor: u32) -> Result<i32> {
        Ok(-1)
    }

    fn release_device(&mut self, _major: u32, _minor: u32) -> Result<()> {
        Ok(())
    }
}

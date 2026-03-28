//! Central desktop manager — orchestrates window stations, desktops,
//! threads, and secure desktop switching.

use crate::desktop::{
    Desktop, DESKTOP_DEFAULT, DESKTOP_DISCONNECT, DESKTOP_SCREENSAVER, DESKTOP_WINLOGON,
};
use crate::error::DesktopError;
use crate::heap::{DEFAULT_HEAP_BUDGET, DEFAULT_INTERACTIVE_HEAP_BUDGET};
use crate::security::{DesktopAccess, DesktopFlags, DesktopSecurity};
use crate::station::WindowStation;
use crate::types::{DesktopId, WindowId, WindowStationId};
use std::collections::HashMap;

/// Central desktop management — creates and destroys stations and desktops,
/// tracks the active desktop, manages thread assignments, and handles
/// secure desktop switching.
#[derive(Debug)]
pub struct DesktopManager {
    /// All window stations, keyed by ID.
    stations: HashMap<WindowStationId, WindowStation>,
    /// All desktops, keyed by ID.
    desktops: HashMap<DesktopId, Desktop>,
    /// Currently active desktop (receives input and is displayed).
    active_desktop: Option<DesktopId>,
    /// Currently active station.
    active_station: Option<WindowStationId>,
    /// Desktop whose input is locked (secure desktop pattern).
    /// When set, only this desktop receives input and switch_desktop
    /// is blocked for non-secure desktops.
    input_locked_to: Option<DesktopId>,
    /// Access control.
    security: DesktopSecurity,
    /// Thread -> desktop assignments (authoritative; security mirrors this).
    thread_desktops: HashMap<u64, DesktopId>,
    /// Next station ID to allocate.
    next_station_id: u32,
    /// Next desktop ID to allocate.
    next_desktop_id: u32,
    /// Next window ID to allocate (for root windows).
    next_window_id: u32,
}

impl DesktopManager {
    /// Creates an empty manager with no stations or desktops.
    pub fn new() -> Self {
        Self {
            stations: HashMap::new(),
            desktops: HashMap::new(),
            active_desktop: None,
            active_station: None,
            input_locked_to: None,
            security: DesktopSecurity::new(),
            thread_desktops: HashMap::new(),
            next_station_id: 1,
            next_desktop_id: 1,
            next_window_id: 1,
        }
    }

    // ---------------------------------------------------------------
    // ID allocation
    // ---------------------------------------------------------------

    fn alloc_station_id(&mut self) -> WindowStationId {
        let id = WindowStationId(self.next_station_id);
        self.next_station_id += 1;
        id
    }

    fn alloc_desktop_id(&mut self) -> DesktopId {
        let id = DesktopId(self.next_desktop_id);
        self.next_desktop_id += 1;
        id
    }

    fn alloc_window_id(&mut self) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    // ---------------------------------------------------------------
    // Window stations
    // ---------------------------------------------------------------

    /// Creates a new interactive window station.
    ///
    /// Returns `Err(StationNameExists)` if a station with the same name
    /// already exists.
    pub fn create_station(
        &mut self,
        name: &str,
        session_id: u32,
    ) -> Result<WindowStationId, DesktopError> {
        // Check for duplicate names.
        if self.stations.values().any(|s| s.name == name) {
            return Err(DesktopError::StationNameExists(name.to_string()));
        }

        let id = self.alloc_station_id();
        let station = WindowStation::new(id, name.to_string(), session_id);
        self.stations.insert(id, station);

        // First station becomes active.
        if self.active_station.is_none() {
            self.active_station = Some(id);
        }

        Ok(id)
    }

    /// Creates a new non-interactive window station (for services).
    pub fn create_non_interactive_station(
        &mut self,
        name: &str,
        session_id: u32,
    ) -> Result<WindowStationId, DesktopError> {
        if self.stations.values().any(|s| s.name == name) {
            return Err(DesktopError::StationNameExists(name.to_string()));
        }

        let id = self.alloc_station_id();
        let station = WindowStation::new_non_interactive(id, name.to_string(), session_id);
        self.stations.insert(id, station);
        Ok(id)
    }

    /// Closes a window station, destroying all its desktops and unassigning
    /// all threads.
    pub fn close_station(&mut self, id: WindowStationId) -> Result<(), DesktopError> {
        let station = self
            .stations
            .get(&id)
            .ok_or(DesktopError::StationNotFound(id))?;

        let desktop_ids: Vec<DesktopId> = station.desktops.clone();

        // Close all desktops in this station.
        for did in desktop_ids {
            self.close_desktop_inner(did);
        }

        self.stations.remove(&id);

        // If this was the active station, pick another (if any).
        if self.active_station == Some(id) {
            self.active_station = self.stations.keys().next().copied();
            // Active desktop must also move.
            if let Some(sid) = self.active_station {
                self.active_desktop = self
                    .stations
                    .get(&sid)
                    .and_then(|s| s.desktops.first().copied());
            } else {
                self.active_desktop = None;
            }
        }

        Ok(())
    }

    /// Returns a reference to a station by ID.
    pub fn station(&self, id: WindowStationId) -> Option<&WindowStation> {
        self.stations.get(&id)
    }

    /// Returns a mutable reference to a station by ID.
    pub fn station_mut(&mut self, id: WindowStationId) -> Option<&mut WindowStation> {
        self.stations.get_mut(&id)
    }

    /// Enumerates all window station IDs.
    pub fn enum_stations(&self) -> Vec<WindowStationId> {
        self.stations.keys().copied().collect()
    }

    /// Returns the currently active station.
    pub fn active_station(&self) -> Option<WindowStationId> {
        self.active_station
    }

    // ---------------------------------------------------------------
    // Desktops
    // ---------------------------------------------------------------

    /// Creates a new desktop within a station.
    ///
    /// Allocates a root window for the desktop and registers it with the
    /// given heap budget.
    pub fn create_desktop(
        &mut self,
        station_id: WindowStationId,
        name: &str,
    ) -> Result<DesktopId, DesktopError> {
        self.create_desktop_with_budget(station_id, name, DEFAULT_INTERACTIVE_HEAP_BUDGET)
    }

    /// Creates a desktop with a custom heap budget.
    pub fn create_desktop_with_budget(
        &mut self,
        station_id: WindowStationId,
        name: &str,
        heap_budget: usize,
    ) -> Result<DesktopId, DesktopError> {
        let station = self
            .stations
            .get(&station_id)
            .ok_or(DesktopError::StationNotFound(station_id))?;

        // Check for duplicate names within this station.
        for &did in &station.desktops {
            if let Some(d) = self.desktops.get(&did) {
                if d.name == name {
                    return Err(DesktopError::DesktopNameExists {
                        station: station_id,
                        name: name.to_string(),
                    });
                }
            }
        }

        let desktop_id = self.alloc_desktop_id();
        let root_window = self.alloc_window_id();

        let desktop = Desktop::new(desktop_id, name.to_string(), station_id, root_window, heap_budget);
        self.desktops.insert(desktop_id, desktop);

        // Register with station.
        self.stations
            .get_mut(&station_id)
            .expect("station just verified")
            .desktops
            .push(desktop_id);

        // First desktop on the active station becomes active.
        if self.active_station == Some(station_id) && self.active_desktop.is_none() {
            self.active_desktop = Some(desktop_id);
            if let Some(d) = self.desktops.get_mut(&desktop_id) {
                d.flags |= DesktopFlags::ACTIVE;
            }
        }

        Ok(desktop_id)
    }

    /// Creates a secure desktop (for login, UAC prompts, lock screen).
    ///
    /// Secure desktops are flagged with `SECURE` and get a larger heap.
    pub fn create_secure_desktop(
        &mut self,
        station_id: WindowStationId,
        name: &str,
    ) -> Result<DesktopId, DesktopError> {
        let id = self.create_desktop_with_budget(station_id, name, DEFAULT_HEAP_BUDGET)?;

        if let Some(d) = self.desktops.get_mut(&id) {
            d.flags |= DesktopFlags::SECURE;
        }

        Ok(id)
    }

    /// Closes a desktop, destroying all its windows and unassigning threads.
    pub fn close_desktop(&mut self, id: DesktopId) -> Result<(), DesktopError> {
        if !self.desktops.contains_key(&id) {
            return Err(DesktopError::DesktopNotFound(id));
        }
        self.close_desktop_inner(id);
        Ok(())
    }

    /// Internal close — assumes the desktop exists. Used by close_station too.
    fn close_desktop_inner(&mut self, id: DesktopId) {
        // Unassign all threads from this desktop.
        let threads_to_remove: Vec<u64> = self
            .thread_desktops
            .iter()
            .filter(|(_, did)| **did == id)
            .map(|(tid, _)| *tid)
            .collect();

        for tid in threads_to_remove {
            self.thread_desktops.remove(&tid);
            self.security.remove_thread(tid);
        }

        // Remove from station.
        if let Some(desktop) = self.desktops.get(&id) {
            let sid = desktop.station_id;
            if let Some(station) = self.stations.get_mut(&sid) {
                station.desktops.retain(|&d| d != id);
            }
        }

        // If this was the active desktop, pick another.
        if self.active_desktop == Some(id) {
            self.active_desktop = None;
            if let Some(sid) = self.active_station {
                if let Some(station) = self.stations.get(&sid) {
                    self.active_desktop = station.desktops.first().copied();
                    if let Some(new_active) = self.active_desktop {
                        if let Some(d) = self.desktops.get_mut(&new_active) {
                            d.flags |= DesktopFlags::ACTIVE;
                        }
                    }
                }
            }
        }

        // If input was locked to this desktop, unlock.
        if self.input_locked_to == Some(id) {
            self.input_locked_to = None;
        }

        self.desktops.remove(&id);
    }

    /// Returns a reference to a desktop by ID.
    pub fn desktop(&self, id: DesktopId) -> Option<&Desktop> {
        self.desktops.get(&id)
    }

    /// Returns a mutable reference to a desktop by ID.
    pub fn desktop_mut(&mut self, id: DesktopId) -> Option<&mut Desktop> {
        self.desktops.get_mut(&id)
    }

    /// Enumerates all desktop IDs within a station.
    pub fn enum_desktops(
        &self,
        station_id: WindowStationId,
    ) -> Result<Vec<DesktopId>, DesktopError> {
        let station = self
            .stations
            .get(&station_id)
            .ok_or(DesktopError::StationNotFound(station_id))?;
        Ok(station.desktops.clone())
    }

    /// Returns the currently active desktop.
    pub fn active_desktop(&self) -> Option<DesktopId> {
        self.active_desktop
    }

    // ---------------------------------------------------------------
    // Desktop switching
    // ---------------------------------------------------------------

    /// Switches the active desktop. The new desktop must belong to the
    /// currently active station.
    ///
    /// If input is locked to a secure desktop, switching is blocked unless
    /// the target is the locked desktop itself.
    pub fn switch_desktop(&mut self, id: DesktopId) -> Result<(), DesktopError> {
        let desktop = self
            .desktops
            .get(&id)
            .ok_or(DesktopError::DesktopNotFound(id))?;

        // Must belong to the active station.
        if let Some(active_sid) = self.active_station {
            if desktop.station_id != active_sid {
                return Err(DesktopError::StationMismatch {
                    desktop: id,
                    expected_station: active_sid,
                    actual_station: desktop.station_id,
                });
            }
        }

        // Check input lock.
        if let Some(locked_id) = self.input_locked_to {
            if locked_id != id {
                return Err(DesktopError::InputLocked(locked_id));
            }
        }

        // Deactivate current.
        if let Some(old_id) = self.active_desktop {
            if let Some(old) = self.desktops.get_mut(&old_id) {
                old.flags.remove(DesktopFlags::ACTIVE);
            }
        }

        // Activate new.
        if let Some(new) = self.desktops.get_mut(&id) {
            new.flags |= DesktopFlags::ACTIVE;
        }
        self.active_desktop = Some(id);

        Ok(())
    }

    // ---------------------------------------------------------------
    // Secure desktop / input lock
    // ---------------------------------------------------------------

    /// Locks input to the specified desktop. Only this desktop will receive
    /// input events, and `switch_desktop` to any other desktop will fail
    /// with `InputLocked`.
    ///
    /// This is the secure desktop pattern: switch to a secure desktop, lock
    /// input, show prompt, then unlock and switch back.
    pub fn lock_input(&mut self, desktop_id: DesktopId) -> Result<(), DesktopError> {
        if !self.desktops.contains_key(&desktop_id) {
            return Err(DesktopError::DesktopNotFound(desktop_id));
        }

        // Mark this desktop as locked.
        if let Some(d) = self.desktops.get_mut(&desktop_id) {
            d.flags |= DesktopFlags::LOCKED;
        }

        self.input_locked_to = Some(desktop_id);
        Ok(())
    }

    /// Unlocks input, allowing desktop switching again. Clears the LOCKED
    /// flag on the previously locked desktop.
    pub fn unlock_input(&mut self) {
        if let Some(locked_id) = self.input_locked_to.take() {
            if let Some(d) = self.desktops.get_mut(&locked_id) {
                d.flags.remove(DesktopFlags::LOCKED);
            }
        }
    }

    /// Returns the desktop that input is currently locked to, if any.
    pub fn input_locked_desktop(&self) -> Option<DesktopId> {
        self.input_locked_to
    }

    // ---------------------------------------------------------------
    // Standard desktops
    // ---------------------------------------------------------------

    /// Creates the standard set of desktops for an interactive station:
    /// Default, Winlogon (secure), Screensaver, and Disconnect.
    ///
    /// Returns the four desktop IDs in order.
    pub fn create_standard_desktops(
        &mut self,
        station_id: WindowStationId,
    ) -> Result<[DesktopId; 4], DesktopError> {
        let default = self.create_desktop(station_id, DESKTOP_DEFAULT)?;
        let winlogon = self.create_secure_desktop(station_id, DESKTOP_WINLOGON)?;
        let screensaver = self.create_desktop(station_id, DESKTOP_SCREENSAVER)?;
        let disconnect = self.create_desktop(station_id, DESKTOP_DISCONNECT)?;

        Ok([default, winlogon, screensaver, disconnect])
    }

    // ---------------------------------------------------------------
    // Thread management
    // ---------------------------------------------------------------

    /// Assigns a thread to a desktop. The thread gains full access to that
    /// desktop.
    pub fn assign_thread(
        &mut self,
        thread_id: u64,
        desktop_id: DesktopId,
    ) -> Result<(), DesktopError> {
        if !self.desktops.contains_key(&desktop_id) {
            return Err(DesktopError::DesktopNotFound(desktop_id));
        }

        self.thread_desktops.insert(thread_id, desktop_id);
        self.security.assign_thread(thread_id, desktop_id);
        Ok(())
    }

    /// Removes a thread's desktop assignment.
    pub fn unassign_thread(&mut self, thread_id: u64) {
        self.thread_desktops.remove(&thread_id);
        self.security.remove_thread(thread_id);
    }

    /// Returns the desktop a thread is assigned to.
    pub fn desktop_for_thread(&self, thread_id: u64) -> Option<DesktopId> {
        self.thread_desktops.get(&thread_id).copied()
    }

    /// Returns all threads assigned to a desktop.
    pub fn threads_on_desktop(&self, desktop_id: DesktopId) -> Vec<u64> {
        self.thread_desktops
            .iter()
            .filter(|(_, did)| **did == desktop_id)
            .map(|(tid, _)| *tid)
            .collect()
    }

    // ---------------------------------------------------------------
    // Access control
    // ---------------------------------------------------------------

    /// Returns a reference to the security database.
    pub fn security(&self) -> &DesktopSecurity {
        &self.security
    }

    /// Returns a mutable reference to the security database.
    pub fn security_mut(&mut self) -> &mut DesktopSecurity {
        &mut self.security
    }

    /// Checks whether a thread has the required access to a desktop.
    pub fn check_access(
        &self,
        desktop_id: DesktopId,
        thread_id: u64,
        required: DesktopAccess,
    ) -> bool {
        self.security.check_access(desktop_id, thread_id, required)
    }

    // ---------------------------------------------------------------
    // Convenience queries
    // ---------------------------------------------------------------

    /// Returns the total number of stations.
    pub fn station_count(&self) -> usize {
        self.stations.len()
    }

    /// Returns the total number of desktops across all stations.
    pub fn desktop_count(&self) -> usize {
        self.desktops.len()
    }

    /// Finds a station by name.
    pub fn find_station_by_name(&self, name: &str) -> Option<WindowStationId> {
        self.stations
            .values()
            .find(|s| s.name == name)
            .map(|s| s.id)
    }

    /// Finds a desktop by name within a station.
    pub fn find_desktop_by_name(
        &self,
        station_id: WindowStationId,
        name: &str,
    ) -> Option<DesktopId> {
        let station = self.stations.get(&station_id)?;
        for &did in &station.desktops {
            if let Some(d) = self.desktops.get(&did) {
                if d.name == name {
                    return Some(did);
                }
            }
        }
        None
    }
}

impl Default for DesktopManager {
    fn default() -> Self {
        Self::new()
    }
}

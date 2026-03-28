#![allow(dead_code)]

use crate::{
    AccessPoint, ConnectivityState, InterfaceId, NetworkBackend, NetworkError,
    NetworkEvent, NetworkInterface, VpnConnection,
};

/// Stub network manager for unsupported platforms.
/// Returns empty lists and `NotSupported` errors.
pub struct NetworkManager;

impl NetworkManager {
    pub fn new() -> Self {
        Self
    }
}

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        Vec::new()
    }

    fn get_interface(&self, _id: &InterfaceId) -> Option<NetworkInterface> {
        None
    }

    fn scan_wifi(&mut self) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn get_access_points(&self) -> Vec<AccessPoint> {
        Vec::new()
    }

    fn connect_wifi(&mut self, _ssid: &str, _password: Option<&str>) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn disconnect_wifi(&mut self, _interface_id: &InterfaceId) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn forget_wifi(&mut self, _ssid: &str) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn enable_interface(&mut self, _id: &InterfaceId) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn disable_interface(&mut self, _id: &InterfaceId) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        Vec::new()
    }

    fn connect_vpn(&mut self, _id: &str) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn disconnect_vpn(&mut self, _id: &str) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn check_connectivity(&self) -> ConnectivityState {
        ConnectivityState::None
    }

    fn is_airplane_mode(&self) -> bool {
        false
    }

    fn set_airplane_mode(&mut self, _enabled: bool) -> Result<(), NetworkError> {
        Err(NetworkError::NotSupported)
    }

    fn poll_events(&mut self) -> Vec<NetworkEvent> {
        Vec::new()
    }
}

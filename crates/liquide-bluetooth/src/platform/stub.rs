#![allow(dead_code)]

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
};

/// Stub Bluetooth manager for unsupported platforms or testing.
/// Returns empty lists and `AdapterNotFound` errors.
pub struct BluetoothManager;

impl BluetoothManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothBackend for BluetoothManager {
    fn adapters(&self) -> Vec<BluetoothAdapter> {
        Vec::new()
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        None
    }

    fn set_powered(&mut self, _adapter_id: &str, _enabled: bool) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn stop_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn set_discoverable(
        &mut self,
        _adapter_id: &str,
        _enabled: bool,
        _timeout_secs: u32,
    ) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        Vec::new()
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        Vec::new()
    }

    fn pair(&mut self, _address: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn unpair(&mut self, _address: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn connect(&mut self, _address: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn disconnect(&mut self, _address: &str) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn trust(&mut self, _address: &str, _trusted: bool) -> Result<(), BtError> {
        Err(BtError::AdapterNotFound)
    }

    fn device_info(&self, _address: &str) -> Option<BluetoothDevice> {
        None
    }

    fn device_audio_profiles(&self, _address: &str) -> Vec<AudioProfile> {
        Vec::new()
    }

    fn poll_events(&mut self) -> Vec<BluetoothEvent> {
        Vec::new()
    }
}

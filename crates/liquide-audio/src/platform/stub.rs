//! Stub audio backend — returns `NotSupported` for every operation.
//!
//! Used as a fallback on platforms without a native audio backend.

use crate::{
    AppAudioStream, AudioBackend, AudioDeviceInfo, AudioError, AudioEvent, CaptureHandle,
    DeviceId, DeviceType, Result, SystemSound, Volume,
};

/// Stub audio manager that does nothing.
pub struct AudioManager;

impl AudioManager {
    /// Create a new stub audio manager.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for AudioManager {
    fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        Vec::new()
    }

    fn default_device(&self, _device_type: DeviceType) -> Option<DeviceId> {
        None
    }

    fn set_default_device(&mut self, _id: DeviceId) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn get_volume(&self, _device_id: DeviceId) -> Result<Volume> {
        Err(AudioError::NotSupported)
    }

    fn set_volume(&mut self, _device_id: DeviceId, _volume: Volume) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn list_streams(&self) -> Vec<AppAudioStream> {
        Vec::new()
    }

    fn set_stream_volume(&mut self, _stream_id: u64, _volume: Volume) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn move_stream_to_device(&mut self, _stream_id: u64, _device_id: DeviceId) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn play_system_sound(&mut self, _sound: SystemSound) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn start_capture(
        &mut self,
        _device_id: DeviceId,
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<CaptureHandle> {
        Err(AudioError::NotSupported)
    }

    fn read_capture(&mut self, _handle: &CaptureHandle, _buf: &mut [f32]) -> Result<usize> {
        Err(AudioError::NotSupported)
    }

    fn stop_capture(&mut self, _handle: CaptureHandle) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        Vec::new()
    }
}

//! Audio playback and microphone management on the client.

/// Playback pipeline state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioState {
    Disabled,
    Initializing,
    Playing,
    Muted,
    Error,
}

/// Microphone pipeline state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneState {
    Disabled,
    Ready,
    Capturing,
    PushToTalk,
    Muted,
}

/// Manages audio playback and microphone state.
pub struct AudioManager {
    playback_state: AudioState,
    mic_state: MicrophoneState,
    playback_volume: u8,
    selected_output: Option<String>,
    selected_input: Option<String>,
    codec: Option<String>,
}

impl AudioManager {
    /// Create a new audio manager in the disabled state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            playback_state: AudioState::Disabled,
            mic_state: MicrophoneState::Disabled,
            playback_volume: 100,
            selected_output: None,
            selected_input: None,
            codec: None,
        }
    }

    /// Set the playback volume (0..=100).
    pub fn set_playback_volume(&mut self, volume: u8) {
        self.playback_volume = volume.min(100);
        if self.playback_state == AudioState::Muted && volume > 0 {
            self.playback_state = AudioState::Playing;
        }
    }

    /// Mute playback.
    pub fn mute(&mut self) {
        if self.playback_state == AudioState::Playing {
            self.playback_state = AudioState::Muted;
        }
    }

    /// Unmute playback.
    pub fn unmute(&mut self) {
        if self.playback_state == AudioState::Muted {
            self.playback_state = AudioState::Playing;
        }
    }

    /// Whether playback is muted.
    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.playback_state == AudioState::Muted
    }

    /// Start microphone capture.
    pub fn start_microphone(&mut self) {
        match self.mic_state {
            MicrophoneState::Disabled | MicrophoneState::Ready => {
                self.mic_state = MicrophoneState::Capturing;
            }
            MicrophoneState::Muted => {
                self.mic_state = MicrophoneState::Capturing;
            }
            _ => {}
        }
    }

    /// Stop microphone capture.
    pub fn stop_microphone(&mut self) {
        if self.mic_state == MicrophoneState::Capturing
            || self.mic_state == MicrophoneState::PushToTalk
        {
            self.mic_state = MicrophoneState::Ready;
        }
    }

    /// Whether the microphone is actively capturing audio.
    #[must_use]
    pub fn is_mic_active(&self) -> bool {
        matches!(
            self.mic_state,
            MicrophoneState::Capturing | MicrophoneState::PushToTalk
        )
    }

    /// Select the audio output device by name.
    pub fn select_output_device(&mut self, name: String) {
        self.selected_output = Some(name);
    }

    /// Select the audio input device by name.
    pub fn select_input_device(&mut self, name: String) {
        self.selected_input = Some(name);
    }

    /// Current playback state.
    #[must_use]
    pub fn playback_state(&self) -> AudioState {
        self.playback_state
    }

    /// Current microphone state.
    #[must_use]
    pub fn mic_state(&self) -> MicrophoneState {
        self.mic_state
    }

    /// Enable audio playback (transition from Disabled to Initializing/Playing).
    pub fn enable_playback(&mut self) {
        if self.playback_state == AudioState::Disabled {
            self.playback_state = AudioState::Initializing;
            self.playback_state = AudioState::Playing;
        }
    }

    /// Disable audio playback.
    pub fn disable_playback(&mut self) {
        self.playback_state = AudioState::Disabled;
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

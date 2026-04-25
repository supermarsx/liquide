//! Client runtime coordinator — wires together every subsystem.

use crate::audio::AudioManager;
use crate::audit::ClientAuditEvent;
use crate::clipboard::ClipboardSync;
use crate::color::ColorPipeline;
use crate::config::ClientConfig;
use crate::connection::{ConnectionManager, ConnectionQuality, ConnectionState};
use crate::crash_screen::{CrashData, CrashScreen, CrashScreenType};
use crate::credential::{CredentialStore, StorageMode};
use crate::cursor::CursorPredictor;
use crate::decoder::{DecodedFrame, FrameQueue};
use crate::display::DisplayManager;
use crate::input::InputManager;
use crate::machine::MachineManager;
use crate::overlay::StreamOverlay;
use crate::{ClientError, Result};

/// Central coordinator for the LiquiDE desktop client.
///
/// Owns every subsystem and exposes high-level operations that the
/// main binary event loop can drive.
pub struct ClientRuntime {
    config: ClientConfig,
    connection_manager: ConnectionManager,
    display_manager: DisplayManager,
    input_manager: InputManager,
    cursor_predictor: CursorPredictor,
    frame_queue: FrameQueue,
    overlay: StreamOverlay,
    machine_manager: MachineManager,
    clipboard_sync: ClipboardSync,
    audio_manager: AudioManager,
    crash_screen: CrashScreen,
    color_pipeline: ColorPipeline,
    credential_store: CredentialStore,
    audit_events: Vec<ClientAuditEvent>,
}

impl ClientRuntime {
    /// Build a new runtime from the given configuration.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        let connection_manager = ConnectionManager::new(config.reconnection.max_attempts);
        let display_manager = DisplayManager::new(config.display.default_mode);
        let input_manager = InputManager::new(config.input.capture_scope, config.input.ime_mode);
        let cursor_predictor = CursorPredictor::new(
            config.cursor.correction_frames,
            config.cursor.smoothing_strategy,
        );
        let frame_queue = FrameQueue::new(config.performance.frame_queue_depth as usize);
        let overlay = StreamOverlay::new();
        let machine_manager = MachineManager::new();
        let clipboard_sync =
            ClipboardSync::new(config.clipboard.mode, config.clipboard.max_history as usize);
        let audio_manager = AudioManager::new();
        let crash_screen = CrashScreen::new();
        let color_pipeline = ColorPipeline::new();
        let credential_store = CredentialStore::new(StorageMode::OsKeychain);

        Self {
            config,
            connection_manager,
            display_manager,
            input_manager,
            cursor_predictor,
            frame_queue,
            overlay,
            machine_manager,
            clipboard_sync,
            audio_manager,
            crash_screen,
            color_pipeline,
            credential_store,
            audit_events: Vec::new(),
        }
    }

    // -- Connection lifecycle ------------------------------------------------

    /// Connect to a remote server.
    pub async fn connect(&mut self, server: &str) -> Result<()> {
        self.audit_events.push(ClientAuditEvent::ConnectionAttempt {
            server: server.to_string(),
        });

        self.connection_manager.connect(server).await.map_err(|e| {
            ClientError::ConnectionFailed {
                server: server.to_string(),
                reason: e.to_string(),
            }
        })?;

        self.audit_events.push(ClientAuditEvent::Connected {
            server: server.to_string(),
            transport: self.config.transport.preferred.clone(),
        });

        // Enable audio if configured.
        if self.config.audio.enabled && self.config.audio.playback_enabled {
            self.audio_manager.enable_playback();
            self.audit_events.push(ClientAuditEvent::AudioStarted);
        }

        Ok(())
    }

    /// Disconnect from the current server.
    pub async fn disconnect(&mut self) {
        let server = "(current)".to_string();
        self.connection_manager.disconnect().await;
        self.audio_manager.disable_playback();
        self.audio_manager.stop_microphone();
        self.frame_queue.clear();
        self.cursor_predictor.reset();

        self.audit_events.push(ClientAuditEvent::Disconnected {
            server,
            reason: "user requested".to_string(),
        });
        self.audit_events.push(ClientAuditEvent::AudioStopped);
    }

    // -- Frame pipeline ------------------------------------------------------

    /// Accept a decoded frame into the queue.
    pub fn handle_frame(&mut self, frame: DecodedFrame) {
        self.frame_queue.push(frame);
    }

    /// Pop the next frame for presentation.
    pub fn present_frame(&mut self) -> Option<DecodedFrame> {
        self.frame_queue.pop()
    }

    // -- Input ---------------------------------------------------------------

    /// Record an input event being sent to the server.
    pub fn handle_input(&mut self, is_app_focused: bool) -> bool {
        self.input_manager.should_capture_key(is_app_focused)
    }

    // -- Cursor --------------------------------------------------------------

    /// Update the local cursor position.
    pub fn update_cursor(&mut self, local_x: f64, local_y: f64) {
        self.cursor_predictor.update_local(local_x, local_y);
    }

    /// Update the server-authoritative cursor position.
    pub fn update_server_cursor(&mut self, x: f64, y: f64) {
        self.cursor_predictor.update_server(x, y);
    }

    // -- Display mode --------------------------------------------------------

    /// Toggle the statistics overlay.
    pub fn toggle_overlay(&mut self) {
        self.overlay.toggle();
    }

    /// Toggle between windowed and fullscreen mode.
    pub fn toggle_fullscreen(&mut self) {
        let new_mode =
            if self.display_manager.current_mode() == crate::display::DisplayMode::Fullscreen {
                crate::display::DisplayMode::SingleWindow
            } else {
                crate::display::DisplayMode::Fullscreen
            };
        self.display_manager.set_mode(new_mode);
        self.audit_events
            .push(ClientAuditEvent::DisplayModeChanged {
                mode: new_mode.to_string(),
            });
    }

    /// Cycle through display modes.
    pub fn cycle_display_mode(&mut self) {
        let next = match self.display_manager.current_mode() {
            crate::display::DisplayMode::SingleWindow => crate::display::DisplayMode::Fullscreen,
            crate::display::DisplayMode::Fullscreen => crate::display::DisplayMode::Tabbed,
            crate::display::DisplayMode::Tabbed => crate::display::DisplayMode::MultiWindow,
            crate::display::DisplayMode::MultiWindow => crate::display::DisplayMode::Seamless,
            crate::display::DisplayMode::Seamless => crate::display::DisplayMode::SingleWindow,
        };
        self.display_manager.set_mode(next);
        self.audit_events
            .push(ClientAuditEvent::DisplayModeChanged {
                mode: next.to_string(),
            });
    }

    // -- Crash ---------------------------------------------------------------

    /// Show the crash screen with the given crash type and code.
    pub fn show_crash_screen(
        &mut self,
        crash_type: CrashScreenType,
        error_code: u32,
        description: &str,
    ) {
        let data = CrashData {
            crash_type,
            error_code,
            description: description.to_string(),
            stack_trace: None,
            session_id: None,
            user: None,
            uptime_seconds: None,
            crash_report_id: None,
            restart_available: crash_type == CrashScreenType::SessionCrash,
        };
        self.crash_screen.show(data);
        self.audit_events.push(ClientAuditEvent::CrashScreenShown {
            crash_type: format!("{crash_type:?}"),
        });
    }

    // -- Audit ---------------------------------------------------------------

    /// Drain all accumulated audit events.
    pub fn drain_audit_events(&mut self) -> Vec<ClientAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    // -- Accessors -----------------------------------------------------------

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.connection_manager.state()
    }

    /// Current connection quality.
    #[must_use]
    pub fn quality(&self) -> ConnectionQuality {
        self.connection_manager.quality()
    }

    /// Access the configuration.
    #[must_use]
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Mutable access to the connection manager.
    pub fn connection_manager_mut(&mut self) -> &mut ConnectionManager {
        &mut self.connection_manager
    }

    /// Mutable access to the display manager.
    pub fn display_manager_mut(&mut self) -> &mut DisplayManager {
        &mut self.display_manager
    }

    /// Mutable access to the input manager.
    pub fn input_manager_mut(&mut self) -> &mut InputManager {
        &mut self.input_manager
    }

    /// Mutable access to the machine manager.
    pub fn machine_manager_mut(&mut self) -> &mut MachineManager {
        &mut self.machine_manager
    }

    /// Mutable access to the clipboard sync.
    pub fn clipboard_sync_mut(&mut self) -> &mut ClipboardSync {
        &mut self.clipboard_sync
    }

    /// Mutable access to the audio manager.
    pub fn audio_manager_mut(&mut self) -> &mut AudioManager {
        &mut self.audio_manager
    }

    /// Mutable access to the crash screen.
    pub fn crash_screen_mut(&mut self) -> &mut CrashScreen {
        &mut self.crash_screen
    }

    /// Mutable access to the color pipeline.
    pub fn color_pipeline_mut(&mut self) -> &mut ColorPipeline {
        &mut self.color_pipeline
    }

    /// Mutable access to the credential store.
    pub fn credential_store_mut(&mut self) -> &mut CredentialStore {
        &mut self.credential_store
    }

    /// Mutable access to the overlay.
    pub fn overlay_mut(&mut self) -> &mut StreamOverlay {
        &mut self.overlay
    }
}

//! Configuration types for the LiquiDE desktop client.

use serde::{Deserialize, Serialize};

use crate::clipboard::ClipboardMode;
use crate::color::ToneMapper;
use crate::cursor::{CursorMode, SmoothingStrategy};
use crate::decoder::DecoderBackend;
use crate::display::{DisplayMode, MonitorStrategy};
use crate::input::{CaptureScope, ImeMode};

/// Log-level selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Clock format for the lock screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockFormat {
    H12,
    H24,
}

/// Lock-screen background style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockBackground {
    Blur,
    Solid,
    Screenshot,
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

/// General application settings.
#[derive(Debug, Clone)]
pub struct GeneralConfig {
    pub log_level: LogLevel,
    pub theme: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Info,
            theme: "auto".to_string(),
        }
    }
}

/// Window chrome and behaviour settings.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub custom_chrome: bool,
    pub title_format: String,
    pub show_latency_in_title: bool,
    pub show_status_indicator: bool,
    pub always_on_top: bool,
    pub start_maximized: bool,
    pub start_fullscreen: bool,
    pub remember_size: bool,
    pub remember_position: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            custom_chrome: true,
            title_format: "{app} \u{2014} {server}".to_string(),
            show_latency_in_title: true,
            show_status_indicator: true,
            always_on_top: false,
            start_maximized: false,
            start_fullscreen: false,
            remember_size: true,
            remember_position: true,
        }
    }
}

/// Display and monitor configuration.
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub default_mode: DisplayMode,
    pub multi_monitor: MonitorStrategy,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_mode: DisplayMode::SingleWindow,
            multi_monitor: MonitorStrategy::MatchLocal,
        }
    }
}

/// Fullscreen toolbar settings.
#[derive(Debug, Clone)]
pub struct FullscreenToolbarConfig {
    pub enabled: bool,
    pub auto_hide: bool,
    pub auto_hide_delay_ms: u32,
    pub position: String,
    pub opacity: f32,
    pub show_latency: bool,
    pub show_audio_controls: bool,
    pub show_monitor_selector: bool,
}

impl Default for FullscreenToolbarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_hide: true,
            auto_hide_delay_ms: 500,
            position: "top-center".to_string(),
            opacity: 0.9,
            show_latency: true,
            show_audio_controls: true,
            show_monitor_selector: true,
        }
    }
}

/// Cursor rendering and prediction settings.
#[derive(Debug, Clone)]
pub struct CursorConfig {
    pub mode: CursorMode,
    pub prediction_enabled: bool,
    pub correction_interpolation: bool,
    pub correction_frames: u32,
    pub smoothing_enabled: bool,
    pub smoothing_strategy: SmoothingStrategy,
    pub dual_local_dot_size: u32,
    pub dual_local_dot_opacity: f32,
    pub hide_on_idle: bool,
    pub hide_delay_ms: u32,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            mode: CursorMode::LocalPredict,
            prediction_enabled: true,
            correction_interpolation: true,
            correction_frames: 3,
            smoothing_enabled: true,
            smoothing_strategy: SmoothingStrategy::Spring,
            dual_local_dot_size: 8,
            dual_local_dot_opacity: 0.6,
            hide_on_idle: true,
            hide_delay_ms: 5000,
        }
    }
}

/// Input capture settings.
#[derive(Debug, Clone)]
pub struct InputConfig {
    pub capture_scope: CaptureScope,
    pub release_key: String,
    pub ime_mode: ImeMode,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            capture_scope: CaptureScope::Application,
            release_key: "Ctrl+Alt+Shift".to_string(),
            ime_mode: ImeMode::Auto,
        }
    }
}

/// Clipboard sharing settings.
#[derive(Debug, Clone)]
pub struct ClipboardConfig {
    pub mode: ClipboardMode,
    pub text_enabled: bool,
    pub rich_text_enabled: bool,
    pub image_enabled: bool,
    pub image_max_size_mb: u32,
    pub max_history: u32,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            mode: ClipboardMode::Bidirectional,
            text_enabled: true,
            rich_text_enabled: true,
            image_enabled: true,
            image_max_size_mb: 10,
            max_history: 20,
        }
    }
}

/// Client-side audio settings.
#[derive(Debug, Clone)]
pub struct AudioClientConfig {
    pub enabled: bool,
    pub playback_enabled: bool,
    pub playback_volume: u8,
    pub microphone_enabled: bool,
    pub push_to_talk: bool,
    pub ptt_key: String,
    pub noise_suppression: bool,
    pub preferred_codecs: Vec<String>,
}

impl Default for AudioClientConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            playback_enabled: true,
            playback_volume: 100,
            microphone_enabled: false,
            push_to_talk: false,
            ptt_key: "F13".to_string(),
            noise_suppression: true,
            preferred_codecs: Vec::new(),
        }
    }
}

/// Transport negotiation settings.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub negotiation: String,
    pub preferred: String,
    pub fallback_order: Vec<String>,
    pub hybrid_enabled: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            negotiation: "auto".to_string(),
            preferred: "quic".to_string(),
            fallback_order: Vec::new(),
            hybrid_enabled: true,
        }
    }
}

/// Performance tuning settings.
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    pub decoder: DecoderBackend,
    pub max_decode_threads: u32,
    pub vsync: bool,
    pub frame_queue_depth: u32,
    pub adaptive_quality: bool,
    pub bandwidth_limit: u32,
    pub fps_limit: u32,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            decoder: DecoderBackend::Auto,
            max_decode_threads: 4,
            vsync: true,
            frame_queue_depth: 3,
            adaptive_quality: true,
            bandwidth_limit: 0,
            fps_limit: 0,
        }
    }
}

/// Reconnection behaviour settings.
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    pub auto_reconnect: bool,
    pub max_attempts: u32,
    pub initial_delay_ms: u32,
    pub max_delay_ms: u32,
    pub show_last_frame: bool,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            max_attempts: 0,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            show_last_frame: true,
        }
    }
}

/// Session thumbnail settings.
#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    pub enabled: bool,
    pub capture_on_disconnect: bool,
    pub capture_on_lock: bool,
    pub blur_on_capture: bool,
    pub blur_radius: u32,
    pub format: String,
    pub quality: u32,
    pub max_width: u32,
    pub max_cache_mb: u32,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_on_disconnect: true,
            capture_on_lock: true,
            blur_on_capture: true,
            blur_radius: 8,
            format: "webp".to_string(),
            quality: 75,
            max_width: 480,
            max_cache_mb: 100,
        }
    }
}

/// Color and HDR settings.
#[derive(Debug, Clone)]
pub struct ColorConfig {
    pub hdr_enabled: bool,
    pub preferred_bit_depth: u8,
    pub force_srgb: bool,
    pub tone_map_local: ToneMapper,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            hdr_enabled: false,
            preferred_bit_depth: 8,
            force_srgb: true,
            tone_map_local: ToneMapper::Reinhard,
        }
    }
}

/// Lock screen display settings.
#[derive(Debug, Clone)]
pub struct LockScreenConfig {
    pub show_clock: bool,
    pub show_session_info: bool,
    pub show_user_avatar: bool,
    pub clock_format: ClockFormat,
    pub background: LockBackground,
}

impl Default for LockScreenConfig {
    fn default() -> Self {
        Self {
            show_clock: true,
            show_session_info: true,
            show_user_avatar: true,
            clock_format: ClockFormat::H24,
            background: LockBackground::Blur,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

/// Root configuration for the LiquiDE desktop client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub general: GeneralConfig,
    pub window: WindowConfig,
    pub display: DisplayConfig,
    pub fullscreen_toolbar: FullscreenToolbarConfig,
    pub cursor: CursorConfig,
    pub input: InputConfig,
    pub clipboard: ClipboardConfig,
    pub audio: AudioClientConfig,
    pub transport: TransportConfig,
    pub performance: PerformanceConfig,
    pub reconnection: ReconnectionConfig,
    pub thumbnail: ThumbnailConfig,
    pub color: ColorConfig,
    pub lock_screen: LockScreenConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            window: WindowConfig::default(),
            display: DisplayConfig::default(),
            fullscreen_toolbar: FullscreenToolbarConfig::default(),
            cursor: CursorConfig::default(),
            input: InputConfig::default(),
            clipboard: ClipboardConfig::default(),
            audio: AudioClientConfig::default(),
            transport: TransportConfig::default(),
            performance: PerformanceConfig::default(),
            reconnection: ReconnectionConfig::default(),
            thumbnail: ThumbnailConfig::default(),
            color: ColorConfig::default(),
            lock_screen: LockScreenConfig::default(),
        }
    }
}

/// Return the platform-specific configuration directory for the client.
///
/// Uses `dirs::config_dir()` appended with `liquide`.
#[must_use]
pub fn config_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("liquide"))
}

/// Return the default path for the client configuration file.
#[must_use]
pub fn default_config_path() -> Option<std::path::PathBuf> {
    config_dir().map(|p| p.join("client.toml"))
}

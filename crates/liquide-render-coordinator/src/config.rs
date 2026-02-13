//! Configuration for render coordinator

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the render coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Number of dedicated window render threads
    pub window_threads: usize,
    
    /// Enable dedicated dock render thread
    pub enable_dock: bool,
    
    /// Enable dedicated status bar render thread
    pub enable_statusbar: bool,
    
    /// Enable dedicated background render thread
    pub enable_background: bool,
    
    /// Enable dedicated wallpaper render thread
    pub enable_wallpaper: bool,
    
    /// Maximum queue size per thread
    pub queue_size: usize,
    
    /// Render timeout duration
    pub timeout: Duration,
    
    /// Enable vsync
    pub vsync: bool,
    
    /// Target frame rate (Hz)
    pub target_fps: u32,
    
    /// Enable frame pacing
    pub frame_pacing: bool,
    
    /// Priority boost for focused window
    pub focused_window_boost: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            window_threads: num_cpus::get().max(4),
            enable_dock: true,
            enable_statusbar: true,
            enable_background: true,
            enable_wallpaper: true,
            queue_size: 128,
            timeout: Duration::from_millis(16), // ~60 FPS
            vsync: true,
            target_fps: 60,
            frame_pacing: true,
            focused_window_boost: true,
        }
    }
}

impl RenderConfig {
    /// Create a new builder
    pub fn builder() -> RenderConfigBuilder {
        RenderConfigBuilder::default()
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.window_threads == 0 {
            return Err("window_threads must be > 0".to_string());
        }
        
        if self.queue_size == 0 {
            return Err("queue_size must be > 0".to_string());
        }
        
        if self.target_fps == 0 || self.target_fps > 1000 {
            return Err("target_fps must be between 1 and 1000".to_string());
        }
        
        Ok(())
    }
    
    /// Get frame duration based on target FPS
    pub fn frame_duration(&self) -> Duration {
        Duration::from_micros(1_000_000 / self.target_fps as u64)
    }
}

/// Builder for RenderConfig
#[derive(Debug, Default)]
pub struct RenderConfigBuilder {
    window_threads: Option<usize>,
    enable_dock: Option<bool>,
    enable_statusbar: Option<bool>,
    enable_background: Option<bool>,
    enable_wallpaper: Option<bool>,
    queue_size: Option<usize>,
    timeout: Option<Duration>,
    vsync: Option<bool>,
    target_fps: Option<u32>,
    frame_pacing: Option<bool>,
    focused_window_boost: Option<bool>,
}

impl RenderConfigBuilder {
    /// Set number of window render threads
    pub fn window_threads(mut self, threads: usize) -> Self {
        self.window_threads = Some(threads);
        self
    }
    
    /// Enable/disable dock rendering
    pub fn enable_dock(mut self, enable: bool) -> Self {
        self.enable_dock = Some(enable);
        self
    }
    
    /// Enable/disable status bar rendering
    pub fn enable_statusbar(mut self, enable: bool) -> Self {
        self.enable_statusbar = Some(enable);
        self
    }
    
    /// Enable/disable background rendering
    pub fn enable_background(mut self, enable: bool) -> Self {
        self.enable_background = Some(enable);
        self
    }
    
    /// Enable/disable wallpaper rendering
    pub fn enable_wallpaper(mut self, enable: bool) -> Self {
        self.enable_wallpaper = Some(enable);
        self
    }
    
    /// Set queue size
    pub fn queue_size(mut self, size: usize) -> Self {
        self.queue_size = Some(size);
        self
    }
    
    /// Set render timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    
    /// Enable/disable vsync
    pub fn vsync(mut self, enable: bool) -> Self {
        self.vsync = Some(enable);
        self
    }
    
    /// Set target FPS
    pub fn target_fps(mut self, fps: u32) -> Self {
        self.target_fps = Some(fps);
        self
    }
    
    /// Enable/disable frame pacing
    pub fn frame_pacing(mut self, enable: bool) -> Self {
        self.frame_pacing = Some(enable);
        self
    }
    
    /// Enable/disable focused window priority boost
    pub fn focused_window_boost(mut self, enable: bool) -> Self {
        self.focused_window_boost = Some(enable);
        self
    }
    
    /// Build the configuration
    pub fn build(self) -> RenderConfig {
        let default = RenderConfig::default();
        
        RenderConfig {
            window_threads: self.window_threads.unwrap_or(default.window_threads),
            enable_dock: self.enable_dock.unwrap_or(default.enable_dock),
            enable_statusbar: self.enable_statusbar.unwrap_or(default.enable_statusbar),
            enable_background: self.enable_background.unwrap_or(default.enable_background),
            enable_wallpaper: self.enable_wallpaper.unwrap_or(default.enable_wallpaper),
            queue_size: self.queue_size.unwrap_or(default.queue_size),
            timeout: self.timeout.unwrap_or(default.timeout),
            vsync: self.vsync.unwrap_or(default.vsync),
            target_fps: self.target_fps.unwrap_or(default.target_fps),
            frame_pacing: self.frame_pacing.unwrap_or(default.frame_pacing),
            focused_window_boost: self.focused_window_boost.unwrap_or(default.focused_window_boost),
        }
    }
}

// Add num_cpus dependency
#[allow(dead_code)]
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

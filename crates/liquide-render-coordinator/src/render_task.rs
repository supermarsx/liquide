//! Render task definitions and execution

use liquide_compositor::damage::DamageTile;
use liquide_compositor::scene::FlatNode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default tile size used for first-pass full-frame rendering.
pub const DEFAULT_TILE_SIZE: u32 = 64;

/// Priority level for render tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RenderPriority {
    /// Critical system UI (emergency)
    Critical = 0,
    /// Focused window (high priority)
    Focused = 1,
    /// Interactive elements (normal priority)
    Interactive = 2,
    /// Background windows (lower priority)
    Background = 3,
    /// Decorations and effects (low priority)
    Decorative = 4,
}

/// Kind of render task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderTaskKind {
    /// Render a window
    Window { window_id: u64, is_focused: bool },
    /// Render the dock/taskbar
    Dock,
    /// Render the status bar
    StatusBar,
    /// Render the desktop background
    Background,
    /// Render animated wallpaper
    Wallpaper { frame: u64 },
    /// Composite multiple layers
    Composite { layer_ids: Vec<u64> },
}

impl RenderTaskKind {
    /// Get default priority for this task kind
    pub fn default_priority(&self) -> RenderPriority {
        match self {
            RenderTaskKind::Window { is_focused, .. } => {
                if *is_focused {
                    RenderPriority::Focused
                } else {
                    RenderPriority::Background
                }
            }
            RenderTaskKind::Dock | RenderTaskKind::StatusBar => RenderPriority::Interactive,
            RenderTaskKind::Background | RenderTaskKind::Wallpaper { .. } => {
                RenderPriority::Decorative
            }
            RenderTaskKind::Composite { .. } => RenderPriority::Interactive,
        }
    }

    /// Check if this task can be batched with others
    pub fn is_batchable(&self) -> bool {
        matches!(
            self,
            RenderTaskKind::Background | RenderTaskKind::Wallpaper { .. }
        )
    }
}

/// Opaque render data
#[derive(Clone)]
pub struct RenderData {
    data: Arc<Vec<u8>>,
    format: RenderDataFormat,
}

impl RenderData {
    /// Create new render data
    pub fn new(data: Vec<u8>, format: RenderDataFormat) -> Self {
        Self {
            data: Arc::new(data),
            format,
        }
    }

    /// Get data reference
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get data format
    pub fn format(&self) -> RenderDataFormat {
        self.format
    }
}

impl std::fmt::Debug for RenderData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderData")
            .field("format", &self.format)
            .field("size", &self.data.len())
            .finish()
    }
}

/// Format of render data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderDataFormat {
    /// Raw RGBA8 pixel data
    Rgba8,
    /// Raw BGRA8 pixel data
    Bgra8,
    /// Compressed image data
    Compressed,
    /// GPU command buffer
    CommandBuffer,
}

impl RenderDataFormat {
    /// Whether this format can back a CPU framebuffer render target.
    pub fn is_cpu_framebuffer_format(self) -> bool {
        matches!(self, Self::Rgba8 | Self::Bgra8)
    }
}

/// Output target for a render task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderTarget {
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
    /// Tile size used for damage classification.
    pub tile_size: u32,
    /// CPU framebuffer pixel format.
    pub format: RenderDataFormat,
}

impl RenderTarget {
    /// Create a BGRA8 render target with the default tile size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tile_size: DEFAULT_TILE_SIZE,
            format: RenderDataFormat::Bgra8,
        }
    }

    /// Set tile size.
    pub fn with_tile_size(mut self, tile_size: u32) -> Self {
        self.tile_size = tile_size;
        self
    }

    /// Set pixel format.
    pub fn with_format(mut self, format: RenderDataFormat) -> Self {
        self.format = format;
        self
    }

    /// Validate this target can be rendered by the CPU path.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err(format!(
                "render target dimensions must be non-zero, got {}x{}",
                self.width, self.height
            ));
        }

        if self.tile_size == 0 {
            return Err("render target tile_size must be non-zero".to_string());
        }

        if !self.format.is_cpu_framebuffer_format() {
            return Err(format!(
                "render target format {:?} is not CPU-framebuffer renderable",
                self.format
            ));
        }

        Ok(())
    }
}

/// Flattened scene input for a render task.
#[derive(Clone)]
pub struct RenderScene {
    target: RenderTarget,
    nodes: Arc<Vec<FlatNode>>,
}

impl RenderScene {
    /// Create a scene using the default BGRA8 target.
    pub fn new(width: u32, height: u32, nodes: Vec<FlatNode>) -> Self {
        Self::with_target(RenderTarget::new(width, height), nodes)
    }

    /// Create a scene with an explicit target.
    pub fn with_target(target: RenderTarget, nodes: Vec<FlatNode>) -> Self {
        Self {
            target,
            nodes: Arc::new(nodes),
        }
    }

    /// Return the render target.
    pub fn target(&self) -> RenderTarget {
        self.target
    }

    /// Borrow flattened scene nodes.
    pub fn nodes(&self) -> &[FlatNode] {
        &self.nodes
    }
}

impl std::fmt::Debug for RenderScene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderScene")
            .field("target", &self.target)
            .field("nodes", &self.nodes.len())
            .finish()
    }
}

/// A render task to be executed
#[derive(Debug, Clone)]
pub struct RenderTask {
    /// Unique task ID
    pub id: u64,

    /// Task kind
    pub kind: RenderTaskKind,

    /// Task priority
    pub priority: RenderPriority,

    /// Render data
    pub data: Option<RenderData>,

    /// Flattened scene and render target for real renderer invocation.
    pub scene: Option<RenderScene>,

    /// Task creation timestamp
    pub created_at: Instant,

    /// Expected completion deadline
    pub deadline: Option<Instant>,
}

impl RenderTask {
    /// Create a new render task
    pub fn new(id: u64, kind: RenderTaskKind) -> Self {
        let priority = kind.default_priority();
        Self {
            id,
            kind,
            priority,
            data: None,
            scene: None,
            created_at: Instant::now(),
            deadline: None,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: RenderPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set render data
    pub fn with_data(mut self, data: RenderData) -> Self {
        self.data = Some(data);
        self
    }

    /// Set flattened scene input.
    pub fn with_scene(mut self, scene: RenderScene) -> Self {
        self.scene = Some(scene);
        self
    }

    /// Set deadline
    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = Some(self.created_at + deadline);
        self
    }

    /// Check if task has exceeded deadline
    pub fn is_overdue(&self) -> bool {
        self.deadline.map_or(false, |d| Instant::now() > d)
    }

    /// Get age of task
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Metadata describing a rendered frame returned from a task.
#[derive(Debug, Clone)]
pub struct RenderOutputMetadata {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes per row.
    pub stride: u32,
    /// Pixel format of the returned byte buffer.
    pub format: RenderDataFormat,
    /// Tile size used for damage classification.
    pub tile_size: u32,
    /// Damage tiles reported by the renderer for this pass.
    pub damage_tiles: Vec<DamageTile>,
}

/// Output from a completed render task
#[derive(Debug, Clone)]
pub struct RenderOutput {
    /// Task ID
    pub task_id: u64,

    /// Rendered data
    pub data: Option<RenderData>,

    /// Rendered frame metadata.
    pub metadata: Option<RenderOutputMetadata>,

    /// Render duration
    pub duration: Duration,

    /// Whether render was successful
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,
}

impl RenderOutput {
    /// Create a successful output
    pub fn success(task_id: u64, data: Option<RenderData>, duration: Duration) -> Self {
        Self {
            task_id,
            data,
            metadata: None,
            duration,
            success: true,
            error: None,
        }
    }

    /// Create a successful output with frame metadata.
    pub fn success_with_metadata(
        task_id: u64,
        data: RenderData,
        metadata: RenderOutputMetadata,
        duration: Duration,
    ) -> Self {
        Self {
            task_id,
            data: Some(data),
            metadata: Some(metadata),
            duration,
            success: true,
            error: None,
        }
    }

    /// Create a failed output
    pub fn failure(task_id: u64, duration: Duration, error: String) -> Self {
        Self {
            task_id,
            data: None,
            metadata: None,
            duration,
            success: false,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_priority_ordering() {
        assert!(RenderPriority::Critical < RenderPriority::Focused);
        assert!(RenderPriority::Focused < RenderPriority::Interactive);
        assert!(RenderPriority::Interactive < RenderPriority::Background);
        assert!(RenderPriority::Background < RenderPriority::Decorative);
    }

    #[test]
    fn test_task_default_priority() {
        let focused = RenderTaskKind::Window {
            window_id: 1,
            is_focused: true,
        };
        assert_eq!(focused.default_priority(), RenderPriority::Focused);

        let background = RenderTaskKind::Window {
            window_id: 2,
            is_focused: false,
        };
        assert_eq!(background.default_priority(), RenderPriority::Background);

        let dock = RenderTaskKind::Dock;
        assert_eq!(dock.default_priority(), RenderPriority::Interactive);
    }

    #[test]
    fn test_task_deadline() {
        let task =
            RenderTask::new(1, RenderTaskKind::Dock).with_deadline(Duration::from_millis(16));

        assert!(task.deadline.is_some());
        assert!(!task.is_overdue());
    }

    #[test]
    fn test_task_batchable() {
        assert!(RenderTaskKind::Background.is_batchable());
        assert!(RenderTaskKind::Wallpaper { frame: 0 }.is_batchable());
        assert!(!RenderTaskKind::Dock.is_batchable());
        assert!(
            !RenderTaskKind::Window {
                window_id: 1,
                is_focused: true
            }
            .is_batchable()
        );
    }

    #[test]
    fn test_render_output_success() {
        let output = RenderOutput::success(42, None, Duration::from_micros(100));
        assert_eq!(output.task_id, 42);
        assert!(output.success);
        assert!(output.error.is_none());
    }

    #[test]
    fn test_render_output_failure() {
        let output = RenderOutput::failure(99, Duration::from_micros(50), "boom".into());
        assert_eq!(output.task_id, 99);
        assert!(!output.success);
        assert_eq!(output.error.as_deref(), Some("boom"));
    }

    #[test]
    fn test_task_with_priority_override() {
        let task = RenderTask::new(1, RenderTaskKind::Dock).with_priority(RenderPriority::Critical);
        assert_eq!(task.priority, RenderPriority::Critical);
    }

    #[test]
    fn test_composite_default_priority() {
        let kind = RenderTaskKind::Composite {
            layer_ids: vec![1, 2, 3],
        };
        assert_eq!(kind.default_priority(), RenderPriority::Interactive);
    }

    #[test]
    fn test_statusbar_default_priority() {
        let kind = RenderTaskKind::StatusBar;
        assert_eq!(kind.default_priority(), RenderPriority::Interactive);
    }

    #[test]
    fn test_wallpaper_default_priority() {
        let kind = RenderTaskKind::Wallpaper { frame: 42 };
        assert_eq!(kind.default_priority(), RenderPriority::Decorative);
    }

    #[test]
    fn test_render_data_format() {
        let data = RenderData::new(vec![1, 2, 3, 4], RenderDataFormat::Rgba8);
        assert_eq!(data.format(), RenderDataFormat::Rgba8);
        assert_eq!(data.data().len(), 4);
    }

    #[test]
    fn test_render_target_validation() {
        assert!(RenderTarget::new(32, 24).validate().is_ok());
        assert!(RenderTarget::new(0, 24).validate().is_err());
        assert!(
            RenderTarget::new(32, 24)
                .with_tile_size(0)
                .validate()
                .is_err()
        );
        assert!(
            RenderTarget::new(32, 24)
                .with_format(RenderDataFormat::Compressed)
                .validate()
                .is_err()
        );
    }
}

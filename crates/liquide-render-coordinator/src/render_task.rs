//! Render task definitions and execution

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Output from a completed render task
#[derive(Debug, Clone)]
pub struct RenderOutput {
    /// Task ID
    pub task_id: u64,

    /// Rendered data
    pub data: Option<RenderData>,

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
}

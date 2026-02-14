//! DOM event types.

use liquide_dom::NodeId;
use serde::{Deserialize, Serialize};

/// A DOM event targeting a specific node.
#[derive(Debug, Clone)]
pub struct DomEvent {
    /// The target node.
    pub target: NodeId,
    /// The event kind.
    pub kind: DomEventKind,
    /// Propagation state.
    pub propagation: Propagation,
}

impl DomEvent {
    /// Create a new DOM event.
    pub fn new(target: NodeId, kind: DomEventKind) -> Self {
        Self {
            target,
            kind,
            propagation: Propagation::default(),
        }
    }
}

/// The kind of DOM event.
#[derive(Debug, Clone)]
pub enum DomEventKind {
    // Mouse events
    MouseDown {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    MouseUp {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    MouseMove {
        x: f32,
        y: f32,
    },
    Click {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    DoubleClick {
        x: f32,
        y: f32,
    },
    MouseEnter,
    MouseLeave,
    ContextMenu {
        x: f32,
        y: f32,
    },

    // Scroll
    Scroll {
        dx: f32,
        dy: f32,
    },

    // Keyboard events
    KeyDown {
        key: u32,
        modifiers: u32,
    },
    KeyUp {
        key: u32,
        modifiers: u32,
    },

    // Focus events
    Focus,
    Blur,

    // IME
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor: usize,
    },
    CompositionEnd {
        text: String,
    },

    // Touch
    TouchStart {
        id: u32,
        x: f32,
        y: f32,
    },
    TouchMove {
        id: u32,
        x: f32,
        y: f32,
    },
    TouchEnd {
        id: u32,
    },
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

/// Event propagation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagation {
    /// Continue propagation.
    Continue,
    /// Stop propagation (no more handlers).
    StopPropagation,
    /// Stop all propagation including current phase.
    StopImmediate,
    /// Prevent default action.
    PreventDefault,
}

impl Default for Propagation {
    fn default() -> Self {
        Propagation::Continue
    }
}

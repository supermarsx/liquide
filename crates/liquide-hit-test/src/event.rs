//! DOM event types.

use liquide_dom::NodeId;
use serde::{Deserialize, Serialize};

/// Event propagation phase (W3C DOM Events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventPhase {
    /// Not currently dispatching.
    None,
    /// Capture phase (root → target).
    Capturing,
    /// At the target element.
    #[default]
    AtTarget,
    /// Bubble phase (target → root).
    Bubbling,
}

/// A DOM event targeting a specific node.
#[derive(Debug, Clone)]
pub struct DomEvent {
    /// The target node (original target, doesn't change during propagation).
    pub target: NodeId,
    /// The current target during propagation (changes as event bubbles/captures).
    pub current_target: NodeId,
    /// The ancestor propagation path, **root-first** (i.e. `[root, …, parent]`),
    /// NOT including the target itself.
    ///
    /// Used by [`EventDispatcher::dispatch_events`] to drive W3C three-phase
    /// (capture → target → bubble) dispatch. The capture phase walks this path
    /// front-to-back; the bubble phase walks it back-to-front.
    pub event_path: Vec<NodeId>,
    /// The event kind.
    pub kind: DomEventKind,
    /// Propagation state.
    pub propagation: Propagation,
    /// Current event phase.
    pub phase: EventPhase,
    /// Whether the event bubbles up through the DOM tree.
    pub bubbles: bool,
    /// Whether the event can be cancelled with preventDefault.
    pub cancelable: bool,
    /// Whether preventDefault was called.
    pub default_prevented: bool,
}

impl DomEvent {
    /// Create a new DOM event.
    pub fn new(target: NodeId, kind: DomEventKind) -> Self {
        let bubbles = kind.bubbles();
        let cancelable = kind.cancelable();
        Self {
            target,
            current_target: target,
            event_path: Vec::new(),
            kind,
            propagation: Propagation::default(),
            phase: EventPhase::None,
            bubbles,
            cancelable,
            default_prevented: false,
        }
    }

    /// Stop event propagation.
    pub fn stop_propagation(&mut self) {
        self.propagation = Propagation::StopPropagation;
    }

    /// Stop immediate propagation (including other handlers on same target).
    pub fn stop_immediate_propagation(&mut self) {
        self.propagation = Propagation::StopImmediate;
    }

    /// Prevent the default action if the event is cancelable.
    pub fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true;
            // Note: prevent_default is independent of propagation —
            // it does NOT stop propagation (W3C DOM Events spec).
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
    TouchCancel {
        id: u32,
    },

    // Pointer Events (W3C Pointer Events API)
    PointerDown {
        pointer_id: u32,
        pointer_type: PointerType,
        button: MouseButton,
        x: f32,
        y: f32,
        pressure: f32,
        tilt_x: f32,
        tilt_y: f32,
        is_primary: bool,
    },
    PointerMove {
        pointer_id: u32,
        pointer_type: PointerType,
        x: f32,
        y: f32,
        pressure: f32,
        tilt_x: f32,
        tilt_y: f32,
        is_primary: bool,
    },
    PointerUp {
        pointer_id: u32,
        pointer_type: PointerType,
        button: MouseButton,
        x: f32,
        y: f32,
        is_primary: bool,
    },
    PointerCancel {
        pointer_id: u32,
        pointer_type: PointerType,
    },
    PointerEnter {
        pointer_id: u32,
        pointer_type: PointerType,
    },
    PointerLeave {
        pointer_id: u32,
        pointer_type: PointerType,
    },
    PointerOver {
        pointer_id: u32,
        pointer_type: PointerType,
        x: f32,
        y: f32,
    },
    PointerOut {
        pointer_id: u32,
        pointer_type: PointerType,
        x: f32,
        y: f32,
    },
    GotPointerCapture {
        pointer_id: u32,
    },
    LostPointerCapture {
        pointer_id: u32,
    },
}

/// Pointer input type (mouse, pen, touch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerType {
    Mouse,
    Pen,
    Touch,
    Unknown,
}

impl DomEventKind {
    /// Whether this event type bubbles through the DOM tree.
    pub fn bubbles(&self) -> bool {
        match self {
            // These events do NOT bubble.
            // Note: the `scroll` event does not bubble per the W3C UI Events spec
            // (unlike `wheel`, which does).
            DomEventKind::MouseEnter
            | DomEventKind::MouseLeave
            | DomEventKind::Focus
            | DomEventKind::Blur
            | DomEventKind::Scroll { .. }
            | DomEventKind::PointerEnter { .. }
            | DomEventKind::PointerLeave { .. }
            | DomEventKind::GotPointerCapture { .. }
            | DomEventKind::LostPointerCapture { .. } => false,
            // All other events bubble
            _ => true,
        }
    }

    /// Whether this event can be cancelled with preventDefault.
    pub fn cancelable(&self) -> bool {
        match self {
            // Non-cancelable events
            DomEventKind::MouseEnter
            | DomEventKind::MouseLeave
            | DomEventKind::Focus
            | DomEventKind::Blur
            | DomEventKind::Scroll { .. }
            | DomEventKind::PointerEnter { .. }
            | DomEventKind::PointerLeave { .. }
            | DomEventKind::PointerCancel { .. }
            | DomEventKind::TouchCancel { .. }
            | DomEventKind::GotPointerCapture { .. }
            | DomEventKind::LostPointerCapture { .. }
            | DomEventKind::CompositionStart
            | DomEventKind::CompositionUpdate { .. }
            | DomEventKind::CompositionEnd { .. } => false,
            // All other events are cancelable
            _ => true,
        }
    }
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

//! Input seat protocol (wl_seat / wl_pointer / wl_keyboard / wl_touch).
//!
//! A seat represents a group of input devices (pointer, keyboard, touch)
//! that logically belong together, typically corresponding to a single
//! user at a physical station.

use crate::protocol::ObjectId;
use bitflags::bitflags;

// ---------------------------------------------------------------------------
// SeatCapability
// ---------------------------------------------------------------------------

bitflags! {
    /// Capabilities of a seat.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SeatCapability: u32 {
        const POINTER  = 1 << 0;
        const KEYBOARD = 1 << 1;
        const TOUCH    = 1 << 2;
    }
}

// ---------------------------------------------------------------------------
// Seat
// ---------------------------------------------------------------------------

/// A logical input seat.
///
/// Corresponds to `wl_seat` in the Wayland protocol. Reports capabilities
/// and manages pointer, keyboard, and touch sub-objects.
#[derive(Debug)]
pub struct Seat {
    /// Protocol object ID.
    id: ObjectId,
    /// Human-readable seat name (e.g. "seat0").
    name: String,
    /// Current capabilities.
    capabilities: SeatCapability,
    /// Pointer sub-object, if capability is present.
    pointer: Option<Pointer>,
    /// Keyboard sub-object, if capability is present.
    keyboard: Option<Keyboard>,
    /// Touch sub-object, if capability is present.
    touch: Option<Touch>,
}

impl Seat {
    /// Create a new seat.
    pub fn new(id: ObjectId, name: impl Into<String>, capabilities: SeatCapability) -> Self {
        Self {
            id,
            name: name.into(),
            capabilities,
            pointer: if capabilities.contains(SeatCapability::POINTER) {
                Some(Pointer::new())
            } else {
                None
            },
            keyboard: if capabilities.contains(SeatCapability::KEYBOARD) {
                Some(Keyboard::new())
            } else {
                None
            },
            touch: if capabilities.contains(SeatCapability::TOUCH) {
                Some(Touch::new())
            } else {
                None
            },
        }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn capabilities(&self) -> SeatCapability {
        self.capabilities
    }

    pub fn pointer(&self) -> Option<&Pointer> {
        self.pointer.as_ref()
    }

    pub fn pointer_mut(&mut self) -> Option<&mut Pointer> {
        self.pointer.as_mut()
    }

    pub fn keyboard(&self) -> Option<&Keyboard> {
        self.keyboard.as_ref()
    }

    pub fn keyboard_mut(&mut self) -> Option<&mut Keyboard> {
        self.keyboard.as_mut()
    }

    pub fn touch(&self) -> Option<&Touch> {
        self.touch.as_ref()
    }

    pub fn touch_mut(&mut self) -> Option<&mut Touch> {
        self.touch.as_mut()
    }

    /// Update capabilities, creating or destroying sub-objects as needed.
    pub fn update_capabilities(&mut self, caps: SeatCapability) {
        if caps.contains(SeatCapability::POINTER) && self.pointer.is_none() {
            self.pointer = Some(Pointer::new());
        } else if !caps.contains(SeatCapability::POINTER) {
            self.pointer = None;
        }

        if caps.contains(SeatCapability::KEYBOARD) && self.keyboard.is_none() {
            self.keyboard = Some(Keyboard::new());
        } else if !caps.contains(SeatCapability::KEYBOARD) {
            self.keyboard = None;
        }

        if caps.contains(SeatCapability::TOUCH) && self.touch.is_none() {
            self.touch = Some(Touch::new());
        } else if !caps.contains(SeatCapability::TOUCH) {
            self.touch = None;
        }

        self.capabilities = caps;
    }
}

// ---------------------------------------------------------------------------
// Pointer events
// ---------------------------------------------------------------------------

/// Button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Released,
    Pressed,
}

/// Axis source (scroll origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisSource {
    Wheel,
    Finger,
    Continuous,
    WheelTilt,
}

/// Axis direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    VerticalScroll,
    HorizontalScroll,
}

/// A pointer event.
#[derive(Debug, Clone)]
pub enum PointerEvent {
    /// Pointer entered a surface.
    Enter {
        serial: u32,
        surface: ObjectId,
        x: f64,
        y: f64,
    },
    /// Pointer left a surface.
    Leave { serial: u32, surface: ObjectId },
    /// Pointer moved within the focused surface.
    Motion { time: u32, x: f64, y: f64 },
    /// A button was pressed or released.
    Button {
        serial: u32,
        time: u32,
        button: u32,
        state: ButtonState,
    },
    /// Scroll axis event.
    Axis { time: u32, axis: Axis, value: f64 },
    /// Indicates the source of an axis event.
    AxisSource { source: AxisSource },
    /// Axis stop (finger lifted from touchpad).
    AxisStop { time: u32, axis: Axis },
    /// Discrete axis step (wheel clicks).
    AxisDiscrete { axis: Axis, discrete: i32 },
    /// Frame boundary: all events up to this point belong together.
    Frame,
}

/// Pointer input sub-object.
#[derive(Debug)]
pub struct Pointer {
    /// Currently focused surface.
    focus: Option<ObjectId>,
    /// Last known position.
    position: (f64, f64),
    /// Accumulated events.
    events: Vec<PointerEvent>,
}

impl Pointer {
    pub fn new() -> Self {
        Self {
            focus: None,
            position: (0.0, 0.0),
            events: Vec::new(),
        }
    }

    pub fn focus(&self) -> Option<ObjectId> {
        self.focus
    }

    pub fn position(&self) -> (f64, f64) {
        self.position
    }

    pub fn events(&self) -> &[PointerEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Record a pointer-enter event.
    pub fn enter(&mut self, serial: u32, surface: ObjectId, x: f64, y: f64) {
        self.focus = Some(surface);
        self.position = (x, y);
        self.events.push(PointerEvent::Enter {
            serial,
            surface,
            x,
            y,
        });
    }

    /// Record a pointer-leave event.
    pub fn leave(&mut self, serial: u32, surface: ObjectId) {
        if self.focus == Some(surface) {
            self.focus = None;
        }
        self.events.push(PointerEvent::Leave { serial, surface });
    }

    /// Record a motion event.
    pub fn motion(&mut self, time: u32, x: f64, y: f64) {
        self.position = (x, y);
        self.events.push(PointerEvent::Motion { time, x, y });
    }

    /// Record a button event.
    pub fn button(&mut self, serial: u32, time: u32, button: u32, state: ButtonState) {
        self.events.push(PointerEvent::Button {
            serial,
            time,
            button,
            state,
        });
    }

    /// Record an axis (scroll) event.
    pub fn axis(&mut self, time: u32, axis: Axis, value: f64) {
        self.events.push(PointerEvent::Axis { time, axis, value });
    }

    /// Record a frame boundary.
    pub fn frame(&mut self) {
        self.events.push(PointerEvent::Frame);
    }
}

impl Default for Pointer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Keyboard events
// ---------------------------------------------------------------------------

/// Key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Released,
    Pressed,
}

/// Keymap format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapFormat {
    /// No keymap; keys are interpreted as raw keycodes.
    NoKeymap,
    /// XKB v1 keymap.
    XkbV1,
}

/// Modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Depressed (physically held) modifiers.
    pub mods_depressed: u32,
    /// Latched modifiers (applied on next key press).
    pub mods_latched: u32,
    /// Locked modifiers (e.g. caps lock).
    pub mods_locked: u32,
    /// Active keyboard group (layout).
    pub group: u32,
}

/// A keyboard event.
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    /// Keymap update.
    Keymap { format: KeymapFormat, size: u32 },
    /// Keyboard focus entered a surface.
    Enter {
        serial: u32,
        surface: ObjectId,
        keys: Vec<u32>,
    },
    /// Keyboard focus left a surface.
    Leave { serial: u32, surface: ObjectId },
    /// A key was pressed or released.
    Key {
        serial: u32,
        time: u32,
        key: u32,
        state: KeyState,
    },
    /// Modifier state changed.
    Modifiers(Modifiers),
    /// Repeat rate and delay.
    RepeatInfo { rate: i32, delay: i32 },
}

/// Keyboard input sub-object.
#[derive(Debug)]
pub struct Keyboard {
    /// Currently focused surface.
    focus: Option<ObjectId>,
    /// Current modifier state.
    modifiers: Modifiers,
    /// Repeat rate (keys per second, 0 = disabled).
    repeat_rate: i32,
    /// Repeat delay in milliseconds.
    repeat_delay: i32,
    /// Accumulated events.
    events: Vec<KeyboardEvent>,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            focus: None,
            modifiers: Modifiers::default(),
            repeat_rate: 25,
            repeat_delay: 600,
            events: Vec::new(),
        }
    }

    pub fn focus(&self) -> Option<ObjectId> {
        self.focus
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn repeat_rate(&self) -> i32 {
        self.repeat_rate
    }

    pub fn repeat_delay(&self) -> i32 {
        self.repeat_delay
    }

    pub fn events(&self) -> &[KeyboardEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Record a keymap event.
    pub fn keymap(&mut self, format: KeymapFormat, size: u32) {
        self.events.push(KeyboardEvent::Keymap { format, size });
    }

    /// Record keyboard focus entering a surface.
    pub fn enter(&mut self, serial: u32, surface: ObjectId, keys: Vec<u32>) {
        self.focus = Some(surface);
        self.events.push(KeyboardEvent::Enter {
            serial,
            surface,
            keys,
        });
    }

    /// Record keyboard focus leaving a surface.
    pub fn leave(&mut self, serial: u32, surface: ObjectId) {
        if self.focus == Some(surface) {
            self.focus = None;
        }
        self.events.push(KeyboardEvent::Leave { serial, surface });
    }

    /// Record a key press/release.
    pub fn key(&mut self, serial: u32, time: u32, key: u32, state: KeyState) {
        self.events.push(KeyboardEvent::Key {
            serial,
            time,
            key,
            state,
        });
    }

    /// Update and record modifier state.
    pub fn update_modifiers(&mut self, mods: Modifiers) {
        self.modifiers = mods;
        self.events.push(KeyboardEvent::Modifiers(mods));
    }

    /// Set repeat info.
    pub fn set_repeat_info(&mut self, rate: i32, delay: i32) {
        self.repeat_rate = rate;
        self.repeat_delay = delay;
        self.events.push(KeyboardEvent::RepeatInfo { rate, delay });
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Touch events
// ---------------------------------------------------------------------------

/// A touch event.
#[derive(Debug, Clone)]
pub enum TouchEvent {
    /// Touch point created.
    Down {
        serial: u32,
        time: u32,
        surface: ObjectId,
        id: i32,
        x: f64,
        y: f64,
    },
    /// Touch point removed.
    Up { serial: u32, time: u32, id: i32 },
    /// Touch point moved.
    Motion { time: u32, id: i32, x: f64, y: f64 },
    /// End of a logical touch event group.
    Frame,
    /// All active touch points cancelled (e.g. palm rejection).
    Cancel,
}

/// Touch input sub-object.
#[derive(Debug)]
pub struct Touch {
    /// Currently active touch point IDs.
    active_points: Vec<i32>,
    /// Accumulated events.
    events: Vec<TouchEvent>,
}

impl Touch {
    pub fn new() -> Self {
        Self {
            active_points: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn active_points(&self) -> &[i32] {
        &self.active_points
    }

    pub fn events(&self) -> &[TouchEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Record a touch-down event.
    pub fn down(&mut self, serial: u32, time: u32, surface: ObjectId, id: i32, x: f64, y: f64) {
        if !self.active_points.contains(&id) {
            self.active_points.push(id);
        }
        self.events.push(TouchEvent::Down {
            serial,
            time,
            surface,
            id,
            x,
            y,
        });
    }

    /// Record a touch-up event.
    pub fn up(&mut self, serial: u32, time: u32, id: i32) {
        self.active_points.retain(|p| *p != id);
        self.events.push(TouchEvent::Up { serial, time, id });
    }

    /// Record a touch-motion event.
    pub fn motion(&mut self, time: u32, id: i32, x: f64, y: f64) {
        self.events.push(TouchEvent::Motion { time, id, x, y });
    }

    /// Record a frame boundary.
    pub fn frame(&mut self) {
        self.events.push(TouchEvent::Frame);
    }

    /// Record a cancel event.
    pub fn cancel(&mut self) {
        self.active_points.clear();
        self.events.push(TouchEvent::Cancel);
    }
}

impl Default for Touch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_creation() {
        let seat = Seat::new(
            ObjectId(1),
            "seat0",
            SeatCapability::POINTER | SeatCapability::KEYBOARD,
        );
        assert_eq!(seat.id(), ObjectId(1));
        assert_eq!(seat.name(), "seat0");
        assert!(seat.pointer().is_some());
        assert!(seat.keyboard().is_some());
        assert!(seat.touch().is_none());
    }

    #[test]
    fn seat_all_capabilities() {
        let seat = Seat::new(
            ObjectId(1),
            "seat0",
            SeatCapability::POINTER | SeatCapability::KEYBOARD | SeatCapability::TOUCH,
        );
        assert!(seat.pointer().is_some());
        assert!(seat.keyboard().is_some());
        assert!(seat.touch().is_some());
    }

    #[test]
    fn seat_no_capabilities() {
        let seat = Seat::new(ObjectId(1), "seat0", SeatCapability::empty());
        assert!(seat.pointer().is_none());
        assert!(seat.keyboard().is_none());
        assert!(seat.touch().is_none());
    }

    #[test]
    fn seat_update_capabilities() {
        let mut seat = Seat::new(ObjectId(1), "seat0", SeatCapability::POINTER);
        assert!(seat.pointer().is_some());
        assert!(seat.keyboard().is_none());

        seat.update_capabilities(SeatCapability::POINTER | SeatCapability::KEYBOARD);
        assert!(seat.pointer().is_some());
        assert!(seat.keyboard().is_some());

        seat.update_capabilities(SeatCapability::KEYBOARD);
        assert!(seat.pointer().is_none());
        assert!(seat.keyboard().is_some());
    }

    #[test]
    fn pointer_enter_leave() {
        let mut ptr = Pointer::new();
        assert!(ptr.focus().is_none());

        ptr.enter(1, ObjectId(10), 100.0, 200.0);
        assert_eq!(ptr.focus(), Some(ObjectId(10)));
        assert_eq!(ptr.position(), (100.0, 200.0));

        ptr.leave(2, ObjectId(10));
        assert!(ptr.focus().is_none());
    }

    #[test]
    fn pointer_motion() {
        let mut ptr = Pointer::new();
        ptr.enter(1, ObjectId(10), 0.0, 0.0);
        ptr.motion(100, 50.5, 75.3);
        assert_eq!(ptr.position(), (50.5, 75.3));
    }

    #[test]
    fn pointer_button() {
        let mut ptr = Pointer::new();
        ptr.button(1, 100, 0x110, ButtonState::Pressed); // BTN_LEFT
        ptr.button(2, 200, 0x110, ButtonState::Released);

        assert_eq!(ptr.events().len(), 2);
        if let PointerEvent::Button { button, state, .. } = &ptr.events()[0] {
            assert_eq!(*button, 0x110);
            assert_eq!(*state, ButtonState::Pressed);
        } else {
            panic!("expected Button event");
        }
    }

    #[test]
    fn pointer_axis() {
        let mut ptr = Pointer::new();
        ptr.axis(100, Axis::VerticalScroll, -15.0);
        if let PointerEvent::Axis { axis, value, .. } = &ptr.events()[0] {
            assert_eq!(*axis, Axis::VerticalScroll);
            assert!((*value - (-15.0)).abs() < 0.001);
        } else {
            panic!("expected Axis event");
        }
    }

    #[test]
    fn pointer_frame() {
        let mut ptr = Pointer::new();
        ptr.motion(100, 10.0, 20.0);
        ptr.frame();
        assert_eq!(ptr.events().len(), 2);
        assert!(matches!(ptr.events()[1], PointerEvent::Frame));
    }

    #[test]
    fn pointer_clear_events() {
        let mut ptr = Pointer::new();
        ptr.motion(100, 10.0, 20.0);
        assert_eq!(ptr.events().len(), 1);
        ptr.clear_events();
        assert!(ptr.events().is_empty());
    }

    #[test]
    fn keyboard_enter_leave() {
        let mut kb = Keyboard::new();
        kb.enter(1, ObjectId(10), vec![30, 31]); // two keys held
        assert_eq!(kb.focus(), Some(ObjectId(10)));

        kb.leave(2, ObjectId(10));
        assert!(kb.focus().is_none());
    }

    #[test]
    fn keyboard_key_press_release() {
        let mut kb = Keyboard::new();
        kb.key(1, 100, 30, KeyState::Pressed); // key 30 = 'a' on many keymaps
        kb.key(2, 200, 30, KeyState::Released);

        assert_eq!(kb.events().len(), 2);
        if let KeyboardEvent::Key { key, state, .. } = &kb.events()[0] {
            assert_eq!(*key, 30);
            assert_eq!(*state, KeyState::Pressed);
        }
    }

    #[test]
    fn keyboard_modifiers() {
        let mut kb = Keyboard::new();
        let mods = Modifiers {
            mods_depressed: 1,
            mods_latched: 0,
            mods_locked: 2,
            group: 0,
        };
        kb.update_modifiers(mods);
        assert_eq!(kb.modifiers().mods_depressed, 1);
        assert_eq!(kb.modifiers().mods_locked, 2);
    }

    #[test]
    fn keyboard_repeat_info() {
        let mut kb = Keyboard::new();
        assert_eq!(kb.repeat_rate(), 25);
        assert_eq!(kb.repeat_delay(), 600);

        kb.set_repeat_info(30, 400);
        assert_eq!(kb.repeat_rate(), 30);
        assert_eq!(kb.repeat_delay(), 400);
    }

    #[test]
    fn keyboard_keymap() {
        let mut kb = Keyboard::new();
        kb.keymap(KeymapFormat::XkbV1, 4096);
        assert_eq!(kb.events().len(), 1);
        if let KeyboardEvent::Keymap { format, size } = &kb.events()[0] {
            assert_eq!(*format, KeymapFormat::XkbV1);
            assert_eq!(*size, 4096);
        }
    }

    #[test]
    fn touch_down_up() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        assert_eq!(touch.active_points(), &[0]);

        touch.up(2, 200, 0);
        assert!(touch.active_points().is_empty());
    }

    #[test]
    fn touch_multitouch() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        touch.down(2, 100, ObjectId(10), 1, 150.0, 175.0);
        assert_eq!(touch.active_points(), &[0, 1]);

        touch.up(3, 200, 0);
        assert_eq!(touch.active_points(), &[1]);
    }

    #[test]
    fn touch_motion() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        touch.motion(150, 0, 60.0, 85.0);
        assert_eq!(touch.events().len(), 2);
    }

    #[test]
    fn touch_frame() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        touch.frame();
        assert!(matches!(touch.events()[1], TouchEvent::Frame));
    }

    #[test]
    fn touch_cancel() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        touch.down(2, 100, ObjectId(10), 1, 150.0, 175.0);
        touch.cancel();
        assert!(touch.active_points().is_empty());
        assert!(matches!(touch.events().last().unwrap(), TouchEvent::Cancel));
    }

    #[test]
    fn touch_clear_events() {
        let mut touch = Touch::new();
        touch.down(1, 100, ObjectId(10), 0, 50.0, 75.0);
        touch.clear_events();
        assert!(touch.events().is_empty());
        // Active points should still be tracked
        assert_eq!(touch.active_points(), &[0]);
    }
}

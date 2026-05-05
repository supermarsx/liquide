//! Internal event loop that pumps platform events and drives one-widget
//! paint/present ticks.

use anyhow::Result;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::{
    KeyCode, KeyEvent as NativeKeyEvent, KeyState, Modifiers as NativeMods,
};
use liquide_input::mouse::{ButtonState, MouseEvent as NativeMouseEvent, ScrollAxis};
use liquide_platform::PlatformBackend;
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::{NativeWindowHandle, NativeWindowParams};
use liquide_ui_core::event::{Key as UiKey, Modifiers as UiMods, MouseButton as UiMB};
use liquide_ui_core::{Constraints, Event, Painter, widget::Widget};
use tracing::debug;

use crate::bootstrap::{AppCx, Size};

/// Aggregated statistics returned from a harness run.
#[derive(Debug, Default, Clone, Copy)]
pub struct FrameStats {
    /// Number of ticks that completed (whether or not they produced paint).
    pub frames: u32,
    /// Total paint commands recorded across all ticks.
    pub paint_commands: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameCapture {
    pub frame_index: u32,
    pub window: NativeWindowHandle,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub paint_commands: u64,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct AppRunReport {
    pub stats: FrameStats,
    pub window_handles: Vec<NativeWindowHandle>,
    /// Number of calls made to the platform presenter.
    pub present_attempt_count: u32,
    /// Number of presents accepted by the platform presenter.
    pub present_count: u32,
    /// Number of presenter calls rejected by the platform backend.
    pub present_error_count: u32,
    /// Last frame accepted by the platform presenter.
    pub last_present: Option<FrameCapture>,
    /// Most recent platform presenter error, if any.
    pub last_present_error: Option<String>,
}

/// Outcome of processing a single tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameResult {
    Ticked,
    Quit,
}

/// Owns the platform backend + window handle and runs ticks.
pub struct EventLoop {
    platform: Box<dyn PlatformBackend>,
    window: Option<NativeWindowHandle>,
    dirty: bool,
    stats: FrameStats,
    present_buffer: Vec<u8>,
    window_handles: Vec<NativeWindowHandle>,
    present_attempt_count: u32,
    present_count: u32,
    present_error_count: u32,
    last_present: Option<FrameCapture>,
    last_present_error: Option<String>,
}

impl EventLoop {
    pub fn new(platform: Box<dyn PlatformBackend>) -> Self {
        Self {
            platform,
            window: None,
            dirty: true,
            stats: FrameStats::default(),
            present_buffer: Vec::new(),
            window_handles: Vec::new(),
            present_attempt_count: 0,
            present_count: 0,
            present_error_count: 0,
            last_present: None,
            last_present_error: None,
        }
    }

    pub(crate) fn create_window(&mut self, cx: &AppCx) -> Result<NativeWindowHandle> {
        let params = NativeWindowParams {
            title: cx.display_name().to_string(),
            geometry: Rect::new(0.0, 0.0, cx.size.width as f32, cx.size.height as f32),
            window_type: "normal".to_string(),
            parent: None,
            app_id: cx.app_id().to_string(),
        };
        let handle = self
            .platform
            .window_host()
            .create_window(params)
            .map_err(|e| anyhow::anyhow!("create_window failed: {e}"))?;
        self.window = Some(handle);
        self.window_handles.push(handle);
        debug!(app_id = %cx.app_id(), window = handle.0, "app-harness: window created");
        Ok(handle)
    }

    fn pump_events(&mut self, cx: &mut AppCx, root: &mut dyn Widget) -> FrameResult {
        loop {
            let Some(ev) = self.platform.poll_event() else {
                break;
            };
            match ev {
                PlatformEvent::Quit | PlatformEvent::WindowCloseRequested { .. } => {
                    return FrameResult::Quit;
                }
                PlatformEvent::WindowResized { width, height, .. } => {
                    cx.size = Size::new(width, height);
                    let _ = root.handle_event(&Event::Resize {
                        width: width as f32,
                        height: height as f32,
                    });
                    self.dirty = true;
                }
                PlatformEvent::WindowRedraw { .. } => {
                    self.dirty = true;
                }
                PlatformEvent::MouseInput { event, .. } => {
                    if let Some(ui_ev) = translate_mouse(event) {
                        let _ = root.handle_event(&ui_ev);
                        self.dirty = true;
                    }
                }
                PlatformEvent::KeyInput { event, .. } => {
                    // IME toggle is a structural gate; a real composition
                    // bridge through `liquide-ime::ImeContext` is deferred
                    // to the follow-up app wiring. Key events are always
                    // forwarded to the widget tree so shortcuts and focus
                    // navigation work even while IME wiring matures.
                    let _ = cx.ime_enabled;
                    if let Some(ui_ev) = translate_key(event) {
                        let _ = root.handle_event(&ui_ev);
                        self.dirty = true;
                    }
                }
                PlatformEvent::FocusGained { .. } => {
                    let _ = root.handle_event(&Event::FocusIn);
                }
                PlatformEvent::FocusLost { .. } => {
                    let _ = root.handle_event(&Event::FocusOut);
                }
                _ => {}
            }
        }
        FrameResult::Ticked
    }

    /// Execute one full tick: drain pending events, relayout if dirty,
    /// paint, and present.
    pub fn tick_once(&mut self, cx: &mut AppCx, root: &mut dyn Widget) -> Result<FrameResult> {
        let pumped = self.pump_events(cx, root);
        if pumped == FrameResult::Quit {
            return Ok(FrameResult::Quit);
        }

        self.stats.frames = self.stats.frames.saturating_add(1);

        if !self.dirty {
            return Ok(FrameResult::Ticked);
        }

        let w = cx.size.width as f32;
        let h = cx.size.height as f32;

        // Measure + layout at the current window size.
        let _ = root.measure(&Constraints::tight(w, h), &cx.theme);
        root.layout(0.0, 0.0, w, h);

        // Paint into a fresh command buffer.
        let mut painter = Painter::new();
        root.paint(&mut painter, &cx.theme);
        let cmd_count = painter.commands().len() as u64;
        self.stats.paint_commands = self.stats.paint_commands.saturating_add(cmd_count);

        // Present. CPU rasterisation of `painter.commands()` into pixels
        // is the responsibility of `liquide-renderer-cpu`; the harness
        // hands a zero-filled buffer to the platform so the presenter
        // path is exercised on every tick. Downstream apps that need
        // real pixels can plug in a custom backend via
        // `AppBootstrap::with_platform` — the CPU renderer integration
        // lives outside this foundation crate and is expected to land
        // alongside the app wiring work.
        if let Some(handle) = self.window {
            let width = cx.size.width;
            let height = cx.size.height;
            let stride = width.saturating_mul(4);
            let needed = (stride as usize).saturating_mul(height as usize);
            if self.present_buffer.len() != needed {
                self.present_buffer = vec![0u8; needed];
            } else {
                self.present_buffer.fill(0);
            }
            self.present_attempt_count = self.present_attempt_count.saturating_add(1);
            match self.platform.present_frame(
                handle,
                &self.present_buffer,
                width,
                height,
                stride,
                PixelFormat::Bgra8,
            ) {
                Ok(()) => {
                    self.present_count = self.present_count.saturating_add(1);
                    self.last_present = Some(FrameCapture {
                        frame_index: self.stats.frames,
                        window: handle,
                        width,
                        height,
                        stride,
                        format: PixelFormat::Bgra8,
                        paint_commands: cmd_count,
                        pixels: self.present_buffer.clone(),
                    });
                }
                Err(error) => {
                    self.present_error_count = self.present_error_count.saturating_add(1);
                    self.last_present_error = Some(error.to_string());
                }
            }
        }

        self.dirty = false;
        Ok(FrameResult::Ticked)
    }

    pub fn run_until_quit(&mut self, cx: &mut AppCx, root: &mut dyn Widget) -> Result<()> {
        let _ = self.run_until_quit_with_report(cx, root)?;
        Ok(())
    }

    pub fn run_until_quit_with_report(
        &mut self,
        cx: &mut AppCx,
        root: &mut dyn Widget,
    ) -> Result<AppRunReport> {
        loop {
            match self.tick_once(cx, root)? {
                FrameResult::Quit => break,
                FrameResult::Ticked => {}
            }
        }
        self.shutdown();
        Ok(self.report())
    }

    pub fn run_for_frames(
        &mut self,
        cx: &mut AppCx,
        root: &mut dyn Widget,
        frames: u32,
    ) -> Result<FrameStats> {
        Ok(self.run_for_frames_with_report(cx, root, frames)?.stats)
    }

    pub fn run_for_frames_with_report(
        &mut self,
        cx: &mut AppCx,
        root: &mut dyn Widget,
        frames: u32,
    ) -> Result<AppRunReport> {
        for _ in 0..frames {
            self.dirty = true;
            if let FrameResult::Quit = self.tick_once(cx, root)? {
                break;
            }
        }
        self.shutdown();
        Ok(self.report())
    }

    fn shutdown(&mut self) {
        if let Some(handle) = self.window.take() {
            let _ = self.platform.window_host().destroy_window(handle);
        }
    }

    fn report(&self) -> AppRunReport {
        AppRunReport {
            stats: self.stats,
            window_handles: self.window_handles.clone(),
            present_attempt_count: self.present_attempt_count,
            present_count: self.present_count,
            present_error_count: self.present_error_count,
            last_present: self.last_present.clone(),
            last_present_error: self.last_present_error.clone(),
        }
    }
}

fn translate_mouse(ev: NativeMouseEvent) -> Option<Event> {
    match ev {
        NativeMouseEvent::Move { x, y } => Some(Event::MouseMove { x, y }),
        NativeMouseEvent::Button {
            button,
            state,
            x,
            y,
        } => {
            let b = match button {
                liquide_input::mouse::MouseButton::Left => UiMB::Left,
                liquide_input::mouse::MouseButton::Right => UiMB::Right,
                liquide_input::mouse::MouseButton::Middle => UiMB::Middle,
                // Back / Forward / Other aren't in ui-core's MouseButton;
                // drop them silently for now.
                _ => return None,
            };
            Some(match state {
                ButtonState::Pressed => Event::MouseDown { x, y, button: b },
                ButtonState::Released => Event::MouseUp { x, y, button: b },
            })
        }
        NativeMouseEvent::Scroll { axis, delta, .. } => Some(match axis {
            ScrollAxis::Vertical => Event::Scroll { dx: 0.0, dy: delta },
            ScrollAxis::Horizontal => Event::Scroll { dx: delta, dy: 0.0 },
        }),
        NativeMouseEvent::Enter { .. } => Some(Event::MouseEnter),
        NativeMouseEvent::Leave => Some(Event::MouseLeave),
    }
}

fn translate_key(ev: NativeKeyEvent) -> Option<Event> {
    let key = translate_keycode(ev.key)?;
    let mods = UiMods {
        shift: ev.modifiers.contains(NativeMods::SHIFT),
        ctrl: ev.modifiers.contains(NativeMods::CTRL),
        alt: ev.modifiers.contains(NativeMods::ALT),
        super_key: ev.modifiers.contains(NativeMods::SUPER),
    };
    Some(match ev.state {
        KeyState::Pressed | KeyState::Repeat => Event::KeyDown {
            key,
            modifiers: mods,
        },
        KeyState::Released => Event::KeyUp {
            key,
            modifiers: mods,
        },
    })
}

fn translate_keycode(kc: KeyCode) -> Option<UiKey> {
    Some(match kc {
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Tab => UiKey::Tab,
        KeyCode::Backspace => UiKey::Backspace,
        KeyCode::Delete => UiKey::Delete,
        KeyCode::Space => UiKey::Space,
        KeyCode::ArrowUp => UiKey::ArrowUp,
        KeyCode::ArrowDown => UiKey::ArrowDown,
        KeyCode::ArrowLeft => UiKey::ArrowLeft,
        KeyCode::ArrowRight => UiKey::ArrowRight,
        KeyCode::Home => UiKey::Home,
        KeyCode::End => UiKey::End,
        KeyCode::PageUp => UiKey::PageUp,
        KeyCode::PageDown => UiKey::PageDown,
        KeyCode::F1 => UiKey::F(1),
        KeyCode::F2 => UiKey::F(2),
        KeyCode::F3 => UiKey::F(3),
        KeyCode::F4 => UiKey::F(4),
        KeyCode::F5 => UiKey::F(5),
        KeyCode::F6 => UiKey::F(6),
        KeyCode::F7 => UiKey::F(7),
        KeyCode::F8 => UiKey::F(8),
        KeyCode::F9 => UiKey::F(9),
        KeyCode::F10 => UiKey::F(10),
        KeyCode::F11 => UiKey::F(11),
        KeyCode::F12 => UiKey::F(12),
        // Alphabetic / digit / symbol keys currently have no one-to-one
        // mapping in ui-core's simplified `Key` enum — they come in via
        // `Event::TextInput` from the platform's IME/keymap layer.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, MutexGuard};

    use crate::bootstrap::{AppBootstrap, Size};
    use liquide_compositor::pixel::PixelFormat;
    use liquide_input::keyboard::{
        KeyCode, KeyEvent as NativeKeyEvent, KeyState, Modifiers as NativeMods,
    };
    use liquide_input::mouse::{
        ButtonState, MouseButton as NativeMouseButton, MouseEvent as NativeMouseEvent,
    };
    use liquide_platform::event_loop::PlatformEvent;
    use liquide_platform::standalone::{
        StandaloneConfig, StandalonePlatform, StandaloneScriptHandle,
    };
    use liquide_platform::window_host::NativeWindowHandle;
    use liquide_platform::{NullPlatform, PlatformBackend, PlatformError, PlatformResult};
    use liquide_ui_core::color::UiColor;
    use liquide_ui_core::event::{
        EventResponse, Key as UiKey, Modifiers as UiMods, MouseButton as UiMB,
    };
    use liquide_ui_core::layout::LayoutResult;
    use liquide_ui_core::painter::Painter;
    use liquide_ui_core::theme::UiTheme;
    use liquide_ui_core::widget::{Widget, WidgetState};
    use liquide_ui_core::{Constraints, Event, WidgetId};

    #[derive(Debug, Clone, PartialEq)]
    enum RecordedEvent {
        Resize { width: u32, height: u32 },
        KeyDown { key: UiKey, modifiers: UiMods },
        KeyUp { key: UiKey, modifiers: UiMods },
        MouseMove { x: f32, y: f32 },
        MouseDown { x: f32, y: f32, button: UiMB },
        MouseUp { x: f32, y: f32, button: UiMB },
        FocusIn,
        FocusOut,
    }

    #[derive(Debug, Default)]
    struct RecordingState {
        events: Vec<RecordedEvent>,
        paint_calls: u32,
    }

    fn lock_state(state: &Arc<Mutex<RecordingState>>) -> MutexGuard<'_, RecordingState> {
        state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn scripted_window() -> NativeWindowHandle {
        NativeWindowHandle(1)
    }

    fn test_bootstrap() -> (
        AppBootstrap,
        StandaloneScriptHandle,
        Arc<Mutex<RecordingState>>,
    ) {
        let platform = StandalonePlatform::new(StandaloneConfig {
            width: 320,
            height: 240,
            hardware_cursor: false,
            ..StandaloneConfig::default()
        })
        .expect("standalone platform should construct for tests");
        let script = platform.script_handle();
        let state = Arc::new(Mutex::new(RecordingState::default()));
        let bootstrap = AppBootstrap::new("com.liquide.test.harness", "Harness Test")
            .with_initial_size(Size::new(320, 240))
            .with_platform(Box::new(platform));
        (bootstrap, script, state)
    }

    struct FailingPresentPlatform {
        inner: NullPlatform,
        attempts: Arc<Mutex<u32>>,
    }

    impl FailingPresentPlatform {
        fn new(attempts: Arc<Mutex<u32>>) -> Self {
            Self {
                inner: NullPlatform::new(),
                attempts,
            }
        }
    }

    impl PlatformBackend for FailingPresentPlatform {
        fn display(&self) -> &dyn liquide_platform::DisplayBackend {
            self.inner.display()
        }

        fn window_host(&mut self) -> &mut dyn liquide_platform::NativeWindowHost {
            self.inner.window_host()
        }

        fn taskbar(&mut self) -> &mut dyn liquide_platform::TaskbarIntegration {
            self.inner.taskbar()
        }

        fn tray(&mut self) -> &mut dyn liquide_platform::NativeTray {
            self.inner.tray()
        }

        fn notifications(&mut self) -> &mut dyn liquide_platform::NativeNotifications {
            self.inner.notifications()
        }

        fn drag_drop(&mut self) -> &mut dyn liquide_platform::NativeDragDrop {
            self.inner.drag_drop()
        }

        fn keymap(&self) -> &dyn liquide_platform::KeymapTranslator {
            self.inner.keymap()
        }

        fn platform_name(&self) -> &str {
            "failing-present"
        }

        fn present_frame(
            &mut self,
            _handle: NativeWindowHandle,
            _pixels: &[u8],
            _width: u32,
            _height: u32,
            _stride: u32,
            _format: PixelFormat,
        ) -> PlatformResult<()> {
            let mut attempts = self
                .attempts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *attempts = attempts.saturating_add(1);
            Err(PlatformError::Presentation(
                "scripted present failure".to_string(),
            ))
        }
    }

    struct RecordingWidget {
        state: WidgetState,
        shared: Arc<Mutex<RecordingState>>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    impl RecordingWidget {
        fn new(shared: Arc<Mutex<RecordingState>>) -> Self {
            Self {
                state: WidgetState::new(WidgetId::new()),
                shared,
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            }
        }
    }

    impl Widget for RecordingWidget {
        fn id(&self) -> WidgetId {
            self.state.id
        }
        fn visible(&self) -> bool {
            self.state.visible
        }
        fn set_visible(&mut self, v: bool) {
            self.state.visible = v;
        }
        fn enabled(&self) -> bool {
            self.state.enabled
        }
        fn set_enabled(&mut self, e: bool) {
            self.state.enabled = e;
        }
        fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
            let (w, h) = constraints.clamp(constraints.max_width, constraints.max_height);
            LayoutResult::new(w, h)
        }
        fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
            self.x = x;
            self.y = y;
            self.w = w;
            self.h = h;
        }
        fn paint(&self, painter: &mut Painter, _theme: &UiTheme) {
            let mut state = lock_state(&self.shared);
            state.paint_calls = state.paint_calls.saturating_add(1);
            painter.fill_rect(
                self.x,
                self.y,
                self.w,
                self.h,
                UiColor::new(32, 96, 200, 255),
            );
        }

        fn handle_event(&mut self, event: &Event) -> EventResponse {
            let recorded = match event {
                Event::Resize { width, height } => Some(RecordedEvent::Resize {
                    width: *width as u32,
                    height: *height as u32,
                }),
                Event::KeyDown { key, modifiers } => Some(RecordedEvent::KeyDown {
                    key: *key,
                    modifiers: *modifiers,
                }),
                Event::KeyUp { key, modifiers } => Some(RecordedEvent::KeyUp {
                    key: *key,
                    modifiers: *modifiers,
                }),
                Event::MouseMove { x, y } => Some(RecordedEvent::MouseMove { x: *x, y: *y }),
                Event::MouseDown { x, y, button } => Some(RecordedEvent::MouseDown {
                    x: *x,
                    y: *y,
                    button: *button,
                }),
                Event::MouseUp { x, y, button } => Some(RecordedEvent::MouseUp {
                    x: *x,
                    y: *y,
                    button: *button,
                }),
                Event::FocusIn => Some(RecordedEvent::FocusIn),
                Event::FocusOut => Some(RecordedEvent::FocusOut),
                _ => None,
            };

            if let Some(recorded) = recorded {
                lock_state(&self.shared).events.push(recorded);
            }
            EventResponse::Ignored
        }
    }

    #[test]
    fn frame_capture_report_matches_standalone_backend() {
        let (bootstrap, script, state) = test_bootstrap();

        let report = bootstrap
            .run_for_frames_with_report(2, |_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("harness should tick cleanly");

        assert_eq!(report.stats.frames, 2);
        assert_eq!(report.present_attempt_count, 2);
        assert_eq!(report.present_count, 2);
        assert_eq!(report.present_error_count, 0);
        assert!(report.last_present_error.is_none());

        let capture = report
            .last_present
            .as_ref()
            .expect("run should retain the last presented frame");
        assert_eq!(capture.window, scripted_window());
        assert_eq!(capture.width, 320);
        assert_eq!(capture.height, 240);
        assert_eq!(capture.pixels.len(), (320 * 240 * 4) as usize);
        assert!(capture.paint_commands >= 1);

        let backend_capture = script
            .last_presented_frame()
            .expect("standalone backend should retain the last present");
        assert_eq!(backend_capture.width, capture.width);
        assert_eq!(backend_capture.height, capture.height);
        assert_eq!(backend_capture.stride, capture.stride);
        assert_eq!(backend_capture.format, capture.format);
        assert_eq!(backend_capture.pixels, capture.pixels);
        assert_eq!(script.present_count(), u64::from(report.present_count));
        assert!(lock_state(&state).paint_calls >= 2);
    }

    #[test]
    fn failed_platform_present_is_not_counted_as_presented() {
        let attempts = Arc::new(Mutex::new(0u32));
        let platform = FailingPresentPlatform::new(Arc::clone(&attempts));
        let state = Arc::new(Mutex::new(RecordingState::default()));

        let report = AppBootstrap::new("com.liquide.test.harness", "Harness Test")
            .with_initial_size(Size::new(320, 240))
            .with_platform(Box::new(platform))
            .run_for_frames_with_report(1, |_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("present failure should be recorded in the report");

        assert_eq!(report.stats.frames, 1);
        assert_eq!(report.present_attempt_count, 1);
        assert_eq!(report.present_count, 0);
        assert_eq!(report.present_error_count, 1);
        assert!(report.last_present.is_none());
        assert!(
            report
                .last_present_error
                .as_deref()
                .is_some_and(|message| message.contains("scripted present failure"))
        );
        assert_eq!(
            *attempts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            1
        );
        assert_eq!(lock_state(&state).paint_calls, 1);
    }

    #[test]
    fn scripted_resize_updates_widget_and_capture() {
        let (bootstrap, script, state) = test_bootstrap();
        script.push_event(PlatformEvent::WindowResized {
            handle: scripted_window(),
            width: 640,
            height: 360,
        });

        let report = bootstrap
            .run_for_frames_with_report(1, |_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("scripted resize should complete");

        let events = lock_state(&state).events.clone();
        assert!(events.contains(&RecordedEvent::Resize {
            width: 640,
            height: 360,
        }));

        let capture = report
            .last_present
            .expect("resize run should present a frame");
        assert_eq!(capture.width, 640);
        assert_eq!(capture.height, 360);
    }

    #[test]
    fn scripted_key_input_reaches_widget() {
        let (bootstrap, script, state) = test_bootstrap();
        let modifiers = NativeMods::from_bits(NativeMods::CTRL);
        script.push_events([
            PlatformEvent::KeyInput {
                handle: scripted_window(),
                event: NativeKeyEvent::new(KeyCode::Enter, KeyState::Pressed, modifiers, 13, 1),
            },
            PlatformEvent::KeyInput {
                handle: scripted_window(),
                event: NativeKeyEvent::new(KeyCode::Enter, KeyState::Released, modifiers, 13, 2),
            },
        ]);

        let report = bootstrap
            .run_for_frames_with_report(1, |_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("scripted key input should complete");

        assert_eq!(report.stats.frames, 1);
        let events = lock_state(&state).events.clone();
        let ctrl_mods = UiMods {
            ctrl: true,
            ..UiMods::NONE
        };
        assert!(events.contains(&RecordedEvent::KeyDown {
            key: UiKey::Enter,
            modifiers: ctrl_mods,
        }));
        assert!(events.contains(&RecordedEvent::KeyUp {
            key: UiKey::Enter,
            modifiers: ctrl_mods,
        }));
    }

    #[test]
    fn scripted_mouse_input_reaches_widget() {
        let (bootstrap, script, state) = test_bootstrap();
        script.push_events([
            PlatformEvent::MouseInput {
                handle: scripted_window(),
                event: NativeMouseEvent::Move { x: 12.0, y: 18.0 },
            },
            PlatformEvent::MouseInput {
                handle: scripted_window(),
                event: NativeMouseEvent::Button {
                    button: NativeMouseButton::Left,
                    state: ButtonState::Pressed,
                    x: 12.0,
                    y: 18.0,
                },
            },
            PlatformEvent::MouseInput {
                handle: scripted_window(),
                event: NativeMouseEvent::Button {
                    button: NativeMouseButton::Left,
                    state: ButtonState::Released,
                    x: 12.0,
                    y: 18.0,
                },
            },
        ]);

        let report = bootstrap
            .run_for_frames_with_report(1, |_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("scripted mouse input should complete");

        assert_eq!(report.present_count, 1);
        let events = lock_state(&state).events.clone();
        assert!(events.contains(&RecordedEvent::MouseMove { x: 12.0, y: 18.0 }));
        assert!(events.contains(&RecordedEvent::MouseDown {
            x: 12.0,
            y: 18.0,
            button: UiMB::Left,
        }));
        assert!(events.contains(&RecordedEvent::MouseUp {
            x: 12.0,
            y: 18.0,
            button: UiMB::Left,
        }));
    }

    #[test]
    fn scripted_quit_stops_without_presenting() {
        let (bootstrap, script, state) = test_bootstrap();
        script.push_event(PlatformEvent::Quit);

        let report = bootstrap
            .run_with_report(|_cx| Box::new(RecordingWidget::new(state.clone())))
            .expect("quit should stop the harness cleanly");

        assert_eq!(report.stats.frames, 0);
        assert_eq!(report.present_count, 0);
        assert!(report.last_present.is_none());
        assert_eq!(lock_state(&state).paint_calls, 0);
    }

    #[test]
    fn spawn_window_is_stub() {
        let res = AppBootstrap::new("com.liquide.test.harness", "Harness Test")
            .with_platform(Box::new(NullPlatform::new()))
            .run_for_frames(1, |cx| {
                let err = cx.spawn_window("secondary").unwrap_err();
                assert!(err.to_string().contains("spawn_window"));
                Box::new(RecordingWidget::new(Arc::new(Mutex::new(
                    RecordingState::default(),
                ))))
            });
        assert!(res.is_ok());
    }
}

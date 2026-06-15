//! t70-s6: per-window live `AppView` registry, render, and input routing.
//!
//! These tests stay app-agnostic — they use a tiny in-test `FakeApp` that
//! implements [`liquide_interop::AppView`], proving the SHELL wiring (registry,
//! factory-on-open, generic content paint, keyboard forwarding, close cleanup)
//! without depending on any real app crate. The HOST-side construction of the
//! real apps is covered by `liquide-session`'s `app_views` tests.

use std::sync::{Arc, Mutex};

use liquide_compositor::scene::{SceneNode, SceneNodeKind};
use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::shell::Shell;
use crate::window::WindowId;
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

/// A minimal app model: it accumulates typed text and exposes it as one content
/// row, so the shell render + input wiring is observable end-to-end.
#[derive(Default)]
struct FakeAppState {
    buffer: String,
    keys: Vec<String>,
}

struct FakeApp {
    state: Arc<Mutex<FakeAppState>>,
}

impl FakeApp {
    fn new(state: Arc<Mutex<FakeAppState>>) -> Self {
        Self { state }
    }
}

impl AppTextInput for FakeApp {
    fn handle_text(&mut self, text: &str) -> bool {
        self.state.lock().unwrap().buffer.push_str(text);
        true
    }
    fn handle_key(&mut self, key: &AppKey) -> bool {
        self.state.lock().unwrap().keys.push(key.name().to_string());
        true
    }
}

impl AppContentProvider for FakeApp {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let s = self.state.lock().unwrap();
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some("FakeApp".to_string());
        view.rows.push(ContentRow::plain(format!("typed:{}", s.buffer)));
        view
    }
}

impl AppView for FakeApp {
    fn app_id(&self) -> &str {
        "com.liquide.fake"
    }
}

fn type_key(key: KeyCode) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(0),
        event: KeyEvent {
            key,
            state: KeyState::Pressed,
            modifiers: Modifiers::new(),
            scancode: 0,
            timestamp_us: 0,
        },
    }
}

fn collect_text(node: &SceneNode, out: &mut Vec<String>) {
    if let SceneNodeKind::Text { text, .. } = &node.kind {
        out.push(text.clone());
    }
    for c in &node.children {
        collect_text(c, out);
    }
}

fn scene_texts(shell: &mut Shell) -> Vec<String> {
    let scene = shell.build_scene();
    let mut out = Vec::new();
    collect_text(&scene, &mut out);
    out
}

#[test]
fn registered_app_view_paints_real_content_not_placeholder() {
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let state = Arc::new(Mutex::new(FakeAppState::default()));
    shell.register_app_view(id, Box::new(FakeApp::new(state)));
    assert!(shell.has_app_view(id));

    let texts = scene_texts(&mut shell);
    // The app's real content (title + row) is painted...
    assert!(texts.iter().any(|t| t == "FakeApp"), "title missing: {texts:?}");
    assert!(texts.iter().any(|t| t == "typed:"), "row missing: {texts:?}");
    // ...and the hard-coded terminal placeholder ("user@liquide:~$") is NOT.
    assert!(
        !texts.iter().any(|t| t.contains("user@liquide")),
        "placeholder should be replaced: {texts:?}"
    );
}

#[test]
fn typing_reaches_registered_app_model_and_repaints() {
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let _ = shell.set_focus(id);
    let state = Arc::new(Mutex::new(FakeAppState::default()));
    shell.register_app_view(id, Box::new(FakeApp::new(state.clone())));

    for key in [KeyCode::H, KeyCode::I] {
        shell.handle_platform_event(&type_key(key));
    }
    // The characters reached the app's MODEL (not the shell's local buffer).
    assert_eq!(state.lock().unwrap().buffer, "hi");
    // The shell's legacy per-window buffer stays empty when a view is present.
    assert_eq!(shell.window_text_input(id), None);

    // The new text is painted from the app's content view.
    let texts = scene_texts(&mut shell);
    assert!(texts.iter().any(|t| t == "typed:hi"), "repaint missing: {texts:?}");
}

#[test]
fn non_printable_keys_reach_registered_app() {
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let _ = shell.set_focus(id);
    let state = Arc::new(Mutex::new(FakeAppState::default()));
    shell.register_app_view(id, Box::new(FakeApp::new(state.clone())));

    for key in [KeyCode::Enter, KeyCode::Backspace, KeyCode::ArrowLeft] {
        shell.handle_platform_event(&type_key(key));
    }
    assert_eq!(
        state.lock().unwrap().keys,
        vec!["Enter".to_string(), "Backspace".to_string(), "ArrowLeft".to_string()]
    );
}

#[test]
fn close_window_drops_app_view() {
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_app_window("com.liquide.terminal");
    shell.register_app_view(id, Box::new(FakeApp::new(Arc::new(Mutex::new(FakeAppState::default())))));
    assert!(shell.has_app_view(id));
    let _ = shell.close_window(id);
    assert!(!shell.has_app_view(WindowId(id.0)));
}

#[test]
fn factory_constructs_and_registers_on_open() {
    let mut shell = Shell::new(1280.0, 720.0);
    // Install a host factory that backs exactly one id with a FakeApp.
    shell.set_app_view_factory(Box::new(|app_id| {
        if app_id == "com.liquide.terminal" {
            Some(Box::new(FakeApp::new(Arc::new(Mutex::new(FakeAppState::default()))))
                as Box<dyn AppView>)
        } else {
            None
        }
    }));
    // Opening the backed app constructs + registers the view automatically.
    let id = shell.open_app_window("com.liquide.terminal");
    assert!(shell.has_app_view(id), "factory should auto-register on open");
    // An unbacked app gets no view (falls back to placeholder painting).
    let other = shell.open_app_window("com.liquide.browser");
    assert!(!shell.has_app_view(other));
}

#[test]
fn without_factory_open_keeps_placeholder() {
    // No factory installed → legacy placeholder painting still works.
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_app_window("com.liquide.terminal");
    assert!(!shell.has_app_view(id));
    let texts = scene_texts(&mut shell);
    assert!(
        texts.iter().any(|t| t.contains("user@liquide")),
        "placeholder terminal should still paint without a factory: {texts:?}"
    );
}

//! Cross-crate seam between built-in applications and the desktop shell.
//!
//! The shell (`liquide-shell`) never embeds an application crate directly — it
//! only knows a [`crate::AppView`] trait object registered against a window. This
//! module defines that contract:
//!
//! * [`AppTextInput`] — routes typed text and key events into an app's model.
//! * [`AppContentProvider`] — exposes a *render model* ([`AppContentView`]) the
//!   shell turns into scene/DOM nodes (replacing the old hard-coded per-app
//!   `build_window_content` branches and the `Label` placeholder roots).
//! * [`AppView`] — the object-safe super-trait the shell holds as
//!   `Box<dyn AppView>`, keyed by `WindowId` (see the S6 shell-hook spec).
//!
//! Apps depend on `liquide-interop` (which depends only on `liquide-common`),
//! so wiring this seam introduces **no** dependency cycle: the shell already
//! depends on `liquide-interop`, and `liquide-interop` depends on no app crate.

use serde::{Deserialize, Serialize};

/// A logical key delivered to an app's model.
///
/// This is intentionally small and self-contained (no `liquide-ui-core`
/// dependency) so the trait stays usable from the seam crate. The shell maps
/// its own `KeyEvent`s onto these before forwarding; apps map these onto their
/// existing handlers (e.g. terminal `send_key`, editor `handle_key`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppKey {
    /// A printable character (already resolved for modifiers/shift).
    Char(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// A named key not covered above (e.g. "F5"); apps may ignore it.
    Named(String),
}

impl AppKey {
    /// The key's printable name, suitable for app handlers that take a `&str`
    /// (e.g. the terminal `send_key` / editor `handle_key` string protocol).
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            AppKey::Char(_) => "",
            AppKey::Enter => "Enter",
            AppKey::Backspace => "Backspace",
            AppKey::Tab => "Tab",
            AppKey::Escape => "Escape",
            AppKey::Delete => "Delete",
            AppKey::Left => "ArrowLeft",
            AppKey::Right => "ArrowRight",
            AppKey::Up => "ArrowUp",
            AppKey::Down => "ArrowDown",
            AppKey::Home => "Home",
            AppKey::End => "End",
            AppKey::PageUp => "PageUp",
            AppKey::PageDown => "PageDown",
            AppKey::Named(s) => s.as_str(),
        }
    }
}

/// Routing of typed text / key events into an application's model.
///
/// Object-safe: the shell holds this as part of a `dyn AppView`. Implementors
/// forward into their existing model (terminal VT `send_input`, editor
/// `handle_char`/`handle_key`, file/settings/etc. search & navigation).
pub trait AppTextInput {
    /// Route a run of typed text (one or more printable characters) into the
    /// model. The shell calls this for committed IME / typed text. Returns
    /// `true` if the model changed and the window should be redrawn.
    fn handle_text(&mut self, text: &str) -> bool;

    /// Route a single logical key into the model. Returns `true` if the model
    /// changed and the window should be redrawn.
    fn handle_key(&mut self, key: &AppKey) -> bool;
}

/// A horizontal run of text sharing one foreground color, within a
/// [`ContentRow`]. Columns are character offsets from the row start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentSpan {
    /// Inclusive start column (character offset).
    pub start_col: u32,
    /// Exclusive end column (character offset).
    pub end_col: u32,
    /// Packed `0xRRGGBBAA` foreground color. `None` = use the theme default.
    pub color: Option<u32>,
    /// Whether the run is rendered bold.
    pub bold: bool,
}

/// One row of renderable content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentRow {
    /// The row text (already laid out as a single logical line).
    pub text: String,
    /// Styled spans over `text`. May be empty (then the whole row uses the
    /// theme default color).
    pub spans: Vec<ContentSpan>,
    /// Optional leading label (e.g. an editor line number / a file icon hint).
    pub gutter: Option<String>,
    /// Whether this row is the focused/active row (e.g. cursor line, selected
    /// list item) — the shell may highlight it.
    pub active: bool,
}

impl ContentRow {
    /// A plain unstyled row.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            spans: Vec::new(),
            gutter: None,
            active: false,
        }
    }
}

/// Visual archetype for the content surface, so the shell can pick a sensible
/// background / metrics (monospace terminal vs. proportional document/list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentKind {
    /// Monospaced grid (terminal).
    Terminal,
    /// Monospaced document with a gutter (text editor).
    Document,
    /// Proportional list / detail view (files, settings, task-manager,
    /// software-center).
    List,
}

/// The render model an app exposes to the shell. This is plain data; the shell
/// turns it into `SceneNode`s (or DOM). It deliberately does **not** reference
/// `liquide-ui-core::Widget` or the shell's `SceneNode`, keeping the apps free
/// of any shell/toolkit coupling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppContentView {
    /// Visual archetype.
    pub kind: ContentKind,
    /// Optional header / title line painted above the rows.
    pub title: Option<String>,
    /// The body rows.
    pub rows: Vec<ContentRow>,
    /// Optional text cursor position `(row, col)` in character cells, relative
    /// to `rows` — `None` if the app has no caret to paint.
    pub cursor: Option<(u32, u32)>,
}

impl AppContentView {
    /// Construct an empty view of a given kind.
    #[must_use]
    pub fn new(kind: ContentKind) -> Self {
        Self {
            kind,
            title: None,
            rows: Vec::new(),
            cursor: None,
        }
    }

    /// Total renderable rows. The shell uses this (plus `title`) to know the
    /// content is non-placeholder; a view with rows or a title is "real".
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.rows.is_empty()
    }
}

/// Exposes an app's renderable content to the shell.
pub trait AppContentProvider {
    /// Build the current content view for the window of the given size in
    /// *character cells* (`cols`, `rows`). Apps that are pixel-agnostic may
    /// ignore the hint; grid apps (terminal) use it to size the viewport.
    fn content_view(&self, cols: u32, rows: u32) -> AppContentView;
}

/// The object-safe seam the shell holds per window: `Box<dyn AppView>`.
///
/// It is the union of input routing, content provision, and the optional
/// widget-UI seam. The text content path and the widget path coexist: the shell
/// prefers a [`crate::AppWidgetModel`] when [`AppView::widget_model`] returns
/// `Some`, and otherwise renders the [`AppContentView`] text path.
///
/// The widget-seam methods ([`widget_model`](AppView::widget_model) /
/// [`apply_action`](AppView::apply_action)) are **defaulted** (model `None`,
/// `apply_action` `false`), so terminal and un-migrated apps keep compiling and
/// keep the text path with **no** changes. An app *opts in* to the widget UI by
/// overriding `widget_model` (and `apply_action`); apps that prefer to keep the
/// widget logic in a separate impl can implement [`crate::AppWidgetProvider`]
/// and forward to it from these methods.
pub trait AppView: AppTextInput + AppContentProvider + Send {
    /// A stable reverse-DNS identifier of the backing app (for diagnostics /
    /// the shell's per-`app_id` styling fallbacks).
    fn app_id(&self) -> &str;

    /// The current widget UI, or `None` to fall back to the text content path.
    ///
    /// Defaults to `None` (no widget UI) so un-migrated apps keep the text path.
    /// See [`crate::AppWidgetProvider::widget_model`].
    fn widget_model(&self) -> Option<crate::AppWidgetModel> {
        None
    }

    /// Apply a host-delivered [`crate::AppWidgetAction`] to the model.
    ///
    /// Returns `true` if the model changed (and the window should be redrawn).
    /// Defaults to a no-op returning `false`. See
    /// [`crate::AppWidgetProvider::apply_action`].
    fn apply_action(&mut self, action: &crate::AppWidgetAction) -> bool {
        let _ = action;
        false
    }

    /// Advance the app's asynchronous state by one frame.
    ///
    /// The shell calls this once per frame for every live app window so apps
    /// backed by an asynchronous source can drain pending output and surface it
    /// in the next [`AppContentProvider::content_view`]. The canonical consumer
    /// is the terminal: a real PTY echoes typed bytes asynchronously, so the
    /// grid only reflects typed input after the terminal runtime drains the PTY
    /// here (completing the t70-s6 terminal echo route).
    ///
    /// Returns `true` if the model changed and the window should be redrawn.
    /// The default implementation is a no-op (`false`) so purely synchronous
    /// apps (editor, files, settings, …) need not implement it.
    fn tick(&mut self) -> bool {
        false
    }
}

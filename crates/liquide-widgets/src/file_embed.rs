//! `<lq-file-embed>` — an embedded file element (NAV/OVERLAY + FILE family).
//!
//! Shows a file as a card: a type icon (derived from the extension), the file
//! name, a human-readable size, and **Open** / **Download** affordances. The file
//! metadata (existence, size) is read from disk via [`std::fs`] — the project's
//! file-IO decision treats this widget as TRUSTED (it is part of the toolkit, not
//! sandboxed guest code), so a direct `std::fs::metadata` read is permitted.
//!
//! Safety contract (the whole point of the std::fs handling):
//! - the read is **lazy + total**: [`FileEmbed::probe`] calls
//!   `std::fs::metadata` once and stores the result; ANY error (missing path,
//!   permission denied, a path that is a directory) becomes a graceful
//!   [`FileState::Error`] — never a panic, never an `unwrap`;
//! - a missing/denied file renders the `.error` state (icon + message) and its
//!   action affordances are disabled, so the owner cannot fire `open`/`download`
//!   on a file that isn't there.
//!
//! Behavior:
//! - **Click Open** (`data-part="open"`) on a present file → `Action`(`open`)
//!   with the path as payload.
//! - **Click Download** (`data-part="download"`) on a present file →
//!   `Action`(`download`) with the path.
//! - On an errored file both affordances are inert.
//! Hit-tests read each affordance box from layout.

use std::path::{Path, PathBuf};

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::layout_query::LayoutQuery;

/// Emitted when the file's Open affordance is activated (payload: path).
pub const OPEN_ACTION: &str = "open";
/// Emitted when the file's Download affordance is activated (payload: path).
pub const DOWNLOAD_ACTION: &str = "download";

/// The resolved metadata state of the embedded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Not yet probed (no `std::fs` call made).
    Unprobed,
    /// The file exists; carries its size in bytes.
    Present {
        /// File size in bytes (from `std::fs::metadata`).
        size: u64,
    },
    /// Probing failed (missing, denied, a directory, …); carries a message.
    Error {
        /// A short, human-readable reason (never a panic).
        message: String,
    },
}

/// An embedded file element.
#[derive(Debug, Clone)]
pub struct FileEmbed {
    /// The file path (as configured; the display name is its file_name).
    path: PathBuf,
    /// The resolved metadata state.
    state: FileState,
}

impl FileEmbed {
    /// Build a file embed for `path` (UNPROBED — call [`probe`](Self::probe) to
    /// read metadata from disk, or [`probed`](Self::probed) to do it eagerly).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: FileState::Unprobed,
        }
    }

    /// Build a file embed for `path` and immediately probe its metadata.
    pub fn probed(path: impl Into<PathBuf>) -> Self {
        let mut f = Self::new(path);
        f.probe();
        f
    }

    /// Read the file's metadata via `std::fs`, resolving [`state`](Self::state).
    ///
    /// TOTAL: every failure mode (NotFound, PermissionDenied, a directory, any
    /// other I/O error) maps to [`FileState::Error`] with a short message. Never
    /// panics, never unwraps. Returns `Changed` so a host re-renders the result.
    pub fn probe(&mut self) -> WidgetOutcome {
        self.state = Self::read_state(&self.path);
        WidgetOutcome::Changed
    }

    /// The pure metadata read: classify a path into a [`FileState`] without ever
    /// panicking. Factored out so it is unit-testable directly.
    fn read_state(path: &Path) -> FileState {
        match std::fs::metadata(path) {
            Ok(md) if md.is_file() => FileState::Present { size: md.len() },
            Ok(md) if md.is_dir() => FileState::Error {
                message: "Path is a directory".to_string(),
            },
            Ok(_) => FileState::Error {
                message: "Not a regular file".to_string(),
            },
            Err(e) => FileState::Error {
                message: match e.kind() {
                    std::io::ErrorKind::NotFound => "File not found".to_string(),
                    std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
                    _ => "Cannot read file".to_string(),
                },
            },
        }
    }

    /// The configured path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The resolved state.
    pub fn state(&self) -> &FileState {
        &self.state
    }

    /// Whether the file is present (probed and a regular file).
    pub fn is_present(&self) -> bool {
        matches!(self.state, FileState::Present { .. })
    }

    /// The probed size in bytes, if present.
    pub fn size(&self) -> Option<u64> {
        match self.state {
            FileState::Present { size } => Some(size),
            _ => None,
        }
    }

    /// The display name (the path's file name, or the whole path if it has none).
    pub fn display_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    /// The lowercase extension (no dot), or empty.
    fn extension(&self) -> String {
        self.path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }

    /// A coarse file-type class derived from the extension (drives the icon CSS).
    pub fn type_class(&self) -> &'static str {
        match self.extension().as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => "image",
            "mp4" | "mov" | "mkv" | "webm" | "avi" => "video",
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
            "pdf" => "pdf",
            "zip" | "tar" | "gz" | "7z" | "rar" => "archive",
            "rs" | "js" | "ts" | "py" | "c" | "cpp" | "h" | "go" | "java" | "rb" | "sh"
            | "json" | "toml" | "yaml" | "yml" | "xml" | "html" | "css" => "code",
            "txt" | "md" | "rtf" | "doc" | "docx" => "document",
            "" => "file",
            _ => "file",
        }
    }

    /// Format a byte count into a short human string (e.g. `1.5 KB`).
    pub fn human_size(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        if bytes < 1024 {
            return format!("{bytes} B");
        }
        let mut size = bytes as f64;
        let mut unit = 0;
        while size >= 1024.0 && unit < UNITS.len() - 1 {
            size /= 1024.0;
            unit += 1;
        }
        format!("{size:.1} {}", UNITS[unit])
    }

    fn box_hit(&self, root: NodeId, part: &str, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }
}

impl WidgetBehavior for FileEmbed {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        // An errored / unprobed-missing file has inert affordances.
        if !self.is_present() {
            return WidgetOutcome::Ignored;
        }
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            let p = Point::new(*x, *y);
            let path = self.path.to_string_lossy().into_owned();
            if self.box_hit(root, "open", p, layout) {
                return WidgetOutcome::action_with(OPEN_ACTION, path);
            }
            if self.box_hit(root, "download", p, layout) {
                return WidgetOutcome::action_with(DOWNLOAD_ACTION, path);
            }
        }
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        self.is_present()
    }

    fn render(&self) -> TemplateNode {
        let present = self.is_present();
        let mut root = TemplateNode::el("lq-file-embed")
            .attr(FOCUSABLE_ATTR, if present { "true" } else { "false" })
            .attr("role", "group")
            .attr("data-type", self.type_class())
            .attr("data-path", &self.path.to_string_lossy())
            .class(self.type_class())
            .class_if("present", present)
            .class_if("error", matches!(self.state, FileState::Error { .. }))
            .pseudo_if(PseudoStateFlags::DISABLED, !present);

        // Icon (its glyph/colour driven by the type class in CSS).
        root = root.child(
            TemplateNode::el("lq-file-icon")
                .attr("data-part", "icon")
                .attr("data-type", self.type_class())
                .class(self.type_class()),
        );

        // Meta column: name + (size | error message).
        let mut meta = TemplateNode::el("lq-file-meta").attr("data-part", "meta").child(
            TemplateNode::el("lq-file-name")
                .attr("data-part", "name")
                .child(TemplateNode::text(&self.display_name())),
        );
        let sub_text = match &self.state {
            FileState::Present { size } => Self::human_size(*size),
            FileState::Error { message } => message.clone(),
            FileState::Unprobed => "—".to_string(),
        };
        meta = meta.child(
            TemplateNode::el("lq-file-sub")
                .attr("data-part", "sub")
                .class_if("error", matches!(self.state, FileState::Error { .. }))
                .child(TemplateNode::text(&sub_text)),
        );
        root = root.child(meta);

        // Action affordances (disabled when not present).
        let actions = TemplateNode::el("lq-file-actions")
            .attr("data-part", "actions")
            .child(
                TemplateNode::el("lq-file-open")
                    .attr("data-part", "open")
                    .attr("role", "button")
                    .class_if("disabled", !present)
                    .pseudo_if(PseudoStateFlags::DISABLED, !present)
                    .child(TemplateNode::text("Open")),
            )
            .child(
                TemplateNode::el("lq-file-download")
                    .attr("data-part", "download")
                    .attr("role", "button")
                    .class_if("disabled", !present)
                    .pseudo_if(PseudoStateFlags::DISABLED, !present)
                    .child(TemplateNode::text("Download")),
            );
        root = root.child(actions);

        if !present {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

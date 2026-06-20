//! Generic `AppWidgetModel` → `liquide-widgets` integration engine (t108, P8).
//!
//! This module is the SHELL-SIDE pipeline that turns ANY app's toolkit-free
//! [`liquide_interop::AppWidgetModel`] into mounted, interactive
//! [`liquide_widgets`] Components rendered through the DOM/CSS pipeline. It is
//! deliberately **app-agnostic**: it lives in the shell because it bridges two
//! crates the shell already depends on (`liquide-interop` for the plain-data
//! model, `liquide-widgets` for the toolkit) and the apps must see neither the
//! toolkit nor the shell's scene types.
//!
//! ## The three pieces
//!
//! 1. **Mapper** ([`mount_model_into`]): walk an [`AppWidgetModel`] tree and, for
//!    each node, either create a structural container DOM element
//!    (`lq-panel`/`lq-card`/…) directly, or mount the matching
//!    [`liquide_widgets`] behavior as its own [`WidgetHost`] entry (so the host
//!    owns its runtime state — selection / checked / value / caret — keyed by the
//!    widget's stable key). Keys are namespaced per window
//!    (`aw-<window_id>-<key>`) so two windows showing the same app key never
//!    collide; the original key is recovered by [`strip_widget_id`] when an
//!    action is translated back to the app.
//!
//! 2. **Per-window host registry** ([`crate::shell::Shell::app_widget_hosts`]):
//!    one [`WidgetHost`] per widget-backed window. The host's widget DOM is
//!    mounted under a per-window `app-content-host` element positioned over the
//!    window's content rect (mirroring the P6 `window-frame` scaffold), so the
//!    widgets flow through the same CSS pipeline that paints all chrome, clipped
//!    to the content rect.
//!
//! 3. **Action translation** ([`translate_action`]): the real
//!    [`liquide_widgets::WidgetAction`] (a toolkit type the app must not see) is
//!    mapped to the plain-data [`liquide_interop::AppWidgetAction`] triple the
//!    app's `apply_action` consumes, using the widget's variant to pick the verb.

use liquide_interop::{
    AppWidget, AppWidgetAction, AppWidgetModel, ButtonKind, ScriptLang, SelectionMode,
    WasmModuleSource,
};
use liquide_widgets::{
    Breadcrumb, Button, Chip, Dropdown, Label, Link, List, Pagination, Progress, RadioGroup,
    Segmented, Slider, Table, TextArea, TextInput, Toggle, Tree, WidgetBehavior,
};

use liquide_dom::{Document, NodeId};
use liquide_widgets::host::WidgetHost;
use liquide_widgets::tree::TreeNode as WTreeNode;

// ════════════════════════════════════════════════════════════════════════════
// Embedded-runtime nodes (t152): WasmApp / ScriptApp.
//
// A `WasmApp { module }` / `ScriptApp { source, lang }` node is plain data that
// names an untrusted WASM / TS-JS module. At mount time we instantiate the
// corresponding host (the REAL sandboxed runtime under the `wasm-apps` /
// `script-apps` feature, the crate's `Null*` stub otherwise), call `render()` to
// obtain the inner [`AppWidgetModel`] the module emitted, and mount THAT through
// the normal widget pipeline (`mount_model_into`). So a WASM/TS module's UI
// renders through exactly the same CSS widget path as a native app's model.
//
// Under the default build (Null hosts) `render()` returns `Unavailable`, so the
// mapper substitutes a graceful "runtime unavailable" placeholder model instead
// of crashing — the wiring is identical, only the resolved model differs.
// ════════════════════════════════════════════════════════════════════════════

/// The class added to an embedded-runtime placeholder wrapper so a theme can
/// style the "runtime unavailable" notice distinctly.
pub(crate) const RUNTIME_PLACEHOLDER_CLASS: &str = "app-runtime-unavailable";

/// Build a synthetic placeholder [`AppWidgetModel`] shown when an embedded
/// runtime cannot produce a UI (the feature is off → a Null host, or the real
/// host errored: bad module, trap, decode failure). It is a normal model (a
/// `GroupBox` + a `Label`), so it renders through the SAME widget pipeline as
/// any other content — never a panic, never an empty hole.
#[must_use]
pub(crate) fn runtime_placeholder_model(runtime: &str, detail: &str) -> AppWidgetModel {
    let text = if detail.is_empty() {
        format!("{runtime} runtime unavailable")
    } else {
        format!("{runtime} runtime unavailable: {detail}")
    };
    AppWidgetModel::with_root(vec![AppWidget::GroupBox {
        label: format!("{runtime} app"),
        children: vec![AppWidget::Label { text }],
    }])
}

/// Whether `model` is the synthetic "runtime unavailable" placeholder produced
/// by [`runtime_placeholder_model`] (a single `GroupBox` containing one `Label`
/// whose text carries the placeholder marker). Used to tag the mount wrapper so
/// a theme can style the notice; a real emitted model is never mistaken for one.
#[must_use]
pub(crate) fn is_placeholder_model(model: &AppWidgetModel) -> bool {
    matches!(
        model.root.as_slice(),
        [AppWidget::GroupBox { children, .. }]
            if matches!(
                children.as_slice(),
                [AppWidget::Label { text }] if text.contains("runtime unavailable")
            )
    )
}

/// Resolve a [`WasmModuleSource`] to module bytes, reading a `Path` from disk.
/// A missing/unreadable path is surfaced as the error string so the placeholder
/// can explain it (and so the Null-host build never needs the bytes at all).
#[cfg(feature = "wasm-apps")]
fn wasm_module_bytes(module: &WasmModuleSource) -> std::result::Result<Vec<u8>, String> {
    match module {
        WasmModuleSource::Bytes { bytes } => Ok(bytes.clone()),
        WasmModuleSource::Path { path } => {
            std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
        }
    }
}

/// Run an embedded WASM module's `render()` and return the inner model it
/// emitted, or a placeholder model when the runtime is unavailable / errored.
///
/// Under the default build (no `wasm-apps` feature) this uses
/// [`liquide_wasm_host::NullWasmHost`], whose `render()` reports `Unavailable`,
/// so the placeholder is returned. With the feature on it loads the bytes into
/// the real sandboxed [`liquide_wasm_host::WasmHost`].
#[must_use]
pub(crate) fn render_wasm_app(module: &WasmModuleSource) -> AppWidgetModel {
    use liquide_wasm_host::WasmHostApi;

    #[cfg(feature = "wasm-apps")]
    let result: liquide_wasm_host::Result<AppWidgetModel> = (|| {
        let bytes = wasm_module_bytes(module)
            .map_err(liquide_wasm_host::WasmHostError::Load)?;
        let mut host = liquide_wasm_host::WasmHost::from_bytes_default(&bytes)?;
        host.render()
    })();

    #[cfg(not(feature = "wasm-apps"))]
    let result: liquide_wasm_host::Result<AppWidgetModel> = {
        // The Null host ignores the bytes and reports Unavailable; pass an empty
        // slice so we never read a file in the default build.
        let _ = module;
        let mut host = liquide_wasm_host::NullWasmHost::from_bytes_default(&[])
            .expect("null wasm host constructs");
        host.render()
    };

    result.unwrap_or_else(|e| runtime_placeholder_model("WASM", &e.to_string()))
}

/// Run an embedded TS/JS module's `render()` and return the inner model it
/// emitted, or a placeholder model when the runtime is unavailable / errored.
///
/// Under the default build (no `script-apps` feature) this uses
/// [`liquide_script_host::NullScriptHost`] (reports `Unavailable` → placeholder);
/// with the feature on it transpiles + runs the source in the real boa+swc host.
/// `lang` is recorded for the future JS-vs-TS authoring distinction; both flow
/// through the host's transpile step (a no-op for type-free JS).
#[must_use]
pub(crate) fn render_script_app(source: &str, lang: ScriptLang) -> AppWidgetModel {
    use liquide_script_host::ScriptHostApi;
    let _ = lang;

    #[cfg(feature = "script-apps")]
    let result: liquide_script_host::Result<AppWidgetModel> = (|| {
        let mut host = liquide_script_host::ScriptHost::from_source_default(source)?;
        host.render()
    })();

    #[cfg(not(feature = "script-apps"))]
    let result: liquide_script_host::Result<AppWidgetModel> = {
        let mut host = liquide_script_host::NullScriptHost::from_source_default(source)
            .expect("null script host constructs");
        host.render()
    };

    result.unwrap_or_else(|e| runtime_placeholder_model("Script", &e.to_string()))
}

/// Route an [`AppWidgetAction`] that targets a widget INSIDE an embedded
/// runtime's emitted UI through that runtime's `apply_action()`, then re-render,
/// returning the fresh inner model (or a placeholder if the runtime is
/// unavailable / errored). This is the embedded-node analogue of the native
/// app's `apply_action → re-render` loop: the action flows into the module that
/// owns the sub-UI, and the module's new `render()` output is what re-mounts.
///
/// Note on statefulness: a freshly-instantiated host is used (the WASM host is
/// stateless across calls by design — every call uses a fresh Store — so this is
/// exact for it). A future dom_sync.rs follow-up should persist a per-node host
/// across frames so a *stateful* script app retains in-context mutations between
/// actions; the seam here keeps the render/apply contract in one place.
///
/// (Currently consumed only by tests — the live per-node action drive lives in
/// `shell/dom_sync.rs::drive_app_widget_hosts`, which is the documented
/// out-of-lock follow-up that persists a per-node host across frames; this seam
/// is what that follow-up calls. `allow(dead_code)` keeps the default lib build
/// warning-clean until then.)
#[allow(dead_code)]
#[must_use]
pub(crate) fn apply_action_to_wasm_app(
    module: &WasmModuleSource,
    action: &AppWidgetAction,
) -> AppWidgetModel {
    let _ = action;

    #[cfg(feature = "wasm-apps")]
    {
        use liquide_wasm_host::WasmHostApi;
        let attempt = (|| -> liquide_wasm_host::Result<AppWidgetModel> {
            let bytes =
                wasm_module_bytes(module).map_err(liquide_wasm_host::WasmHostError::Load)?;
            let mut host = liquide_wasm_host::WasmHost::from_bytes_default(&bytes)?;
            host.apply_action(action)?;
            host.render()
        })();
        attempt.unwrap_or_else(|e| runtime_placeholder_model("WASM", &e.to_string()))
    }

    #[cfg(not(feature = "wasm-apps"))]
    {
        // Null host: nothing to apply; re-render yields the placeholder.
        render_wasm_app(module)
    }
}

/// The [`ScriptApp`](AppWidget::ScriptApp) analogue of
/// [`apply_action_to_wasm_app`] (see its note on the dom_sync.rs follow-up).
#[allow(dead_code)]
#[must_use]
pub(crate) fn apply_action_to_script_app(
    source: &str,
    lang: ScriptLang,
    action: &AppWidgetAction,
) -> AppWidgetModel {
    let _ = (lang, action);

    #[cfg(feature = "script-apps")]
    {
        use liquide_script_host::ScriptHostApi;
        let attempt = (|| -> liquide_script_host::Result<AppWidgetModel> {
            let mut host = liquide_script_host::ScriptHost::from_source_default(source)?;
            host.apply_action(action)?;
            host.render()
        })();
        attempt.unwrap_or_else(|e| runtime_placeholder_model("Script", &e.to_string()))
    }

    #[cfg(not(feature = "script-apps"))]
    {
        render_script_app(source, lang)
    }
}

/// Apply a pan/zoom action to a `Map` node's viewport and return the new
/// `(center_lat, center_lon, zoom)`. Pure math: it builds a transient
/// [`liquide_map::MapState`] for the current viewport, applies the action, and
/// reads the updated centre/zoom back out. The caller writes those back into the
/// `Map` model node (so the next render re-derives the visible-tile set).
///
/// Action verbs (`AppWidgetAction.name`):
///   * `"pan"`  — payload `"dx,dy"` screen-pixel drag delta.
///   * `"zoom"` — payload `"delta"` (centre-fixed) or `"delta@ax,ay"`
///     (wheel-zoom toward screen anchor `(ax,ay)`).
/// An unrecognised verb / unparseable payload leaves the viewport unchanged.
///
/// `allow(dead_code)` outside tests: the LIVE action drive that calls this — a
/// drag on the laid-out map box → `pan`, a wheel → `zoom` toward the cursor,
/// then writing the new centre/zoom back into the `Map` model node and
/// remounting — is the documented dom_sync follow-up (it owns the per-node
/// pointer/drag geometry from the laid-out box, like the t152/t155 follow-ups).
/// This chokepoint + its tests are in lock today.
#[allow(dead_code)]
#[must_use]
pub(crate) fn apply_map_action(
    center_lat: f64,
    center_lon: f64,
    zoom: u32,
    action: &AppWidgetAction,
) -> (f64, f64, u32) {
    let mut state = map_state_for(center_lat, center_lon, zoom);
    match action.name.as_str() {
        "pan" => {
            if let Some((dx, dy)) = parse_pair(&action.payload) {
                state.pan(dx, dy);
            }
        }
        "zoom" => {
            // "delta" or "delta@ax,ay".
            let (delta_str, anchor) = match action.payload.split_once('@') {
                Some((d, a)) => (d, parse_pair(a)),
                None => (action.payload.as_str(), None),
            };
            if let Ok(delta) = delta_str.trim().parse::<i32>() {
                match anchor {
                    Some((ax, ay)) => {
                        state.zoom_at(delta, ax, ay);
                    }
                    None => {
                        state.zoom_by(delta);
                    }
                }
            }
        }
        _ => {}
    }
    (state.viewport.center.lat, state.viewport.center.lon, state.viewport.zoom)
}

/// Parse a `"a,b"` pair of `f64`s; `None` if malformed.
#[allow(dead_code)] // used by `apply_map_action` (the live drive is the dom_sync follow-up).
fn parse_pair(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

// ════════════════════════════════════════════════════════════════════════════
// Silent <video> surface (t155): the `Video` model node.
//
// A `Video { src, .. }` node is plain data naming a video file. At mount time:
//  * Under the `video` feature (real pure-Rust AV1 decoder available) it mounts a
//    `lq-video` SURFACE element bound to a stable per-node `image_id`. The session
//    render loop holds a `liquide_video::VideoSource` for that node, polls a frame
//    each tick, and pushes it via `register_image_rgba(image_id, rgba, w, h)`; the
//    surface's `SceneNodeKind::Image` blits it (renderer/images.rs). That live
//    poll→texture drive lives in `liquide-session` (render_thread.rs) — this
//    mapper just emits the surface + the stable id binding it.
//  * By default (no codec → `NullVideoSource` reports `Unavailable`) it mounts a
//    graceful "video unavailable / no codec" placeholder model through the SAME
//    widget pipeline (the t152 `runtime_placeholder_model` precedent) — never a
//    panic, never an empty hole.
// ════════════════════════════════════════════════════════════════════════════

/// Whether the real pure-Rust video decoder is compiled in (the `video` feature).
/// When false, a `Video` node mounts the "no codec" placeholder.
#[must_use]
pub(crate) fn video_codec_available() -> bool {
    cfg!(feature = "video")
}

/// A stable per-node texture id for a `<video>` surface, derived from the window
/// id and the source path so the session render loop can target the surface's
/// `SceneNodeKind::Image` with `register_image_rgba`. Two videos with different
/// sources (or in different windows) get distinct ids; the same one is stable
/// across frames so each tick re-uploads under the same key.
#[must_use]
pub(crate) fn video_image_id(window_id: u64, src: &str) -> u64 {
    // FNV-1a over the window id + src — stable, no extra dependency.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mix(&window_id.to_le_bytes());
    mix(b"/video/");
    mix(src.as_bytes());
    // Keep it out of the low range to avoid colliding with small wallpaper ids.
    hash | 0x4000_0000_0000_0000
}

/// The placeholder model shown for a `Video` node when no codec is compiled in.
#[must_use]
pub(crate) fn video_placeholder_model(src: &str) -> AppWidgetModel {
    AppWidgetModel::with_root(vec![AppWidget::GroupBox {
        label: "Video".into(),
        children: vec![AppWidget::Label {
            text: format!("video unavailable: no codec (cannot play {src})"),
        }],
    }])
}

// ════════════════════════════════════════════════════════════════════════════
// OpenStreetMap slippy-map surface (t159): the `Map` model node.
//
// A `Map { center_lat, center_lon, zoom }` node is plain data describing a
// viewport. At mount time the shell builds a `liquide_map::MapState` for the
// viewport, asks it for the visible tiles' SCREEN RECTS + stable IMAGE KEYS
// (pure Web-Mercator math, no network), and emits one positioned tile element
// per visible tile:
//   * Each tile element is styled with `background-image: url(tile://z/x/y)`, so
//     the scene bridge hashes that key to a renderer image id and the session
//     decode path registers the fetched+decoded RGBA under the SAME hash — the
//     tile's `Image` scene node then blits it (the wallpaper/video seam).
//   * Tiles whose bytes are not (yet) loaded get a PLACEHOLDER class instead, so
//     the default (no-net) build paints a graceful grid + an "offline" notice —
//     no panic, no required network.
// Pan (drag) + zoom (wheel/buttons) update the viewport through `apply_action`
// (see `apply_map_action`); the laid-out map box gives the hit/drag geometry.
//
// The map box is laid out at a fixed default size here; the live per-node map
// registry (one persistent `MapState` per Map node, ticked each frame to fetch
// + decode tiles) is the documented session/dom_sync follow-up, mirroring the
// t152 per-node host + t155 per-node VideoSource. This mapper emits the tiles +
// their stable image-key bindings; the in-lock session chokepoint
// (`push_map_tile`) + its decode test are wired today.
// ════════════════════════════════════════════════════════════════════════════

/// The on-screen size (px) the map surface is laid out at when a `Map` node is
/// mounted. The viewport's visible-tile math uses this box; the live drive can
/// later resize it from the actual laid-out box (the documented follow-up).
pub(crate) const MAP_SURFACE_WIDTH: f64 = 640.0;
pub(crate) const MAP_SURFACE_HEIGHT: f64 = 400.0;

/// The class added to the map wrapper when no tiles are loaded (the default
/// offline build), so a theme can style the placeholder grid + "offline" notice.
pub(crate) const MAP_OFFLINE_CLASS: &str = "map-offline";

/// Whether real OSM tile fetching is compiled in (the shell's `map` feature →
/// `liquide-map/net`). When false the map does the tile MATH only and shows the
/// placeholder grid until something feeds it tiles.
#[must_use]
pub(crate) fn map_tile_fetch_available() -> bool {
    cfg!(feature = "map")
}

/// Build the [`liquide_map::MapState`] for a `Map` node's viewport at the default
/// surface size. Pure math — constructs no network client.
#[must_use]
pub(crate) fn map_state_for(center_lat: f64, center_lon: f64, zoom: u32) -> liquide_map::MapState {
    liquide_map::MapState::new(
        liquide_map::LatLon::new(center_lat, center_lon),
        zoom,
        MAP_SURFACE_WIDTH,
        MAP_SURFACE_HEIGHT,
    )
}

/// Per-window prefix for namespacing a widget's stable key into a globally-unique
/// [`liquide_widgets::WidgetId`]: `aw-<window_id>-<key>`.
#[must_use]
pub(crate) fn widget_id(window_id: u64, key: &str) -> String {
    format!("aw-{window_id}-{key}")
}

/// Recover the app-side key from a namespaced widget id produced by
/// [`widget_id`]. Returns the original `key` (the substring after the
/// `aw-<window_id>-` prefix), or the whole id unchanged if it lacks the prefix.
#[must_use]
pub(crate) fn strip_widget_id(window_id: u64, id: &str) -> String {
    let prefix = format!("aw-{window_id}-");
    id.strip_prefix(&prefix).unwrap_or(id).to_string()
}

/// Map a [`ButtonKind`] to the widget toolkit's CSS variant class.
fn button_variant(kind: ButtonKind) -> &'static str {
    match kind {
        ButtonKind::Normal => "",
        ButtonKind::Primary => "primary",
        ButtonKind::Danger => "danger",
        ButtonKind::Ghost => "ghost",
    }
}

/// Convert an interop `TreeNode` (plain data) into a `liquide-widgets` `TreeNode`.
fn map_tree_node(node: &liquide_interop::TreeNode) -> WTreeNode {
    if node.children.is_empty() {
        WTreeNode::leaf(node.id.clone(), node.label.clone()).expanded(node.expanded)
    } else {
        let children: Vec<WTreeNode> = node.children.iter().map(map_tree_node).collect();
        WTreeNode::branch(node.id.clone(), node.label.clone(), children).expanded(node.expanded)
    }
}

/// Build the `liquide-widgets` behavior for a single *interactive / leaf*
/// [`AppWidget`], preserving its current value/selection so the mounted widget
/// reflects the model. Containers return `None` (they are handled structurally
/// by [`mount_model_into`]).
///
/// The index of `selected` in an option list is recovered by matching the
/// option value, so the toolkit's index-based `select(idx)` lands on the right
/// option even though the model stores the selection by value.
#[must_use]
pub(crate) fn behavior_for(widget: &AppWidget) -> Option<Box<dyn WidgetBehavior>> {
    let b: Box<dyn WidgetBehavior> = match widget {
        // ── static / inert ──────────────────────────────────────────────
        AppWidget::Label { text } => Box::new(Label::new(text.clone())),
        AppWidget::Link { text, href } => Box::new(Link::new(text.clone(), href.clone())),
        AppWidget::Chip { label } => Box::new(Chip::new(label.clone())),
        AppWidget::Breadcrumb { crumbs } => Box::new(Breadcrumb::new(crumbs.clone())),
        AppWidget::Progress { value } => Box::new(Progress::fraction(*value as f32)),

        // ── buttons ─────────────────────────────────────────────────────
        AppWidget::Button { label, kind, .. } => {
            // The button's `action` IS its key/id — translate_action keys on the
            // emitting widget id, so the action name carried by the widget can be
            // a fixed verb ("click").
            Box::new(Button::new(label.clone(), "click").variant(button_variant(*kind)))
        }

        AppWidget::Segmented {
            options, selected, ..
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut seg = Segmented::new(pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                seg = seg.select(idx);
            }
            Box::new(seg)
        }

        // ── inputs ──────────────────────────────────────────────────────
        AppWidget::TextInput { value, .. } => Box::new(TextInput::new("").with_text(value.clone())),
        AppWidget::TextArea {
            value,
            gutter,
            readonly,
            ..
        } => Box::new(
            TextArea::new("")
                .with_text(value.clone())
                .with_gutter(*gutter)
                .disabled(*readonly),
        ),
        AppWidget::Checkbox { checked, .. } => Box::new(Toggle::checkbox("").checked(*checked)),
        AppWidget::Switch { checked, .. } => Box::new(Toggle::switch("").checked(*checked)),
        AppWidget::RadioGroup {
            key,
            options,
            selected,
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut rg = RadioGroup::new(key.clone(), pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                rg = rg.select(idx);
            }
            Box::new(rg)
        }
        AppWidget::Slider {
            min,
            max,
            step,
            value,
            ..
        } => Box::new(Slider::new(*min as f32, *max as f32, *value as f32).step(*step as f32)),
        AppWidget::Dropdown {
            options, selected, ..
        } => {
            let pairs = options.iter().map(|o| (o.value.clone(), o.label.clone()));
            let mut dd = Dropdown::new(pairs);
            if let Some(idx) = selected_index(options, selected.as_deref()) {
                dd = dd.select(idx);
            }
            Box::new(dd)
        }

        // ── collections ─────────────────────────────────────────────────
        AppWidget::List {
            items,
            selection_mode,
            selected,
            ..
        } => {
            // The list stores (value, label); the app uses indices as the value,
            // so value == label == item text and selection is by index.
            let pairs = items
                .iter()
                .map(|item| (item.clone(), item.clone()));
            let mut list = List::new(pairs);
            if matches!(selection_mode, SelectionMode::Multiple) {
                list = list.multi();
            }
            // Pre-select the first selected index (List::select clears prior),
            // matching the model's primary selection.
            if let Some(&first) = selected.first() {
                list = list.select(first as usize);
            }
            Box::new(list)
        }
        AppWidget::Table {
            columns,
            rows,
            sort,
            selection_mode,
            ..
        } => {
            let mut table = Table::new();
            let any_sortable = columns.iter().any(|c| c.sortable);
            for col in columns {
                table = table.column(col.label.clone());
            }
            for row in rows {
                table = table.row(row.clone());
            }
            if any_sortable {
                table = table.sortable(true);
            }
            let _ = (sort, selection_mode); // initial sort/selection: model-driven re-render keeps them in sync
            Box::new(table)
        }
        AppWidget::Tree { nodes, .. } => {
            let mut tree = Tree::new();
            for node in nodes {
                tree = tree.root(map_tree_node(node));
            }
            Box::new(tree)
        }
        AppWidget::Pagination { page, pages, .. } => {
            Box::new(Pagination::new(*pages as usize).page(*page as usize))
        }

        // ── containers: handled structurally, not as a behavior ─────────
        AppWidget::Panel { .. }
        | AppWidget::Card { .. }
        | AppWidget::GroupBox { .. }
        | AppWidget::Tabs { .. }
        | AppWidget::Toolbar { .. }
        | AppWidget::Accordion { .. } => return None,

        // ── embedded runtimes: rendered into a sub-model + mounted structurally
        //    by `mount_widget`, never as a behavior ─────────────────────────
        AppWidget::WasmApp { .. } | AppWidget::ScriptApp { .. } => return None,

        // ── video: a live texture surface (or placeholder), mounted structurally
        //    by `mount_widget`, never as a behavior ─────────────────────────
        AppWidget::Video { .. } => return None,

        // ── map: a tiled slippy-map surface, mounted structurally by
        //    `mount_widget` (positioned tile Image nodes), never as a behavior ─
        AppWidget::Map { .. } => return None,
    };
    Some(b)
}

/// A STRUCTURE-only signature of a model: the variant shape, keys, option
/// values, and container nesting — but NOT the mutable per-widget values
/// (checked / value / selected / caret). Two models with the same structure but
/// different values hash the same, so an action-driven value change does NOT
/// trigger a remount (the affected widget re-renders in place); only an
/// external structural change (added/removed/reordered widgets, changed option
/// sets) remounts. Returned as a `String` so the caller can hash it.
#[must_use]
pub(crate) fn model_structure(model: &AppWidgetModel) -> String {
    let mut out = String::new();
    for w in &model.root {
        structure_into(w, &mut out);
    }
    out
}

fn structure_into(widget: &AppWidget, out: &mut String) {
    match widget {
        AppWidget::Panel { children } => {
            out.push_str("P(");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Card { title, children } => {
            out.push_str("Cd[");
            out.push_str(title.as_deref().unwrap_or(""));
            out.push_str("](");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::GroupBox { label, children } => {
            out.push_str("Gb[");
            out.push_str(label);
            out.push_str("](");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Toolbar { children } => {
            out.push_str("Tb(");
            for c in children {
                structure_into(c, out);
            }
            out.push(')');
        }
        AppWidget::Tabs { tabs, selected } => {
            // The selected tab determines which panel is mounted, so it is part
            // of the structure (changing tabs remounts the panel).
            out.push_str(&format!("Tabs[{selected}](" ));
            if let Some(tab) = tabs.get(*selected as usize) {
                for c in &tab.children {
                    structure_into(c, out);
                }
            }
            out.push(')');
        }
        AppWidget::Accordion { sections } => {
            out.push_str("Ac(");
            for s in sections {
                // Expanded sections mount their bodies, so expansion is structure.
                out.push_str(&format!("s[{}:{}]", s.id, s.expanded));
                if s.expanded {
                    for c in &s.children {
                        structure_into(c, out);
                    }
                }
            }
            out.push(')');
        }
        AppWidget::Label { .. } => out.push_str("L;"),
        AppWidget::Link { href, .. } => out.push_str(&format!("Ln[{href}];")),
        AppWidget::Chip { .. } => out.push_str("Ch;"),
        AppWidget::Breadcrumb { crumbs } => out.push_str(&format!("Bc[{}];", crumbs.len())),
        AppWidget::Progress { .. } => out.push_str("Pr;"),
        AppWidget::Button { id, kind, .. } => out.push_str(&format!("B[{id}:{kind:?}];")),
        AppWidget::Segmented { key, options, .. } => {
            out.push_str(&format!("Sg[{key}:{}];", options.len()))
        }
        AppWidget::TextInput { key, .. } => out.push_str(&format!("Ti[{key}];")),
        AppWidget::TextArea { key, gutter, .. } => {
            out.push_str(&format!("Tx[{key}:{gutter}];"))
        }
        AppWidget::Checkbox { key, .. } => out.push_str(&format!("Cb[{key}];")),
        AppWidget::Switch { key, .. } => out.push_str(&format!("Sw[{key}];")),
        AppWidget::RadioGroup { key, options, .. } => {
            out.push_str(&format!("Rg[{key}:{}];", options.len()))
        }
        AppWidget::Slider { key, .. } => out.push_str(&format!("Sl[{key}];")),
        AppWidget::Dropdown { key, options, .. } => {
            out.push_str(&format!("Dd[{key}:{}];", options.len()))
        }
        AppWidget::List {
            key,
            items,
            selection_mode,
            ..
        } => out.push_str(&format!("Li[{key}:{}:{selection_mode:?}];", items.len())),
        AppWidget::Table {
            key, columns, rows, ..
        } => out.push_str(&format!("Ta[{key}:{}x{}];", columns.len(), rows.len())),
        AppWidget::Tree { key, nodes } => out.push_str(&format!("Tr[{key}:{}];", nodes.len())),
        AppWidget::Pagination { key, pages, .. } => {
            out.push_str(&format!("Pg[{key}:{pages}];"))
        }
        // Embedded runtimes: a change to the module/source is a STRUCTURAL change
        // (the emitted sub-model is re-rendered + remounted), so fold the module
        // identity into the signature. We do NOT recurse the (host-rendered)
        // inner model here — it does not exist until mount time; the remount on a
        // source change re-runs render() and rebuilds the subtree.
        AppWidget::WasmApp { module } => {
            let id = match module {
                WasmModuleSource::Path { path } => format!("p:{path}"),
                WasmModuleSource::Bytes { bytes } => format!("b:{}", bytes.len()),
            };
            out.push_str(&format!("Wa[{id}];"));
        }
        AppWidget::ScriptApp { source, lang } => {
            out.push_str(&format!("Sa[{lang:?}:{}];", source.len()));
        }
        // Video: a change to the source/flags is a structural change (the surface
        // rebinds to a new texture id / restarts), so fold them into the signature.
        AppWidget::Video {
            src,
            autoplay,
            loop_playback,
        } => {
            out.push_str(&format!("Vd[{src}:{autoplay}:{loop_playback}];"));
        }
        // Map: the centre/zoom drive the visible-tile set, so a pan/zoom IS a
        // structural change (the tile grid + their positions change). Folding the
        // viewport into the signature remounts the surface so the new tile slots
        // mount. (Rounded to keep tiny float jitter from over-remounting.)
        AppWidget::Map {
            center_lat,
            center_lon,
            zoom,
        } => {
            out.push_str(&format!(
                "Mp[{:.5}:{:.5}:{zoom}];",
                center_lat, center_lon
            ));
        }
    }
}

/// The index of the option whose `value` equals `selected`, if any.
fn selected_index(
    options: &[liquide_interop::WidgetOption],
    selected: Option<&str>,
) -> Option<usize> {
    let want = selected?;
    options.iter().position(|o| o.value == want)
}

/// Whether an [`AppWidget`] is a structural container (mounted as a plain DOM
/// wrapper) rather than an interactive behavior.
fn is_container(widget: &AppWidget) -> bool {
    matches!(
        widget,
        AppWidget::Panel { .. }
            | AppWidget::Card { .. }
            | AppWidget::GroupBox { .. }
            | AppWidget::Tabs { .. }
            | AppWidget::Toolbar { .. }
            | AppWidget::Accordion { .. }
    )
}

/// Mount an entire [`AppWidgetModel`] under `parent` for window `window_id`:
/// create container DOM wrappers directly and mount each interactive widget as
/// its own [`WidgetHost`] entry (so its runtime state is keyed and survives
/// reconciliation). Returns the namespaced widget ids that were mounted, in
/// document order.
///
/// The dispatcher is needed because [`WidgetHost::mount`] registers the
/// per-widget event handlers on it (the single source of truth for events).
pub(crate) fn mount_model_into(
    model: &AppWidgetModel,
    window_id: u64,
    parent: NodeId,
    host: &mut WidgetHost,
    doc: &mut Document,
    dispatcher: &mut liquide_hit_test::EventDispatcher,
) -> Vec<String> {
    let mut mounted = Vec::new();
    for widget in &model.root {
        mount_widget(widget, window_id, parent, host, doc, dispatcher, &mut mounted);
    }
    mounted
}

/// Recursively mount one widget (and its children, for containers).
fn mount_widget(
    widget: &AppWidget,
    window_id: u64,
    parent: NodeId,
    host: &mut WidgetHost,
    doc: &mut Document,
    dispatcher: &mut liquide_hit_test::EventDispatcher,
    mounted: &mut Vec<String>,
) {
    // Embedded-runtime node: instantiate the host, render the inner model, and
    // mount THAT inside a wrapper so the emitted UI flows through the normal
    // widget pipeline. Under the default build the Null host yields a graceful
    // placeholder model (see `render_wasm_app` / `render_script_app`).
    if let AppWidget::WasmApp { .. } | AppWidget::ScriptApp { .. } = widget {
        let inner = match widget {
            AppWidget::WasmApp { module } => render_wasm_app(module),
            AppWidget::ScriptApp { source, lang } => render_script_app(source, *lang),
            _ => unreachable!("embedded-runtime guard"),
        };
        // Wrap so the emitted UI is grouped (and a placeholder is themable via
        // `RUNTIME_PLACEHOLDER_CLASS`). The wrapper is a plain DOM element; the
        // inner model's widgets mount under it as their own keyed host entries,
        // so their actions still route through the standard apply_action loop.
        let wrapper = doc.create_element("lq-app-embed");
        if is_placeholder_model(&inner) {
            doc.set_attribute(wrapper, "class", RUNTIME_PLACEHOLDER_CLASS);
        }
        doc.append_child(parent, wrapper);
        for child in &inner.root {
            mount_widget(child, window_id, wrapper, host, doc, dispatcher, mounted);
        }
        return;
    }

    // Video node: a live RGBA surface (codec compiled in) or a "no codec"
    // placeholder (default build). Handled inline before the container check.
    if let AppWidget::Video {
        src,
        autoplay,
        loop_playback,
    } = widget
    {
        if video_codec_available() {
            // Mount a surface element bound to a stable texture id. The session
            // render loop owns the VideoSource and pushes frames into that id via
            // register_image_rgba each tick; the surface's Image scene node blits.
            let surface = doc.create_element("lq-video");
            let image_id = video_image_id(window_id, src);
            doc.set_attribute(surface, "data-video-id", &image_id.to_string());
            doc.set_attribute(surface, "data-video-src", src);
            if *autoplay {
                doc.set_attribute(surface, "data-autoplay", "true");
            }
            if *loop_playback {
                doc.set_attribute(surface, "data-loop", "true");
            }
            doc.append_child(parent, surface);
            // Record the namespaced id so callers can correlate the surface back
            // to its node (mirrors how interactive widgets push their mount id).
            mounted.push(widget_id(window_id, src));
        } else {
            // No codec: mount the graceful placeholder model through the normal
            // widget pipeline (same shape as the embedded-runtime fallback).
            let placeholder = video_placeholder_model(src);
            let wrapper = doc.create_element("lq-app-embed");
            doc.set_attribute(wrapper, "class", RUNTIME_PLACEHOLDER_CLASS);
            doc.append_child(parent, wrapper);
            for child in &placeholder.root {
                mount_widget(child, window_id, wrapper, host, doc, dispatcher, mounted);
            }
        }
        return;
    }

    // Map node: a slippy-map surface. Emit one positioned tile element per
    // visible tile (its screen rect from the viewport math); a loaded tile is
    // styled with `background-image: url(tile://z/x/y)` (the scene bridge turns
    // that into an Image node + the session decodes/registers the bytes under the
    // same key), and a not-yet-loaded tile gets the placeholder class. Handled
    // inline before the container check (it has no interop children).
    if let AppWidget::Map {
        center_lat,
        center_lon,
        zoom,
    } = widget
    {
        // Build the viewport + (offline) tile state and lay tiles out by screen
        // rect. No network here: `placement()` is pure math; tiles are "loaded"
        // only once the live session drive has fetched + cached them, so under
        // the default build every tile is a placeholder (the offline grid).
        let map_state = map_state_for(*center_lat, *center_lon, *zoom);
        let placement = map_state.placement();
        let any_loaded = placement.iter().any(|p| p.loaded);

        let surface = doc.create_element("lq-map");
        // The surface is a positioning context sized to the viewport box.
        doc.set_inline_style(surface, "position", "relative");
        doc.set_inline_style(surface, "width", &format!("{}px", MAP_SURFACE_WIDTH));
        doc.set_inline_style(surface, "height", &format!("{}px", MAP_SURFACE_HEIGHT));
        doc.set_attribute(surface, "data-map-zoom", &zoom.to_string());
        doc.set_attribute(surface, "data-map-lat", &center_lat.to_string());
        doc.set_attribute(surface, "data-map-lon", &center_lon.to_string());
        if !any_loaded {
            // No tiles loaded (the default no-net build) → tag the surface so a
            // theme can paint the placeholder grid + show the offline notice.
            doc.set_attribute(surface, "class", MAP_OFFLINE_CLASS);
        }
        doc.append_child(parent, surface);

        for p in &placement {
            let tile_el = doc.create_element("lq-map-tile");
            doc.set_inline_style(tile_el, "position", "absolute");
            doc.set_inline_style(tile_el, "left", &format!("{}px", p.tile.x));
            doc.set_inline_style(tile_el, "top", &format!("{}px", p.tile.y));
            doc.set_inline_style(tile_el, "width", &format!("{}px", p.tile.size));
            doc.set_inline_style(tile_el, "height", &format!("{}px", p.tile.size));
            // The stable image key — hashed to the renderer image id on both the
            // compositing side (scene bridge) and the session decode side.
            doc.set_attribute(tile_el, "data-tile-key", &p.image_key);
            if p.loaded {
                // Loaded: bind the texture via background-image so the scene
                // bridge emits an Image node keyed by hash(image_key).
                doc.set_inline_style(
                    tile_el,
                    "background-image",
                    &format!("url({})", p.image_key),
                );
            } else {
                // Not loaded yet: the placeholder tile (styled by the theme).
                doc.set_attribute(tile_el, "class", "map-tile-placeholder");
            }
            doc.append_child(surface, tile_el);
        }

        // An offline notice element (text reaches the DOM so a default build
        // tells the user tiles can't be fetched). Present only when nothing is
        // loaded, so a live (net) build with tiles shows no notice.
        if !any_loaded {
            let notice = doc.create_element("lq-map-notice");
            let detail = if map_tile_fetch_available() {
                "map tiles loading…"
            } else {
                "map offline: tile fetching unavailable (build without network)"
            };
            let txt = doc.create_text(detail);
            doc.append_child(notice, txt);
            doc.append_child(surface, notice);
        }
        return;
    }

    if is_container(widget) {
        // Create a plain DOM wrapper element (styled by widgets.css) and mount
        // the interactive children inside it, so nesting is preserved while each
        // interactive widget keeps its own keyed host state.
        // The optional class pins the wrapper's layout intent. The toolbar in
        // particular MUST carry `horizontal` so `lq-toolbar.horizontal` resolves
        // `flex-direction: row`; without it the engine's default cross-axis
        // (column) stacks the nav buttons vertically + oversized (t186 bucket D2).
        let (tag, class, caption, children): (&str, Option<&str>, Option<&str>, Vec<&AppWidget>) =
            match widget {
                AppWidget::Panel { children } => {
                    ("lq-panel", None, None, children.iter().collect())
                }
                AppWidget::Card { title, children } => {
                    ("lq-card", None, title.as_deref(), children.iter().collect())
                }
                AppWidget::GroupBox { label, children } => (
                    "lq-groupbox",
                    None,
                    Some(label.as_str()),
                    children.iter().collect(),
                ),
                AppWidget::Toolbar { children } => (
                    "lq-toolbar",
                    Some("horizontal"),
                    None,
                    children.iter().collect(),
                ),
            AppWidget::Tabs { tabs, selected } => {
                // Flatten to the selected tab's children (a structural tab strip
                // would need its own behavior; the selected panel is what paints).
                let sel = *selected as usize;
                let panel = doc.create_element("lq-tabs");
                doc.append_child(parent, panel);
                if let Some(tab) = tabs.get(sel) {
                    for child in &tab.children {
                        mount_widget(child, window_id, panel, host, doc, dispatcher, mounted);
                    }
                }
                return;
            }
            AppWidget::Accordion { sections } => {
                let acc = doc.create_element("lq-accordion");
                doc.append_child(parent, acc);
                for section in sections {
                    if section.expanded {
                        for child in &section.children {
                            mount_widget(child, window_id, acc, host, doc, dispatcher, mounted);
                        }
                    }
                }
                return;
            }
            _ => unreachable!("is_container guard"),
        };

        let wrapper = doc.create_element(tag);
        if let Some(c) = class {
            doc.add_class(wrapper, c);
        }
        doc.append_child(parent, wrapper);
        if let Some(cap) = caption {
            let cap_el = doc.create_element("lq-caption");
            let txt = doc.create_text(cap);
            doc.append_child(cap_el, txt);
            doc.append_child(wrapper, cap_el);
        }
        for child in children {
            mount_widget(child, window_id, wrapper, host, doc, dispatcher, mounted);
        }
        return;
    }

    // Interactive / leaf widget: mount as its own keyed host entry.
    let Some(behavior) = behavior_for(widget) else {
        return;
    };
    // Use the widget's stable key when it has one; otherwise synthesize a stable
    // ordinal so non-keyed leaves (labels, chips) still get a unique mount id.
    let key = widget
        .key()
        .map(str::to_string)
        .unwrap_or_else(|| format!("leaf{}", mounted.len()));
    let id = widget_id(window_id, &key);
    host.mount(id.clone(), behavior, doc, parent, dispatcher);
    mounted.push(id);
}

/// Translate a toolkit [`liquide_widgets::WidgetAction`] into the plain-data
/// [`AppWidgetAction`] the app's `apply_action` consumes.
///
/// `widget` is the model node that emitted the action (looked up by stripped
/// key), used to choose the verb. `app_key` is the app-side key (already
/// stripped of the per-window namespace).
#[must_use]
pub(crate) fn translate_action(
    app_key: &str,
    model_widget: Option<&AppWidget>,
    action: &liquide_widgets::WidgetAction,
) -> AppWidgetAction {
    let mut payload = action.payload.clone().unwrap_or_default();

    // CANONICAL TABLE-SORT CONTRACT (chokepoint normalization).
    //
    // The `liquide_widgets::Table` emits its sort action with the payload
    // `"<col>:<dir>"` (e.g. `"0:asc"`, see `table.rs::sort_by`) — it carries the
    // direction it just toggled to. But every Table-consuming app (task-manager,
    // files, …) toggles the direction *itself* on a re-click of the same column
    // (`ProcessSortColumn` / `DirectoryListing::set_sort`), so the toolkit's
    // direction suffix is redundant — and an app that parses the payload as a bare
    // column index silently drops the whole header click (the t124 bug the e2e
    // harness surfaced). Rather than make each app defensively re-parse
    // `"col:dir"` (duplicated, easy to regress in a future Table consumer), we
    // normalize here, at the single shell-side chokepoint that already translates
    // toolkit → interop: a Table `sorted` payload is reduced to the bare column
    // index `"<col>"`. So EVERY app — present and future — receives the one
    // documented, stable sort payload form: the column index, nothing else.
    if matches!(model_widget, Some(AppWidget::Table { .. })) && action.name == "sorted" {
        if let Some((col, _dir)) = payload.split_once(':') {
            payload = col.to_string();
        }
    }

    // Map the toolkit verb (+ the target widget family) to the interop verb the
    // app understands. The toolkit emits a small set of names: "click",
    // "changed", "navigate", "toggled", "sorted", "remove".
    let name: &str = match (model_widget, action.name.as_str()) {
        (Some(AppWidget::Button { .. }), _) => "click",
        (Some(AppWidget::Checkbox { .. }), _) | (Some(AppWidget::Switch { .. }), _) => "toggle",
        (Some(AppWidget::TextInput { .. }), "changed") => "change",
        (Some(AppWidget::TextArea { .. }), "changed") => "change",
        (Some(AppWidget::Slider { .. }), "changed") => "change",
        (Some(AppWidget::Dropdown { .. }), "changed") => "select",
        (Some(AppWidget::Segmented { .. }), "changed") => "select",
        (Some(AppWidget::RadioGroup { .. }), "changed") => "select",
        (Some(AppWidget::List { .. }), "changed") => "select",
        (Some(AppWidget::Table { .. }), "sorted") => "sort",
        (Some(AppWidget::Table { .. }), "changed") => "select",
        (Some(AppWidget::Tree { .. }), "toggled") => "toggle",
        (Some(AppWidget::Tree { .. }), "changed") => "select",
        (Some(AppWidget::Accordion { .. }), "toggled") => "toggle",
        (Some(AppWidget::Pagination { .. }), "changed") => "navigate",
        (Some(AppWidget::Breadcrumb { .. }), "navigate") => "navigate",
        (Some(AppWidget::Link { .. }), "navigate") => "navigate",
        (Some(AppWidget::Chip { .. }), "remove") => "remove",
        (Some(AppWidget::Chip { .. }), "changed") => "toggle",
        // Fall back to the toolkit verb verbatim for anything unmapped.
        (_, other) => other,
    };
    AppWidgetAction::new(app_key.to_string(), name.to_string(), payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_interop::{TableColumn, WidgetOption};

    #[test]
    fn widget_id_round_trips() {
        let id = widget_id(7, "opacity");
        assert_eq!(id, "aw-7-opacity");
        assert_eq!(strip_widget_id(7, &id), "opacity");
        // A key containing dashes survives the strip (only the prefix is removed).
        let id2 = widget_id(3, "sort-by-name");
        assert_eq!(strip_widget_id(3, &id2), "sort-by-name");
    }

    #[test]
    fn behavior_for_maps_each_interactive_family() {
        assert!(behavior_for(&AppWidget::Button {
            id: "b".into(),
            label: "Save".into(),
            kind: ButtonKind::Primary,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Checkbox {
            key: "c".into(),
            checked: true,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Slider {
            key: "s".into(),
            min: 0.0,
            max: 1.0,
            step: 0.1,
            value: 0.5,
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Dropdown {
            key: "d".into(),
            options: vec![WidgetOption::new("a"), WidgetOption::new("b")],
            selected: Some("b".into()),
        })
        .is_some());
        assert!(behavior_for(&AppWidget::Table {
            key: "t".into(),
            columns: vec![TableColumn::new("PID")],
            rows: vec![vec!["1".into()]],
            sort: None,
            selection_mode: SelectionMode::Single,
            selected: vec![],
        })
        .is_some());
        // Containers are NOT behaviors.
        assert!(behavior_for(&AppWidget::Panel { children: vec![] }).is_none());
    }

    #[test]
    fn behavior_for_textarea_produces_a_multiline_textarea_with_key_value_and_gutter() {
        // The TextArea node must map to a real liquide_widgets::TextArea carrying
        // the model's full (newline-bearing) text and the gutter flag — NOT a
        // single-line TextInput. Downcast the behavior to prove the concrete type.
        let widget = AppWidget::TextArea {
            key: "document".into(),
            value: "line one\nline two\nline three".into(),
            gutter: true,
            readonly: false,
        };
        let behavior = behavior_for(&widget).expect("TextArea maps to a behavior");

        // It must be a TextArea (the multi-line widget), not a TextInput.
        assert!(
            behavior.as_any().downcast_ref::<TextInput>().is_none(),
            "TextArea must NOT map to the single-line TextInput"
        );
        let ta = behavior
            .as_any()
            .downcast_ref::<TextArea>()
            .expect("behavior must be a liquide_widgets::TextArea");

        // The full multi-line value round-trips (the newline is preserved).
        assert_eq!(ta.text(), "line one\nline two\nline three");
        assert_eq!(ta.line_count(), 3);
        // The gutter flag flowed through.
        assert!(ta.gutter_visible(), "gutter flag must reach the widget");
    }

    #[test]
    fn behavior_for_textarea_readonly_disables_the_widget() {
        let widget = AppWidget::TextArea {
            key: "doc".into(),
            value: "ro".into(),
            gutter: false,
            readonly: true,
        };
        let behavior = behavior_for(&widget).expect("behavior");
        let ta = behavior
            .as_any()
            .downcast_ref::<TextArea>()
            .expect("TextArea");
        // A read-only TextArea is not focusable (disabled).
        assert!(!ta.focusable(), "readonly TextArea must be disabled");
    }

    #[test]
    fn translate_action_textarea_changed_maps_to_change_with_full_text() {
        let a = translate_action(
            "document",
            Some(&AppWidget::TextArea {
                key: "document".into(),
                value: "old".into(),
                gutter: false,
                readonly: false,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-document".into(),
                name: "changed".into(),
                payload: Some("new\nbody".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("document", "change", "new\nbody"));
    }

    #[test]
    fn translate_action_table_sorted_normalizes_to_bare_column_index() {
        // The toolkit Table emits the REAL payload "<col>:<dir>" (here "0:asc",
        // exactly as `table.rs::sort_by` produces). The chokepoint must normalize
        // it to the bare column index "0" so every consuming app's bare-u32 parse
        // succeeds — this is the t124 wiring fix. Asserting on the realistic
        // "<col>:<dir>" form (NOT a hand-cleaned "0") is the no-fake-green teeth:
        // before the fix this produced AppWidgetAction(.., "sort", "0:asc").
        let table = AppWidget::Table {
            key: "process_table".into(),
            columns: vec![TableColumn::new("Name"), TableColumn::new("PID")],
            rows: vec![vec!["a".into(), "1".into()]],
            sort: None,
            selection_mode: SelectionMode::Single,
            selected: vec![],
        };
        // Ascending.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("0:asc".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "0"));

        // Descending (re-click) — the direction suffix is also stripped; the app
        // owns the toggle, so it only needs the column.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("1:desc".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "1"));

        // A bare column index (no suffix) passes through unchanged — idempotent.
        let a = translate_action(
            "process_table",
            Some(&table),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-process_table".into(),
                name: "sorted".into(),
                payload: Some("0".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("process_table", "sort", "0"));
    }

    #[test]
    fn translate_action_picks_the_right_verb() {
        // Button → click.
        let a = translate_action(
            "save",
            Some(&AppWidget::Button {
                id: "save".into(),
                label: "Save".into(),
                kind: ButtonKind::Normal,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-save".into(),
                name: "click".into(),
                payload: None,
            },
        );
        assert_eq!(a, AppWidgetAction::new("save", "click", ""));

        // Slider changed → change with the value payload.
        let a = translate_action(
            "opacity",
            Some(&AppWidget::Slider {
                key: "opacity".into(),
                min: 0.0,
                max: 1.0,
                step: 0.05,
                value: 0.5,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-opacity".into(),
                name: "changed".into(),
                payload: Some("0.75".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("opacity", "change", "0.75"));

        // Dropdown changed → select with the chosen value.
        let a = translate_action(
            "theme",
            Some(&AppWidget::Dropdown {
                key: "theme".into(),
                options: vec![WidgetOption::new("Light"), WidgetOption::new("Dark")],
                selected: Some("Dark".into()),
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-theme".into(),
                name: "changed".into(),
                payload: Some("Light".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("theme", "select", "Light"));

        // Checkbox → toggle.
        let a = translate_action(
            "wifi",
            Some(&AppWidget::Checkbox {
                key: "wifi".into(),
                checked: false,
            }),
            &liquide_widgets::WidgetAction {
                widget: "aw-1-wifi".into(),
                name: "changed".into(),
                payload: Some("true".into()),
            },
        );
        assert_eq!(a, AppWidgetAction::new("wifi", "toggle", "true"));
    }

    // ════════════════════════════════════════════════════════════════════════
    // Embedded-runtime wiring (t152): WasmApp / ScriptApp.
    //
    // These run under the DEFAULT build (no `wasm-apps` / `script-apps`
    // feature), so the host crates resolve to their `Null*` stubs. The contract
    // under test: a WasmApp / ScriptApp node is HANDLED (never panics, never an
    // empty hole) and yields the graceful "runtime unavailable" placeholder —
    // and that placeholder MOUNTS through the normal widget pipeline. The
    // feature-on path (a real host's emitted model mounting) is exercised by the
    // host crates' own sandbox tests (t139/t140), which compile a real module
    // and assert `render()` returns a deserialised AppWidgetModel; this crate
    // can't enable those heavy features in a default `cargo test` run, so it
    // proves the wiring up to the host boundary and the placeholder fallback.
    // ════════════════════════════════════════════════════════════════════════

    use liquide_dom::{Document, NodeId};
    use liquide_hit_test::EventDispatcher;
    use liquide_interop::{ScriptLang, WasmModuleSource};
    use liquide_widgets::host::WidgetHost;

    /// Collect every text node's content under `node` (depth-first), joined by
    /// spaces, so a test can assert the placeholder message reached the DOM.
    fn collect_text(doc: &Document, node: NodeId) -> String {
        let mut out = String::new();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if let Some(node_ref) = doc.get(n) {
                if let Some(t) = node_ref.text_content() {
                    if !t.is_empty() {
                        out.push_str(t);
                        out.push(' ');
                    }
                }
            }
            // Push children (reverse so traversal is left-to-right; order is not
            // asserted, only membership).
            for &c in doc.children(n) {
                stack.push(c);
            }
        }
        out
    }

    /// Whether any element at/under `node` has tag `tag`.
    fn has_tag(doc: &Document, node: NodeId, tag: &str) -> bool {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if doc.tag_name(n).as_deref() == Some(tag) {
                return true;
            }
            for &c in doc.children(n) {
                stack.push(c);
            }
        }
        false
    }

    #[test]
    fn wasm_app_default_build_renders_the_unavailable_placeholder() {
        // The Null wasm host reports Unavailable, so render_wasm_app must return
        // the placeholder model (NOT panic, NOT an empty model).
        let model = render_wasm_app(&WasmModuleSource::Path {
            path: "does/not/matter.wasm".into(),
        });
        assert!(
            is_placeholder_model(&model),
            "default (Null) build must yield the placeholder, got {model:?}"
        );
        // The message names the runtime and the unavailable reason.
        let text = match &model.root[..] {
            [AppWidget::GroupBox { children, .. }] => match &children[..] {
                [AppWidget::Label { text }] => text.clone(),
                other => panic!("expected one Label, got {other:?}"),
            },
            other => panic!("expected one GroupBox, got {other:?}"),
        };
        assert!(text.contains("WASM"), "message: {text}");
        assert!(text.contains("runtime unavailable"), "message: {text}");
    }

    #[test]
    fn script_app_default_build_renders_the_unavailable_placeholder() {
        let model = render_script_app(
            "export function render(){ return { root: [] }; }",
            ScriptLang::TypeScript,
        );
        assert!(
            is_placeholder_model(&model),
            "default (Null) build must yield the placeholder, got {model:?}"
        );
        let text = collect_placeholder_text(&model);
        assert!(text.contains("Script"), "message: {text}");
        assert!(text.contains("runtime unavailable"), "message: {text}");
    }

    /// Helper: pull the placeholder Label text out of a placeholder model.
    fn collect_placeholder_text(model: &AppWidgetModel) -> String {
        match &model.root[..] {
            [AppWidget::GroupBox { children, .. }] => match &children[..] {
                [AppWidget::Label { text }] => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }

    #[test]
    fn mounting_a_wasm_app_node_emits_the_placeholder_into_the_dom() {
        // The whole mapper path: a model containing a WasmApp node mounts without
        // panicking and the placeholder notice reaches the DOM under the host
        // node, tagged so a theme can style it.
        let model = AppWidgetModel::with_root(vec![AppWidget::WasmApp {
            module: WasmModuleSource::Bytes {
                bytes: vec![0, 1, 2, 3],
            },
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let mounted = mount_model_into(&model, 7, root, &mut host, &mut doc, &mut dispatcher);

        // The embed wrapper element exists and carries the placeholder class.
        assert!(
            has_tag(&doc, root, "lq-app-embed"),
            "embed wrapper must be mounted"
        );
        // The placeholder Label mounted as its own keyed host entry (so it is a
        // real widget in the pipeline, not a dangling element).
        assert!(!mounted.is_empty(), "placeholder Label must mount as a widget");
        // The unavailable message reached the DOM text.
        let text = collect_text(&doc, root);
        assert!(
            text.contains("runtime unavailable"),
            "placeholder text must reach the DOM, got: {text:?}"
        );
    }

    #[test]
    fn mounting_a_script_app_node_emits_the_placeholder_into_the_dom() {
        let model = AppWidgetModel::with_root(vec![AppWidget::ScriptApp {
            source: "export function render(){return{root:[]}}".into(),
            lang: ScriptLang::JavaScript,
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let mounted = mount_model_into(&model, 1, root, &mut host, &mut doc, &mut dispatcher);

        assert!(has_tag(&doc, root, "lq-app-embed"));
        assert!(!mounted.is_empty());
        let text = collect_text(&doc, root);
        assert!(
            text.contains("runtime unavailable"),
            "placeholder text must reach the DOM, got: {text:?}"
        );
    }

    #[test]
    fn embedded_runtime_node_does_not_break_sibling_mounting() {
        // A WasmApp node next to a normal Button: BOTH must mount. This proves
        // the embedded node is handled inline without aborting the walk (the
        // anti-fake-green tooth: a node left unhandled would either panic on the
        // exhaustive match or skip the sibling).
        let model = AppWidgetModel::with_root(vec![AppWidget::Panel {
            children: vec![
                AppWidget::WasmApp {
                    module: WasmModuleSource::Path {
                        path: "x.wasm".into(),
                    },
                },
                AppWidget::Button {
                    id: "ok".into(),
                    label: "OK".into(),
                    kind: ButtonKind::Primary,
                },
            ],
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let mounted = mount_model_into(&model, 3, root, &mut host, &mut doc, &mut dispatcher);

        // The Button sibling mounted (its namespaced id is present).
        assert!(
            mounted.iter().any(|id| id == "aw-3-ok"),
            "the Button sibling must still mount past the WasmApp node, got {mounted:?}"
        );
        // The embed wrapper is present too.
        assert!(has_tag(&doc, root, "lq-app-embed"));
    }

    #[test]
    fn embedded_runtime_node_contributes_to_the_structure_signature() {
        // The structure signature MUST change when the module/source changes, so
        // a different embedded module remounts (re-runs render()). Two WasmApps
        // with different paths must hash differently; identical ones the same.
        let sig = |w: AppWidget| model_structure(&AppWidgetModel::with_root(vec![w]));

        let a = sig(AppWidget::WasmApp {
            module: WasmModuleSource::Path { path: "a.wasm".into() },
        });
        let b = sig(AppWidget::WasmApp {
            module: WasmModuleSource::Path { path: "b.wasm".into() },
        });
        assert_ne!(a, b, "different wasm modules must remount");

        let a2 = sig(AppWidget::WasmApp {
            module: WasmModuleSource::Path { path: "a.wasm".into() },
        });
        assert_eq!(a, a2, "identical wasm modules must NOT remount");

        // Script: different source length / lang must differ from wasm and each
        // other; the signature is non-empty (the node is actually accounted for).
        let s1 = sig(AppWidget::ScriptApp {
            source: "render()".into(),
            lang: ScriptLang::TypeScript,
        });
        let s2 = sig(AppWidget::ScriptApp {
            source: "render(){}".into(),
            lang: ScriptLang::TypeScript,
        });
        assert!(!s1.is_empty());
        assert_ne!(s1, s2, "different script sources must remount");
        assert_ne!(s1, a, "a script node and a wasm node are distinct shapes");
    }

    // ════════════════════════════════════════════════════════════════════════
    // Silent <video> surface (t155).
    //
    // These run under the DEFAULT build (no `video` feature → NullVideoSource),
    // so a Video node mounts the "no codec" placeholder. The feature-on surface
    // path (lq-video element + stable image id) is asserted by the codec-on test
    // (gated on the feature) below; the live poll→register_image_rgba drive is in
    // liquide-session (render_thread.rs).
    // ════════════════════════════════════════════════════════════════════════

    #[test]
    fn video_image_id_is_stable_and_distinct() {
        // Same window + src → same id (stable across frames).
        let a = video_image_id(7, "clip.ivf");
        let b = video_image_id(7, "clip.ivf");
        assert_eq!(a, b, "id must be stable for a given (window, src)");
        // Different src → different id.
        assert_ne!(a, video_image_id(7, "other.ivf"));
        // Different window → different id.
        assert_ne!(a, video_image_id(8, "clip.ivf"));
        // High bit set (kept clear of small wallpaper ids).
        assert_ne!(a & 0x4000_0000_0000_0000, 0);
    }

    #[test]
    #[cfg(not(feature = "video"))]
    fn video_node_default_build_mounts_the_no_codec_placeholder() {
        // No codec compiled in: a Video node mounts the placeholder notice into
        // the DOM (tagged so a theme can style it), and does NOT mount an
        // lq-video surface (there is nothing to feed it).
        let model = AppWidgetModel::with_root(vec![AppWidget::Video {
            src: "movies/demo.ivf".into(),
            autoplay: true,
            loop_playback: false,
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let mounted = mount_model_into(&model, 5, root, &mut host, &mut doc, &mut dispatcher);

        // No live surface element in the default build.
        assert!(
            !has_tag(&doc, root, "lq-video"),
            "default build must NOT mount a video surface"
        );
        // The placeholder wrapper + Label mounted instead.
        assert!(has_tag(&doc, root, "lq-app-embed"), "placeholder wrapper present");
        assert!(!mounted.is_empty(), "placeholder Label mounts as a widget");
        let text = collect_text(&doc, root);
        assert!(
            text.contains("video unavailable") && text.contains("no codec"),
            "placeholder text must reach the DOM, got: {text:?}"
        );
        assert!(text.contains("movies/demo.ivf"), "names the src: {text:?}");
    }

    #[test]
    #[cfg(not(feature = "video"))]
    fn video_node_does_not_break_sibling_mounting() {
        // A Video node next to a Button: both must mount (the walk is not aborted).
        let model = AppWidgetModel::with_root(vec![AppWidget::Panel {
            children: vec![
                AppWidget::Video {
                    src: "v.ivf".into(),
                    autoplay: false,
                    loop_playback: false,
                },
                AppWidget::Button {
                    id: "ok".into(),
                    label: "OK".into(),
                    kind: ButtonKind::Primary,
                },
            ],
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let mounted = mount_model_into(&model, 3, root, &mut host, &mut doc, &mut dispatcher);
        assert!(
            mounted.iter().any(|id| id == "aw-3-ok"),
            "Button sibling must still mount past the Video node, got {mounted:?}"
        );
    }

    #[test]
    #[cfg(feature = "video")]
    fn video_node_codec_build_mounts_a_surface_bound_to_a_stable_id() {
        // With the codec compiled in, a Video node mounts an lq-video surface
        // carrying the stable texture id + src the render loop uses.
        let src = "movies/demo.ivf";
        let model = AppWidgetModel::with_root(vec![AppWidget::Video {
            src: src.into(),
            autoplay: true,
            loop_playback: true,
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();

        let _ = mount_model_into(&model, 9, root, &mut host, &mut doc, &mut dispatcher);
        assert!(has_tag(&doc, root, "lq-video"), "surface element must mount");
        // The surface carries the stable image id.
        let expect_id = video_image_id(9, src).to_string();
        let mut found = false;
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if doc.tag_name(n).as_deref() == Some("lq-video") {
                assert_eq!(doc.get_attribute(n, "data-video-id").as_deref(), Some(expect_id.as_str()));
                assert_eq!(doc.get_attribute(n, "data-video-src").as_deref(), Some(src));
                found = true;
            }
            for &c in doc.children(n) {
                stack.push(c);
            }
        }
        assert!(found, "lq-video surface with id binding");
    }

    #[test]
    fn apply_action_to_embedded_runtime_default_build_yields_placeholder() {
        // Routing an action into a Null host's apply_action + re-render must not
        // panic and must surface the placeholder (the Null host reports
        // Unavailable for both apply_action and render).
        let action = AppWidgetAction::new("inner", "click", "");
        let wasm = apply_action_to_wasm_app(
            &WasmModuleSource::Bytes { bytes: vec![0] },
            &action,
        );
        assert!(is_placeholder_model(&wasm));

        let script = apply_action_to_script_app("x", ScriptLang::TypeScript, &action);
        assert!(is_placeholder_model(&script));
    }

    // ════════════════════════════════════════════════════════════════════════
    // OpenStreetMap slippy-map surface (t159).
    //
    // Default build (no `map` feature → no tile fetch): a Map node mounts the
    // surface with positioned placeholder tiles + an offline notice. The
    // positioned-Image emit + tile decode/register live drive is in the session
    // (render_thread.rs::push_map_tile); these prove the mapper end.
    // ════════════════════════════════════════════════════════════════════════

    /// Collect every element with tag `tag` under `node`.
    fn nodes_with_tag(doc: &Document, node: NodeId, tag: &str) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if doc.tag_name(n).as_deref() == Some(tag) {
                out.push(n);
            }
            for &c in doc.children(n) {
                stack.push(c);
            }
        }
        out
    }

    #[test]
    fn map_node_mounts_a_surface_with_positioned_tiles() {
        // A Map node emits an lq-map surface containing one positioned tile
        // element per visible tile, each at the screen rect the viewport math
        // produced and bound to a stable image key.
        let model = AppWidgetModel::with_root(vec![AppWidget::Map {
            center_lat: 0.0,
            center_lon: 0.0,
            zoom: 2,
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();
        let _ = mount_model_into(&model, 1, root, &mut host, &mut doc, &mut dispatcher);

        let surface = nodes_with_tag(&doc, root, "lq-map");
        assert_eq!(surface.len(), 1, "exactly one map surface");
        let tiles = nodes_with_tag(&doc, root, "lq-map-tile");
        // The viewport math drives the count; it must equal the placement and be
        // > 0 (a real tiled grid, not an empty surface).
        let expected = map_state_for(0.0, 0.0, 2).placement();
        assert!(!expected.is_empty());
        assert_eq!(
            tiles.len(),
            expected.len(),
            "one tile element per visible tile"
        );
        // Each tile carries its stable image key AND an absolute screen position
        // that matches the viewport math (NOT a constant) — the anti-fake-green
        // tooth: the tiles are placed by the slippy math, so the set of keys must
        // equal the set of placement keys, and each key's left/top must equal its
        // computed screen rect.
        for t in &tiles {
            let key = doc
                .get_attribute(*t, "data-tile-key")
                .expect("tile key attr");
            let p = expected
                .iter()
                .find(|p| p.image_key == key)
                .unwrap_or_else(|| panic!("tile key {key} not in placement"));
            assert_eq!(
                doc.get_inline_style(*t, "left").as_deref(),
                Some(format!("{}px", p.tile.x).as_str()),
                "tile {key} left must match the viewport math"
            );
            assert_eq!(
                doc.get_inline_style(*t, "top").as_deref(),
                Some(format!("{}px", p.tile.y).as_str()),
                "tile {key} top must match the viewport math"
            );
        }
    }

    #[test]
    #[cfg(not(feature = "map"))]
    fn map_node_default_build_is_offline_with_placeholder_tiles_and_notice() {
        // No tile fetch compiled in → every tile is a placeholder, the surface is
        // tagged offline, and an offline notice reaches the DOM. No panic.
        let model = AppWidgetModel::with_root(vec![AppWidget::Map {
            center_lat: 48.8566,
            center_lon: 2.3522,
            zoom: 4,
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();
        let _ = mount_model_into(&model, 2, root, &mut host, &mut doc, &mut dispatcher);

        let surface = nodes_with_tag(&doc, root, "lq-map");
        assert_eq!(surface.len(), 1);
        assert_eq!(
            doc.get_attribute(surface[0], "class").as_deref(),
            Some(MAP_OFFLINE_CLASS),
            "offline surface must be tagged"
        );
        // Every tile is a placeholder (none bound to a background-image texture).
        let tiles = nodes_with_tag(&doc, root, "lq-map-tile");
        assert!(!tiles.is_empty());
        for t in &tiles {
            assert_eq!(
                doc.get_attribute(*t, "class").as_deref(),
                Some("map-tile-placeholder"),
                "offline tile must be a placeholder"
            );
            assert!(
                doc.get_inline_style(*t, "background-image").is_none(),
                "offline tile must NOT bind a texture"
            );
        }
        // The offline notice text reaches the DOM.
        let text = collect_text(&doc, root);
        assert!(
            text.contains("offline") && text.contains("unavailable"),
            "offline notice must reach the DOM, got: {text:?}"
        );
    }

    #[test]
    fn map_node_does_not_break_sibling_mounting() {
        // A Map node next to a Button: both must mount (the walk is not aborted).
        let model = AppWidgetModel::with_root(vec![AppWidget::Panel {
            children: vec![
                AppWidget::Map {
                    center_lat: 0.0,
                    center_lon: 0.0,
                    zoom: 1,
                },
                AppWidget::Button {
                    id: "ok".into(),
                    label: "OK".into(),
                    kind: ButtonKind::Primary,
                },
            ],
        }]);
        let mut doc = Document::new();
        let root = doc.root();
        let mut host = WidgetHost::new();
        let mut dispatcher = EventDispatcher::new();
        let mounted = mount_model_into(&model, 3, root, &mut host, &mut doc, &mut dispatcher);
        assert!(
            mounted.iter().any(|id| id == "aw-3-ok"),
            "Button sibling must still mount past the Map node, got {mounted:?}"
        );
    }

    #[test]
    fn map_pan_action_shifts_the_centre_and_remounts() {
        // A drag pans the viewport: apply_map_action moves the centre, and the
        // structure signature changes (so the surface remounts with new tiles).
        let (lat0, lon0, z0) = (0.0_f64, 0.0_f64, 4_u32);
        let sig_before = {
            let mut s = String::new();
            structure_into(
                &AppWidget::Map {
                    center_lat: lat0,
                    center_lon: lon0,
                    zoom: z0,
                },
                &mut s,
            );
            s
        };
        // Drag content left → centre moves east (lon increases).
        let (lat1, lon1, z1) =
            apply_map_action(lat0, lon0, z0, &AppWidgetAction::new("map", "pan", "-256,0"));
        assert_eq!(z1, z0, "pan does not change zoom");
        assert!(lon1 > lon0, "dragging left pans east: {lon0} -> {lon1}");
        assert!((lat1 - lat0).abs() < 1e-6, "horizontal pan keeps lat");
        let sig_after = {
            let mut s = String::new();
            structure_into(
                &AppWidget::Map {
                    center_lat: lat1,
                    center_lon: lon1,
                    zoom: z1,
                },
                &mut s,
            );
            s
        };
        assert_ne!(sig_before, sig_after, "a pan must change the remount signature");
    }

    #[test]
    fn map_zoom_action_changes_the_zoom_level() {
        let (_, _, z_in) =
            apply_map_action(0.0, 0.0, 4, &AppWidgetAction::new("map", "zoom", "1"));
        assert_eq!(z_in, 5, "zoom +1");
        let (_, _, z_out) =
            apply_map_action(0.0, 0.0, 4, &AppWidgetAction::new("map", "zoom", "-1"));
        assert_eq!(z_out, 3, "zoom -1");
        // Wheel-zoom toward an anchor also changes zoom (and keeps the anchor
        // point fixed; that property is unit-tested in liquide-map).
        let (_, _, z_anchor) = apply_map_action(
            40.0,
            -74.0,
            5,
            &AppWidgetAction::new("map", "zoom", "1@320,80"),
        );
        assert_eq!(z_anchor, 6, "anchored zoom +1");
        // An unparseable/unknown action leaves the viewport untouched.
        let (la, lo, zz) =
            apply_map_action(40.0, -74.0, 5, &AppWidgetAction::new("map", "wat", "x"));
        assert!((la - 40.0).abs() < 1e-6 && (lo + 74.0).abs() < 1e-6 && zz == 5);
    }
}

//! Application launcher for the shell surface.
//!
//! Provides fuzzy search over installed applications, inline calculator
//! evaluation, custom commands, and web-search fallback.  The launcher
//! maintains its own list of favourites, tracks per-app launch frequency,
//! and supports both list and grid presentation modes.

use std::fmt;

use serde::{Deserialize, Serialize};

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNode;

use crate::calculator::{self, CalcResult};

// ---------------------------------------------------------------------------
// LauncherView
// ---------------------------------------------------------------------------

/// Presentation mode for the launcher overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LauncherView {
    /// One-column scrollable list.
    #[default]
    List,
    /// Icon grid.
    Grid,
}

impl fmt::Display for LauncherView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::List => write!(f, "List"),
            Self::Grid => write!(f, "Grid"),
        }
    }
}

// ---------------------------------------------------------------------------
// AppCategory
// ---------------------------------------------------------------------------

/// Coarse category for a desktop application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppCategory {
    System,
    Development,
    Internet,
    Office,
    Media,
    Graphics,
    Utilities,
    Games,
    Settings,
    #[default]
    Other,
}

impl fmt::Display for AppCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => write!(f, "System"),
            Self::Development => write!(f, "Development"),
            Self::Internet => write!(f, "Internet"),
            Self::Office => write!(f, "Office"),
            Self::Media => write!(f, "Media"),
            Self::Graphics => write!(f, "Graphics"),
            Self::Utilities => write!(f, "Utilities"),
            Self::Games => write!(f, "Games"),
            Self::Settings => write!(f, "Settings"),
            Self::Other => write!(f, "Other"),
        }
    }
}

// ---------------------------------------------------------------------------
// LauncherConfig
// ---------------------------------------------------------------------------

/// Persistent configuration for the application launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    /// Which view mode to use when the launcher opens.
    pub default_view: LauncherView,
    /// Whether to display the favourites row.
    pub show_favorites: bool,
    /// Whether to display recently-launched applications.
    pub show_recent: bool,
    /// Maximum number of recent apps to display.
    pub recent_count: usize,
    /// Allow the search to include file-system results.
    pub search_files: bool,
    /// Allow the search to fall back to a web query.
    pub search_web: bool,
    /// Enable the inline calculator on math-like queries.
    pub calculator_enabled: bool,
    /// Show a workspace switcher strip inside the launcher.
    pub workspace_switcher: bool,
    /// Maximum number of pinned favourites.
    pub max_favorites: usize,
    /// Enable open/close transition animations.
    pub animation_enabled: bool,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            default_view: LauncherView::default(),
            show_favorites: true,
            show_recent: true,
            recent_count: 10,
            search_files: false,
            search_web: false,
            calculator_enabled: true,
            workspace_switcher: false,
            max_favorites: 9,
            animation_enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// LauncherApp
// ---------------------------------------------------------------------------

/// Metadata for a single application known to the launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherApp {
    /// Unique application identifier (e.g. `"org.gnome.Terminal"`).
    pub app_id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional one-line description.
    pub description: Option<String>,
    /// Optional icon name or path.
    pub icon: Option<String>,
    /// Optional command line used to launch the application.
    pub exec: Option<String>,
    /// Categories the application belongs to.
    pub categories: Vec<AppCategory>,
    /// Extra keywords for search matching.
    pub keywords: Vec<String>,
    /// Whether the application should run inside a terminal emulator.
    pub terminal: bool,
    /// Whether the application should be hidden from the launcher listing.
    pub no_display: bool,
    /// How many times this application has been launched.
    pub launch_count: u32,
    /// Monotonic timestamp (microseconds) of the last launch.
    pub last_launched_us: u64,
}

// ---------------------------------------------------------------------------
// SearchResultKind
// ---------------------------------------------------------------------------

/// Discriminant for a [`SearchResult`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchResultKind {
    /// A matching desktop application.
    Application { app_id: String },
    /// An inline calculator evaluation.
    Calculator { expression: String, result: f64 },
    /// A raw shell command prefixed with `>`.
    CustomCommand { command: String },
    /// A web-search fallback.
    WebSearch { query: String },
    /// A plugin-provided action.
    Plugin { plugin_id: String, action: String },
}

// ---------------------------------------------------------------------------
// SearchResult
// ---------------------------------------------------------------------------

/// A single entry in the launcher's result list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Display title.
    pub title: String,
    /// Optional secondary text.
    pub description: Option<String>,
    /// Optional icon name or path.
    pub icon: Option<String>,
    /// What this result represents.
    pub kind: SearchResultKind,
    /// Relevance score in `[0.0, ...)` — higher is better.
    pub relevance: f64,
}

// ---------------------------------------------------------------------------
// LauncherSection
// ---------------------------------------------------------------------------

/// Top-level section inside the launcher overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LauncherSection {
    Favorites,
    Recent,
    AllApps,
    SearchResults,
}

// ---------------------------------------------------------------------------
// ContextAction
// ---------------------------------------------------------------------------

/// Action that can be performed from a right-click context menu on a
/// launcher entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextAction {
    Launch,
    PinToFavorites,
    UnpinFromFavorites,
    PinToDock,
    OpenFileLocation,
    RunInTerminal,
    AppInfo,
}

impl fmt::Display for ContextAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch => write!(f, "Launch"),
            Self::PinToFavorites => write!(f, "Pin to Favorites"),
            Self::UnpinFromFavorites => write!(f, "Unpin from Favorites"),
            Self::PinToDock => write!(f, "Pin to Dock"),
            Self::OpenFileLocation => write!(f, "Open File Location"),
            Self::RunInTerminal => write!(f, "Run in Terminal"),
            Self::AppInfo => write!(f, "App Info"),
        }
    }
}

// ---------------------------------------------------------------------------
// Launcher
// ---------------------------------------------------------------------------

/// Runtime state of the application launcher.
///
/// Owns the registered application list, favourites, search state, and
/// presentation settings.  Call [`open`](Launcher::open) /
/// [`close`](Launcher::close) to toggle visibility and
/// [`set_query`](Launcher::set_query) to drive the search.
pub struct Launcher {
    config: LauncherConfig,
    apps: Vec<LauncherApp>,
    favorites: Vec<String>,
    query: String,
    results: Vec<SearchResult>,
    selected_index: usize,
    active_section: LauncherSection,
    current_view: LauncherView,
    visible: bool,
}

impl Launcher {
    // -- construction -------------------------------------------------------

    /// Create a new launcher with the given configuration.
    #[must_use]
    pub fn new(config: LauncherConfig) -> Self {
        let view = config.default_view;
        Self {
            config,
            apps: Vec::new(),
            favorites: Vec::new(),
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            active_section: LauncherSection::Favorites,
            current_view: view,
            visible: false,
        }
    }

    // -- app management -----------------------------------------------------

    /// Register a new application with the launcher.
    pub fn add_app(&mut self, app: LauncherApp) {
        self.apps.push(app);
    }

    /// Remove an application by its id.
    ///
    /// Returns `true` if the application was found and removed.
    pub fn remove_app(&mut self, app_id: &str) -> bool {
        let before = self.apps.len();
        self.apps.retain(|a| a.app_id != app_id);
        self.favorites.retain(|id| id != app_id);
        self.apps.len() < before
    }

    /// Look up an application by its id.
    #[must_use]
    pub fn app(&self, app_id: &str) -> Option<&LauncherApp> {
        self.apps.iter().find(|a| a.app_id == app_id)
    }

    /// The total number of registered applications.
    #[must_use]
    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    // -- search -------------------------------------------------------------

    /// Update the search query and recompute results.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.search();
    }

    /// The current search query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Recompute [`results`](Launcher::results) based on the current query
    /// and configuration.
    pub fn search(&mut self) {
        self.results.clear();
        self.selected_index = 0;

        let query = self.query.trim();

        // ----- empty query: show favourites / recent -----------------------
        if query.is_empty() {
            self.active_section = LauncherSection::Favorites;
            return;
        }

        self.active_section = LauncherSection::SearchResults;

        // ----- custom command (">…") ---------------------------------------
        if let Some(cmd) = query.strip_prefix('>') {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                self.results.push(SearchResult {
                    title: format!("Run: {cmd}"),
                    description: Some("Execute custom command".into()),
                    icon: Some("terminal".into()),
                    kind: SearchResultKind::CustomCommand {
                        command: cmd.to_owned(),
                    },
                    relevance: 1.0,
                });
            }
            return;
        }

        // ----- inline calculator -------------------------------------------
        if self.config.calculator_enabled && Self::looks_like_math(query) {
            match calculator::evaluate(query) {
                CalcResult::Number(n) => {
                    self.results.push(SearchResult {
                        title: format!("{query} = {n}"),
                        description: Some("Calculator".into()),
                        icon: Some("calculator".into()),
                        kind: SearchResultKind::Calculator {
                            expression: query.to_owned(),
                            result: n,
                        },
                        relevance: 2.0, // above app matches
                    });
                }
                CalcResult::Conversion {
                    value,
                    from_unit,
                    to_unit,
                    result,
                } => {
                    self.results.push(SearchResult {
                        title: format!("{value} {from_unit} = {result} {to_unit}"),
                        description: Some("Unit conversion".into()),
                        icon: Some("calculator".into()),
                        kind: SearchResultKind::Calculator {
                            expression: query.to_owned(),
                            result,
                        },
                        relevance: 2.0,
                    });
                }
                CalcResult::Error(_) => { /* not a valid expression — skip */ }
            }
        }

        // ----- application matching ----------------------------------------
        let query_lower = query.to_lowercase();

        for app in &self.apps {
            if app.no_display {
                continue;
            }

            let mut relevance = 0.0_f64;

            // Name matching.
            let name_lower = app.name.to_lowercase();
            let name_score = Self::fuzzy_score(&query_lower, &name_lower);
            if name_score > 0.0 {
                // Boost exact, prefix, or substring matches.
                relevance = relevance.max(name_score);
            }

            // Keyword matching.
            for kw in &app.keywords {
                let kw_lower = kw.to_lowercase();
                let kw_score = Self::fuzzy_score(&query_lower, &kw_lower);
                if kw_score > 0.0 {
                    // Keywords score slightly lower than the name.
                    relevance = relevance.max(kw_score * 0.6);
                }
            }

            // Description matching.
            if let Some(ref desc) = app.description {
                let desc_lower = desc.to_lowercase();
                if desc_lower.contains(&query_lower) {
                    relevance = relevance.max(0.5);
                }
            }

            if relevance > 0.0 {
                // Weight by launch frequency (diminishing returns).
                let freq_boost = 1.0 + 0.1 * (app.launch_count.min(10) as f64);
                relevance *= freq_boost;

                self.results.push(SearchResult {
                    title: app.name.clone(),
                    description: app.description.clone(),
                    icon: app.icon.clone(),
                    kind: SearchResultKind::Application {
                        app_id: app.app_id.clone(),
                    },
                    relevance,
                });
            }
        }

        // ----- sort by relevance descending --------------------------------
        self.results
            .sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));

        // ----- web search fallback -----------------------------------------
        if self.results.is_empty() && self.config.search_web {
            self.results.push(SearchResult {
                title: format!("Search the web for \"{query}\""),
                description: Some("Open in default browser".into()),
                icon: Some("web-browser".into()),
                kind: SearchResultKind::WebSearch {
                    query: query.to_owned(),
                },
                relevance: 0.1,
            });
        }
    }

    /// Heuristic: returns `true` if the query looks like a mathematical
    /// expression (starts with a digit or contains an arithmetic operator).
    fn looks_like_math(query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return false;
        }
        let first = trimmed.as_bytes()[0];
        if first.is_ascii_digit() {
            return true;
        }
        trimmed.contains('+')
            || trimmed.contains('-')
            || trimmed.contains('*')
            || trimmed.contains('/')
            || trimmed.contains('^')
            || trimmed.contains('%')
    }

    /// Case-insensitive fuzzy scoring of `query` against `target`.
    ///
    /// Returns:
    /// - `1.0` — exact match
    /// - `0.9` — `target` starts with `query`
    /// - `0.7` — `target` contains `query` as a substring
    /// - `0.3` — `query` characters appear as a subsequence in `target`
    /// - `0.0` — no match
    #[must_use]
    pub fn fuzzy_score(query: &str, target: &str) -> f64 {
        if query.is_empty() || target.is_empty() {
            return 0.0;
        }

        let q = query.to_lowercase();
        let t = target.to_lowercase();

        // Exact match.
        if q == t {
            return 1.0;
        }

        // Prefix match.
        if t.starts_with(&q) {
            return 0.9;
        }

        // Substring match.
        if t.contains(&q) {
            return 0.7;
        }

        // Subsequence match.
        let mut t_iter = t.chars();
        let mut matched = 0_usize;
        let q_chars: Vec<char> = q.chars().collect();
        for qc in &q_chars {
            let mut found = false;
            for tc in t_iter.by_ref() {
                if tc == *qc {
                    found = true;
                    matched += 1;
                    break;
                }
            }
            if !found {
                break;
            }
        }
        if matched == q_chars.len() {
            return 0.3;
        }

        0.0
    }

    // -- result access ------------------------------------------------------

    /// The current search results.
    #[must_use]
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }

    /// Number of results in the current result set.
    #[must_use]
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    // -- selection ----------------------------------------------------------

    /// Index of the currently highlighted result.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move the selection to the next result, wrapping around.
    pub fn select_next(&mut self) {
        if self.results.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.results.len();
    }

    /// Move the selection to the previous result, wrapping around.
    pub fn select_prev(&mut self) {
        if self.results.is_empty() {
            self.selected_index = 0;
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.results.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Jump the selection to a specific index, clamped to the result set.
    pub fn select_index(&mut self, index: usize) {
        if self.results.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = index.min(self.results.len() - 1);
        }
    }

    /// Returns the [`SearchResultKind`] of the currently selected result,
    /// or `None` if the result list is empty.
    #[must_use]
    pub fn activate_selected(&self) -> Option<&SearchResultKind> {
        self.results.get(self.selected_index).map(|r| &r.kind)
    }

    // -- favourites ---------------------------------------------------------

    /// Pin an application to the favourites strip.
    ///
    /// Returns `false` if the maximum number of favourites has been reached
    /// or the app is already pinned.
    pub fn pin_favorite(&mut self, app_id: &str) -> bool {
        if self.favorites.len() >= self.config.max_favorites {
            return false;
        }
        if self.favorites.iter().any(|id| id == app_id) {
            return false;
        }
        self.favorites.push(app_id.to_owned());
        true
    }

    /// Remove an application from the favourites strip.
    ///
    /// Returns `true` if the application was found and removed.
    pub fn unpin_favorite(&mut self, app_id: &str) -> bool {
        let before = self.favorites.len();
        self.favorites.retain(|id| id != app_id);
        self.favorites.len() < before
    }

    /// Whether the given application is currently pinned as a favourite.
    #[must_use]
    pub fn is_favorite(&self, app_id: &str) -> bool {
        self.favorites.iter().any(|id| id == app_id)
    }

    /// The ordered list of favourite application ids.
    #[must_use]
    pub fn favorites(&self) -> &[String] {
        &self.favorites
    }

    // -- context actions ----------------------------------------------------

    /// Build the list of context actions available for the given application.
    #[must_use]
    pub fn context_actions(&self, app_id: &str) -> Vec<ContextAction> {
        let mut actions = vec![ContextAction::Launch];

        if self.is_favorite(app_id) {
            actions.push(ContextAction::UnpinFromFavorites);
        } else {
            actions.push(ContextAction::PinToFavorites);
        }

        actions.push(ContextAction::PinToDock);

        if let Some(app) = self.app(app_id) {
            if app.exec.is_some() {
                actions.push(ContextAction::OpenFileLocation);
                actions.push(ContextAction::RunInTerminal);
            }
        }

        actions.push(ContextAction::AppInfo);
        actions
    }

    // -- view ---------------------------------------------------------------

    /// Toggle between [`LauncherView::List`] and [`LauncherView::Grid`].
    pub fn toggle_view(&mut self) {
        self.current_view = match self.current_view {
            LauncherView::List => LauncherView::Grid,
            LauncherView::Grid => LauncherView::List,
        };
    }

    /// Set the view mode explicitly.
    pub fn set_view(&mut self, view: LauncherView) {
        self.current_view = view;
    }

    /// The current view mode.
    #[must_use]
    pub fn current_view(&self) -> LauncherView {
        self.current_view
    }

    // -- visibility ---------------------------------------------------------

    /// Open the launcher overlay.
    ///
    /// Resets the query, clears results, and sets the section back to
    /// [`LauncherSection::Favorites`].
    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
        self.active_section = LauncherSection::Favorites;
    }

    /// Close the launcher overlay.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Whether the launcher overlay is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggle the launcher open/closed.
    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    // -- category filtering -------------------------------------------------

    /// Return all non-hidden apps that belong to the given category.
    #[must_use]
    pub fn apps_by_category(&self, category: AppCategory) -> Vec<&LauncherApp> {
        self.apps
            .iter()
            .filter(|a| !a.no_display && a.categories.contains(&category))
            .collect()
    }

    // -- launch tracking ----------------------------------------------------

    /// Record that an application was launched.
    ///
    /// Increments the launch counter and sets the last-launched timestamp.
    pub fn record_launch(&mut self, app_id: &str, timestamp_us: u64) {
        if let Some(app) = self.apps.iter_mut().find(|a| a.app_id == app_id) {
            app.launch_count = app.launch_count.saturating_add(1);
            app.last_launched_us = timestamp_us;
        }
    }

    /// Return the top `n` most frequently launched applications.
    #[must_use]
    pub fn most_frequent(&self, n: usize) -> Vec<&LauncherApp> {
        let mut sorted: Vec<&LauncherApp> = self.apps.iter().filter(|a| !a.no_display).collect();
        sorted.sort_by(|a, b| b.launch_count.cmp(&a.launch_count));
        sorted.truncate(n);
        sorted
    }

    /// Return the top `n` most recently launched applications.
    ///
    /// Only includes applications that have been launched at least once
    /// (`last_launched_us > 0`).
    #[must_use]
    pub fn most_recent(&self, n: usize) -> Vec<&LauncherApp> {
        let mut sorted: Vec<&LauncherApp> = self
            .apps
            .iter()
            .filter(|a| !a.no_display && a.last_launched_us > 0)
            .collect();
        sorted.sort_by(|a, b| b.last_launched_us.cmp(&a.last_launched_us));
        sorted.truncate(n);
        sorted
    }

    // -- section & config ---------------------------------------------------

    /// The currently active section.
    #[must_use]
    pub fn active_section(&self) -> LauncherSection {
        self.active_section
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &LauncherConfig {
        &self.config
    }

    /// Build the scene graph for the launcher overlay.
    pub fn build_scene(&self, screen: Rect, theme: &crate::theme::ShellTheme) -> SceneNode {
        use crate::scene_builder::*;
        use liquide_compositor::geometry::Rect as GRect;
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

        if !self.visible {
            return SceneNode::new(
                NODE_LAUNCHER,
                SceneNodeKind::Overlay,
                NodeProperties::new(GRect::ZERO).with_visible(false),
            );
        }

        let mut launcher_root = SceneNode::new(
            NODE_LAUNCHER,
            SceneNodeKind::Tint {
                color: theme.launcher_overlay,
            },
            NodeProperties::new(screen).with_z_order(980),
        );

        let panel_w = screen.width * 0.6;
        let panel_h = screen.height * 0.7;
        let panel_x = screen.x + (screen.width - panel_w) / 2.0;
        let panel_y = screen.y + (screen.height - panel_h) / 2.0;
        let panel_bounds = GRect::new(panel_x, panel_y, panel_w, panel_h);

        launcher_root.add_child(SceneNode::new(
            NODE_LAUNCHER + 1,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 25,
                tint_color: theme.launcher_glass_tint,
                inner_glow: true,
                parallax: false,
            }),
            NodeProperties::new(panel_bounds).with_z_order(981),
        ));

        let search_bounds = GRect::new(panel_x + 20.0, panel_y + 15.0, panel_w - 40.0, 36.0);
        launcher_root.add_child(solid_rect(
            NODE_LAUNCHER + 2,
            theme.launcher_search_bar,
            search_bounds,
            982,
        ));

        let item_start_y = panel_y + 65.0;
        let item_height = 40.0_f32;
        let item_gap = 4.0_f32;
        let max_visible = ((panel_h - 80.0) / (item_height + item_gap)) as usize;
        let items_to_show = if self.results.is_empty() {
            self.favorites.len().min(max_visible)
        } else {
            self.results.len().min(max_visible)
        };

        for i in 0..items_to_show {
            let iy = item_start_y + i as f32 * (item_height + item_gap);
            let item_bounds = GRect::new(panel_x + 20.0, iy, panel_w - 40.0, item_height);
            let color = if i == self.selected_index {
                theme.launcher_item_selected
            } else {
                theme.launcher_item_normal
            };
            launcher_root.add_child(solid_rect(NODE_LAUNCHER + 10 + i as u64, color, item_bounds, 983));
        }

        launcher_root
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Launcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Launcher({} apps, {})",
            self.apps.len(),
            if self.visible { "visible" } else { "hidden" },
        )
    }
}

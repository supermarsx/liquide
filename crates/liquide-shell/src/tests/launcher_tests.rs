use crate::launcher::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_launcher() -> Launcher {
    Launcher::new(LauncherConfig::default())
}

fn make_app(id: &str, name: &str) -> LauncherApp {
    LauncherApp {
        app_id: id.into(),
        name: name.into(),
        description: Some(format!("{name} application")),
        icon: Some(format!("{id}.png")),
        exec: Some(format!("/usr/bin/{id}")),
        categories: vec![AppCategory::Other],
        keywords: vec![],
        terminal: false,
        no_display: false,
        launch_count: 0,
        last_launched_us: 0,
    }
}

/// Like [`make_app`] but with NO description/keywords, so matching is decided
/// purely by the title (used by the ranking teeth where a stray description
/// match would muddy the tier under test).
fn make_named(id: &str, name: &str) -> LauncherApp {
    LauncherApp {
        description: None,
        ..make_app(id, name)
    }
}

// ========== LauncherConfig defaults ==========

#[test]
fn launcher_config_default_values() {
    let cfg = LauncherConfig::default();
    assert_eq!(cfg.default_view, LauncherView::List);
    assert!(cfg.show_favorites);
    assert!(cfg.show_recent);
    assert_eq!(cfg.recent_count, 10);
    assert!(!cfg.search_files);
    assert!(!cfg.search_web);
    assert!(cfg.calculator_enabled);
    assert!(!cfg.workspace_switcher);
    assert_eq!(cfg.max_favorites, 9);
    assert!(cfg.animation_enabled);
}

// ========== App management ==========

#[test]
fn add_app_increases_count() {
    let mut launcher = default_launcher();
    assert_eq!(launcher.app_count(), 0);
    launcher.add_app(make_app("term", "Terminal"));
    assert_eq!(launcher.app_count(), 1);
}

#[test]
fn app_lookup_by_id() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("firefox", "Firefox"));
    let app = launcher.app("firefox");
    assert!(app.is_some());
    assert_eq!(app.unwrap().name, "Firefox");
}

#[test]
fn app_lookup_missing_returns_none() {
    let launcher = default_launcher();
    assert!(launcher.app("nonexistent").is_none());
}

#[test]
fn remove_app_returns_true_when_found() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("vim", "Vim"));
    assert!(launcher.remove_app("vim"));
    assert_eq!(launcher.app_count(), 0);
}

#[test]
fn remove_app_returns_false_when_not_found() {
    let mut launcher = default_launcher();
    assert!(!launcher.remove_app("nonexistent"));
}

#[test]
fn remove_app_also_removes_from_favorites() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("code", "VS Code"));
    launcher.pin_favorite("code");
    assert!(launcher.is_favorite("code"));
    launcher.remove_app("code");
    assert!(!launcher.is_favorite("code"));
}

// ========== Fuzzy search scoring ==========

// The scorer is tiered: a better match KIND always outranks a worse one, and
// the bands stay inside [0.3, 1.0] so the calculator's 2.0 relevance still wins.
// Bands: exact = 1.0, prefix ∈ [0.85, 0.90), substring ∈ [0.65, 0.75),
// subsequence ∈ [0.30, 0.50). Within a tier a fine fuzzy nudge orders ties.

#[test]
fn fuzzy_score_exact_match() {
    // Exact is a single fixed value at the top of the scale.
    let score = Launcher::fuzzy_score("terminal", "terminal");
    assert!((score - 1.0).abs() < 0.001, "exact must be 1.0, got {score}");
}

#[test]
fn fuzzy_score_prefix_match_in_band() {
    let score = Launcher::fuzzy_score("fire", "firefox");
    assert!(
        (0.85..0.90).contains(&score),
        "prefix must land in [0.85, 0.90), got {score}"
    );
}

#[test]
fn fuzzy_score_substring_match_in_band() {
    let score = Launcher::fuzzy_score("fox", "firefox");
    assert!(
        (0.65..0.75).contains(&score),
        "substring must land in [0.65, 0.75), got {score}"
    );
}

#[test]
fn fuzzy_score_subsequence_match_in_band() {
    let score = Launcher::fuzzy_score("ffx", "firefox");
    assert!(
        (0.30..0.50).contains(&score),
        "subsequence must land in [0.30, 0.50), got {score}"
    );
}

#[test]
fn fuzzy_score_tiers_are_strictly_ordered() {
    // The core ranking contract: exact > prefix > substring > subsequence, with
    // NO band overlap, so a better match kind always sorts ahead of a worse one.
    let exact = Launcher::fuzzy_score("firefox", "firefox");
    let prefix = Launcher::fuzzy_score("fire", "firefox");
    let substring = Launcher::fuzzy_score("fox", "firefox");
    let subseq = Launcher::fuzzy_score("ffx", "firefox");
    assert!(
        exact > prefix && prefix > substring && substring > subseq,
        "tiers must be strictly ordered: exact {exact} > prefix {prefix} > substring {substring} > subseq {subseq}"
    );
}

#[test]
fn fuzzy_score_no_match() {
    let score = Launcher::fuzzy_score("zzz", "firefox");
    assert!((score - 0.0).abs() < 0.001);
}

#[test]
fn fuzzy_score_empty_query() {
    assert!((Launcher::fuzzy_score("", "firefox") - 0.0).abs() < 0.001);
}

#[test]
fn fuzzy_score_empty_target() {
    assert!((Launcher::fuzzy_score("fire", "") - 0.0).abs() < 0.001);
}

#[test]
fn fuzzy_score_case_insensitive() {
    // Mixed-case query against mixed-case target still resolves as a prefix.
    let score = Launcher::fuzzy_score("FIRE", "Firefox");
    assert!(
        (0.85..0.90).contains(&score),
        "case-insensitive prefix must land in the prefix band, got {score}"
    );
    // And the score is identical regardless of query casing.
    let lower = Launcher::fuzzy_score("fire", "Firefox");
    assert!(
        (score - lower).abs() < 0.001,
        "score must not depend on query casing: {score} vs {lower}"
    );
}

#[test]
fn fuzzy_score_prefix_beats_midword_substring() {
    // A query that is a PREFIX of one title and only a MID-WORD substring of
    // another must rank the prefix higher (Spotlight-style "best match first").
    let prefix = Launcher::fuzzy_score("set", "Settings");
    let midword = Launcher::fuzzy_score("set", "Reset");
    assert!(
        prefix > midword,
        "prefix 'set'→Settings ({prefix}) must beat mid-word 'set'→Reset ({midword})"
    );
}

#[test]
fn fuzzy_score_boundary_subsequence_beats_scattered() {
    // Within the SAME (subsequence) tier the word-boundary-aligned match wins
    // via the fuzzy nudge: "fb" hits the start of both words in "Foo Bar"
    // (F…B), but is buried mid-word in "fabric" (Fa-B-ric). Both are
    // non-contiguous subsequences, so both sit in [0.30, 0.50) and only the
    // nudge separates them.
    let aligned = Launcher::fuzzy_score("fb", "Foo Bar");
    let buried = Launcher::fuzzy_score("fb", "fabric");
    assert!(
        (0.30..0.50).contains(&aligned) && (0.30..0.50).contains(&buried),
        "both must be subsequence-tier: aligned {aligned}, buried {buried}"
    );
    assert!(
        aligned > buried,
        "boundary-aligned 'fb'→'Foo Bar' ({aligned}) must beat buried 'fb'→'fabric' ({buried})"
    );
}

// ========== Search — calculator integration ==========

#[test]
fn search_math_expression_produces_calculator_result() {
    let mut launcher = default_launcher();
    launcher.set_query("2+3");
    assert!(launcher.result_count() > 0);
    let r = &launcher.results()[0];
    assert!(r.title.contains("= 5"));
    match &r.kind {
        SearchResultKind::Calculator { result, .. } => {
            assert!((result - 5.0).abs() < 0.01);
        }
        _ => panic!("Expected Calculator result kind"),
    }
}

#[test]
fn search_calculator_disabled_skips_math() {
    let cfg = LauncherConfig {
        calculator_enabled: false,
        ..LauncherConfig::default()
    };
    let mut launcher = Launcher::new(cfg);
    launcher.set_query("2+3");
    // No calculator result and no apps, so results should be empty
    assert_eq!(launcher.result_count(), 0);
}

// ========== Search — custom commands ==========

#[test]
fn search_custom_command_with_prefix() {
    let mut launcher = default_launcher();
    launcher.set_query(">ls -la");
    assert_eq!(launcher.result_count(), 1);
    match &launcher.results()[0].kind {
        SearchResultKind::CustomCommand { command } => {
            assert_eq!(command, "ls -la");
        }
        _ => panic!("Expected CustomCommand result kind"),
    }
}

#[test]
fn search_custom_command_empty_after_prefix() {
    let mut launcher = default_launcher();
    launcher.set_query(">  ");
    // Trim makes it empty, so no custom command result
    assert_eq!(launcher.result_count(), 0);
}

// ========== Search — application matching ==========

#[test]
fn search_matches_app_by_name() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("firefox", "Firefox Browser"));
    launcher.set_query("fire");
    assert!(launcher.result_count() > 0);
    match &launcher.results()[0].kind {
        SearchResultKind::Application { app_id } => {
            assert_eq!(app_id, "firefox");
        }
        _ => panic!("Expected Application result kind"),
    }
}

#[test]
fn search_no_display_app_excluded() {
    let mut launcher = default_launcher();
    let mut app = make_app("hidden", "Hidden App");
    app.no_display = true;
    launcher.add_app(app);
    launcher.set_query("hidden");
    assert_eq!(launcher.result_count(), 0);
}

#[test]
fn search_matches_keywords() {
    let mut launcher = default_launcher();
    let mut app = make_app("code", "Visual Studio Code");
    app.keywords = vec!["editor".into(), "ide".into()];
    launcher.add_app(app);
    launcher.set_query("ide");
    assert!(launcher.result_count() > 0);
}

#[test]
fn search_matches_description() {
    let mut launcher = default_launcher();
    let mut app = make_app("gimp", "GIMP");
    app.description = Some("Image manipulation program".into());
    launcher.add_app(app);
    launcher.set_query("image manipulation");
    assert!(launcher.result_count() > 0);
}

#[test]
fn search_empty_query_shows_default_app_listing() {
    // t59-shell: an empty query now seeds the default listing (favorites / all
    // registered apps) so the launcher grid is populated on open, instead of
    // rendering an empty card. The section stays Favorites.
    let mut launcher = default_launcher();
    launcher.add_app(make_app("foo", "Foo"));
    launcher.set_query("");
    assert_eq!(
        launcher.result_count(),
        1,
        "empty query must list the registered app(s), not render an empty grid"
    );
    assert!(matches!(
        launcher.results()[0].kind,
        SearchResultKind::Application { .. }
    ));
    assert_eq!(launcher.active_section(), LauncherSection::Favorites);
}

#[test]
fn search_results_sorted_by_relevance() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("xfox", "Xfox"));
    launcher.add_app(make_app("firefox", "Firefox"));
    launcher.set_query("firefox");
    // Exact match (Firefox) should rank higher
    assert!(launcher.result_count() >= 1);
    match &launcher.results()[0].kind {
        SearchResultKind::Application { app_id } => {
            assert_eq!(app_id, "firefox");
        }
        _ => panic!("Expected Application result kind"),
    }
}

// ========== Search — filtering + ranking TEETH (t195) ==========
//
// These mirror the real default launcher catalog (Files / Terminal / Browser /
// Settings / Calculator) and assert the ACTUAL filtered + ranked result set,
// not merely "didn't panic". They are the teeth behind the t195 search fixes:
// live filtering, case-insensitive fuzzy matching, sensible best-first ranking,
// empty-query "show all", and a real no-match empty set.

/// Build a launcher seeded with the production default-app catalog.
fn catalog_launcher() -> Launcher {
    let mut launcher = default_launcher();
    for (id, name) in [
        ("com.liquide.files", "Files"),
        ("com.liquide.terminal", "Terminal"),
        ("com.liquide.browser", "Browser"),
        ("com.liquide.settings", "Settings"),
        ("com.liquide.calculator", "Calculator"),
    ] {
        // No description/keywords — match purely on the name, like the defaults
        // (description is empty so it cannot accidentally widen the match set).
        launcher.add_app(LauncherApp {
            app_id: id.into(),
            name: name.into(),
            description: None,
            icon: Some("icon".into()),
            exec: None,
            categories: vec![],
            keywords: vec![],
            terminal: false,
            no_display: false,
            launch_count: 0,
            last_launched_us: 0,
        });
    }
    launcher
}

/// The set of matched app_ids in result order.
fn result_ids(launcher: &Launcher) -> Vec<String> {
    launcher
        .results()
        .iter()
        .filter_map(|r| match &r.kind {
            SearchResultKind::Application { app_id } => Some(app_id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn search_filters_to_matching_apps_not_all_not_none() {
    let mut launcher = catalog_launcher();
    // "te" matches Terminal (prefix) only among names; Settings has no "te"
    // subsequence in its NAME ("Settings": no 't' followed by 'e').
    launcher.set_query("te");
    let ids = result_ids(&launcher);
    assert_eq!(
        ids,
        vec!["com.liquide.terminal".to_string()],
        "typing 'te' must filter to exactly Terminal, got {ids:?}"
    );
    // Sanity: filtering actually narrowed from the full catalog of 5.
    assert!(ids.len() < 5, "filtering must narrow the catalog");
}

#[test]
fn search_prefix_is_selected_and_ranked_first() {
    let mut launcher = catalog_launcher();
    launcher.set_query("ca");
    let ids = result_ids(&launcher);
    assert_eq!(
        ids,
        vec!["com.liquide.calculator".to_string()],
        "typing 'ca' must filter to exactly Calculator, got {ids:?}"
    );
    // The first (and selected) result is the prefix match.
    assert_eq!(launcher.selected_index(), 0);
}

#[test]
fn search_ranks_prefix_match_before_weaker_match() {
    // "se" is a PREFIX of Settings but only a non-contiguous SUBSEQUENCE of
    // "Browser" (browSE-r → b…r…o…wsE? actually B-r-o-w-s-e-r contains "se"
    // contiguously). To get an unambiguous prefix-vs-subsequence ranking we use
    // two synthetic apps so the tiers are clean.
    let mut launcher = default_launcher();
    launcher.add_app(make_named("a.sysenv", "System Env")); // "se" subsequence: Sy…s..E
    launcher.add_app(make_named("a.setup", "Setup")); // "se" prefix
    launcher.set_query("se");
    let ids = result_ids(&launcher);
    assert!(
        ids.len() == 2,
        "both apps should match 'se' as a subsequence, got {ids:?}"
    );
    assert_eq!(
        ids[0], "a.setup",
        "the prefix match (Setup) must rank ahead of the subsequence match, got {ids:?}"
    );
}

#[test]
fn search_is_case_insensitive() {
    let mut launcher = catalog_launcher();
    launcher.set_query("FILES");
    let ids = result_ids(&launcher);
    assert_eq!(
        ids,
        vec!["com.liquide.files".to_string()],
        "uppercase query must still match the lowercase title, got {ids:?}"
    );
}

#[test]
fn search_empty_query_shows_all_apps() {
    let mut launcher = catalog_launcher();
    launcher.set_query("fi");
    assert_eq!(result_ids(&launcher).len(), 1, "filtered to one first");
    // Clearing the query (e.g. backspace to empty) restores the full catalog.
    launcher.set_query("");
    assert_eq!(
        result_ids(&launcher).len(),
        5,
        "empty query must show ALL apps, not the filtered subset"
    );
}

#[test]
fn search_no_match_yields_empty_set() {
    // With web fallback off (the default), a query matching nothing must produce
    // an EMPTY result set so the UI shows its no-results empty-state — not a
    // stale or full list.
    let mut launcher = catalog_launcher();
    launcher.set_query("qqzzx");
    assert_eq!(
        launcher.result_count(),
        0,
        "a no-match query must yield zero results (drives the empty-state)"
    );
}

// ========== Search — web search fallback ==========

#[test]
fn search_web_fallback_when_no_matches() {
    let cfg = LauncherConfig {
        search_web: true,
        ..LauncherConfig::default()
    };
    let mut launcher = Launcher::new(cfg);
    launcher.set_query("xyznonexistent");
    assert!(launcher.result_count() > 0);
    match &launcher.results()[0].kind {
        SearchResultKind::WebSearch { query } => {
            assert_eq!(query, "xyznonexistent");
        }
        _ => panic!("Expected WebSearch result kind"),
    }
}

// ========== Selection navigation ==========

#[test]
fn select_next_wraps_around() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.add_app(make_app("b", "Beta"));
    launcher.set_query("a"); // matches both Alpha
    let count = launcher.result_count();
    assert!(count > 0);
    assert_eq!(launcher.selected_index(), 0);
    for _ in 0..count {
        launcher.select_next();
    }
    assert_eq!(launcher.selected_index(), 0); // wrapped
}

#[test]
fn select_prev_wraps_around() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.set_query("alpha");
    assert_eq!(launcher.selected_index(), 0);
    launcher.select_prev();
    assert_eq!(launcher.selected_index(), launcher.result_count() - 1);
}

#[test]
fn select_next_with_empty_results_stays_zero() {
    let mut launcher = default_launcher();
    launcher.select_next();
    assert_eq!(launcher.selected_index(), 0);
}

#[test]
fn select_prev_with_empty_results_stays_zero() {
    let mut launcher = default_launcher();
    launcher.select_prev();
    assert_eq!(launcher.selected_index(), 0);
}

#[test]
fn select_index_clamps_to_bounds() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.set_query("alpha");
    launcher.select_index(999);
    assert_eq!(launcher.selected_index(), launcher.result_count() - 1);
}

#[test]
fn select_index_empty_results_stays_zero() {
    let mut launcher = default_launcher();
    launcher.select_index(5);
    assert_eq!(launcher.selected_index(), 0);
}

#[test]
fn activate_selected_returns_kind() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    launcher.set_query("term");
    let kind = launcher.activate_selected();
    assert!(kind.is_some());
}

#[test]
fn activate_selected_empty_returns_none() {
    let launcher = default_launcher();
    assert!(launcher.activate_selected().is_none());
}

// ========== Favorites ==========

#[test]
fn pin_favorite_adds_to_list() {
    let mut launcher = default_launcher();
    assert!(launcher.pin_favorite("app1"));
    assert!(launcher.is_favorite("app1"));
    assert_eq!(launcher.favorites().len(), 1);
}

#[test]
fn pin_favorite_duplicate_returns_false() {
    let mut launcher = default_launcher();
    launcher.pin_favorite("app1");
    assert!(!launcher.pin_favorite("app1"));
}

#[test]
fn pin_favorite_max_limit() {
    let mut launcher = default_launcher();
    for i in 0..9 {
        assert!(launcher.pin_favorite(&format!("app{i}")));
    }
    // 10th should fail (max_favorites = 9)
    assert!(!launcher.pin_favorite("app9"));
    assert_eq!(launcher.favorites().len(), 9);
}

#[test]
fn unpin_favorite_removes_from_list() {
    let mut launcher = default_launcher();
    launcher.pin_favorite("app1");
    assert!(launcher.unpin_favorite("app1"));
    assert!(!launcher.is_favorite("app1"));
}

#[test]
fn unpin_favorite_not_found_returns_false() {
    let mut launcher = default_launcher();
    assert!(!launcher.unpin_favorite("nonexistent"));
}

// ========== Context actions ==========

#[test]
fn context_actions_for_non_favorite_app() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    let actions = launcher.context_actions("term");
    assert!(actions.contains(&ContextAction::Launch));
    assert!(actions.contains(&ContextAction::PinToFavorites));
    assert!(!actions.contains(&ContextAction::UnpinFromFavorites));
    assert!(actions.contains(&ContextAction::PinToDock));
    assert!(actions.contains(&ContextAction::OpenFileLocation));
    assert!(actions.contains(&ContextAction::RunInTerminal));
    assert!(actions.contains(&ContextAction::AppInfo));
}

#[test]
fn context_actions_for_favorite_app() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    launcher.pin_favorite("term");
    let actions = launcher.context_actions("term");
    assert!(actions.contains(&ContextAction::UnpinFromFavorites));
    assert!(!actions.contains(&ContextAction::PinToFavorites));
}

#[test]
fn context_actions_for_app_without_exec() {
    let mut launcher = default_launcher();
    let mut app = make_app("special", "Special");
    app.exec = None;
    launcher.add_app(app);
    let actions = launcher.context_actions("special");
    assert!(!actions.contains(&ContextAction::OpenFileLocation));
    assert!(!actions.contains(&ContextAction::RunInTerminal));
}

// ========== View toggling ==========

#[test]
fn toggle_view_switches_between_list_and_grid() {
    let mut launcher = default_launcher();
    assert_eq!(launcher.current_view(), LauncherView::List);
    launcher.toggle_view();
    assert_eq!(launcher.current_view(), LauncherView::Grid);
    launcher.toggle_view();
    assert_eq!(launcher.current_view(), LauncherView::List);
}

#[test]
fn set_view_explicit() {
    let mut launcher = default_launcher();
    launcher.set_view(LauncherView::Grid);
    assert_eq!(launcher.current_view(), LauncherView::Grid);
}

// ========== Visibility ==========

#[test]
fn launcher_initially_hidden() {
    let launcher = default_launcher();
    assert!(!launcher.is_visible());
}

#[test]
fn open_makes_visible_and_resets_state() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.set_query("alpha");
    launcher.open();
    assert!(launcher.is_visible());
    assert_eq!(launcher.query(), "");
    // t59-shell: open() clears the query AND seeds the default app listing, so
    // the grid shows the registered apps immediately (was previously empty).
    assert_eq!(
        launcher.result_count(),
        1,
        "open() must seed the default app listing so the grid is not blank"
    );
    assert_eq!(launcher.selected_index(), 0);
    assert_eq!(launcher.active_section(), LauncherSection::Favorites);
}

#[test]
fn close_hides_launcher() {
    let mut launcher = default_launcher();
    launcher.open();
    launcher.close();
    assert!(!launcher.is_visible());
}

#[test]
fn toggle_opens_and_closes() {
    let mut launcher = default_launcher();
    launcher.toggle();
    assert!(launcher.is_visible());
    launcher.toggle();
    assert!(!launcher.is_visible());
}

// ========== Category filtering ==========

#[test]
fn apps_by_category_returns_matching() {
    let mut launcher = default_launcher();
    let mut app = make_app("code", "VS Code");
    app.categories = vec![AppCategory::Development];
    launcher.add_app(app);
    launcher.add_app(make_app("calc", "Calculator")); // category Other

    let dev_apps = launcher.apps_by_category(AppCategory::Development);
    assert_eq!(dev_apps.len(), 1);
    assert_eq!(dev_apps[0].app_id, "code");
}

#[test]
fn apps_by_category_excludes_no_display() {
    let mut launcher = default_launcher();
    let mut app = make_app("hidden", "Hidden Dev");
    app.categories = vec![AppCategory::Development];
    app.no_display = true;
    launcher.add_app(app);
    assert_eq!(launcher.apps_by_category(AppCategory::Development).len(), 0);
}

// ========== Launch tracking ==========

#[test]
fn record_launch_increments_count() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    launcher.record_launch("term", 1000);
    let app = launcher.app("term").unwrap();
    assert_eq!(app.launch_count, 1);
    assert_eq!(app.last_launched_us, 1000);
}

#[test]
fn record_launch_multiple_times() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    launcher.record_launch("term", 100);
    launcher.record_launch("term", 200);
    launcher.record_launch("term", 300);
    assert_eq!(launcher.app("term").unwrap().launch_count, 3);
    assert_eq!(launcher.app("term").unwrap().last_launched_us, 300);
}

#[test]
fn most_frequent_returns_sorted_by_count() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.add_app(make_app("b", "Beta"));
    launcher.record_launch("a", 100);
    launcher.record_launch("b", 200);
    launcher.record_launch("b", 300);
    let freq = launcher.most_frequent(2);
    assert_eq!(freq[0].app_id, "b"); // launched twice
    assert_eq!(freq[1].app_id, "a"); // launched once
}

#[test]
fn most_recent_returns_sorted_by_timestamp() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.add_app(make_app("b", "Beta"));
    launcher.record_launch("a", 100);
    launcher.record_launch("b", 200);
    let recent = launcher.most_recent(2);
    assert_eq!(recent[0].app_id, "b"); // more recent
    assert_eq!(recent[1].app_id, "a");
}

#[test]
fn most_recent_excludes_never_launched() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.add_app(make_app("b", "Beta"));
    launcher.record_launch("a", 100);
    let recent = launcher.most_recent(10);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].app_id, "a");
}

#[test]
fn launch_frequency_boosts_search_relevance() {
    let mut launcher = default_launcher();
    let mut app_a = make_app("alpha", "Alpha App");
    app_a.launch_count = 10;
    let app_b = make_app("alphax", "Alpha X App");
    launcher.add_app(app_a);
    launcher.add_app(app_b);
    launcher.set_query("alpha");
    // Both match, but alpha has higher launch_count so should rank first
    assert!(launcher.result_count() >= 2);
    match &launcher.results()[0].kind {
        SearchResultKind::Application { app_id } => {
            assert_eq!(app_id, "alpha");
        }
        _ => panic!("Expected Application result kind"),
    }
}

// ========== Display impls ==========

#[test]
fn display_launcher_view() {
    assert_eq!(format!("{}", LauncherView::List), "List");
    assert_eq!(format!("{}", LauncherView::Grid), "Grid");
}

#[test]
fn display_app_category() {
    assert_eq!(format!("{}", AppCategory::Development), "Development");
    assert_eq!(format!("{}", AppCategory::Internet), "Internet");
    assert_eq!(format!("{}", AppCategory::Games), "Games");
    assert_eq!(format!("{}", AppCategory::Other), "Other");
}

#[test]
fn display_context_action() {
    assert_eq!(format!("{}", ContextAction::Launch), "Launch");
    assert_eq!(
        format!("{}", ContextAction::PinToFavorites),
        "Pin to Favorites"
    );
    assert_eq!(
        format!("{}", ContextAction::UnpinFromFavorites),
        "Unpin from Favorites"
    );
}

#[test]
fn display_launcher() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("a", "Alpha"));
    launcher.open();
    let s = format!("{launcher}");
    assert!(s.contains("Launcher"));
    assert!(s.contains("1 apps"));
    assert!(s.contains("visible"));
}

#[test]
fn display_launcher_hidden() {
    let launcher = default_launcher();
    let s = format!("{launcher}");
    assert!(s.contains("hidden"));
}

// ========== Shell-services verb/app resolution (t51-e10) ==========

use liquide_shell_services::{ShellExecuteError, ShellTarget, ShellVerb};
use std::path::PathBuf;

/// A launcher launch must resolve through the canonical
/// `liquide-shell-services` planner: the registry built from launcher apps
/// produces a spawn-free command plan for the requested app/verb.
#[test]
fn resolve_launch_drives_shell_services_plan() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));

    let plan = launcher
        .resolve_launch(
            "term",
            ShellTarget::File(PathBuf::from("/tmp/doc.txt")),
            ShellVerb::Open,
        )
        .expect("registered app with exec should resolve");

    // The plan reflects the canonical shell-services resolution, not the
    // launcher's ad-hoc open_app_window shortcut.
    assert_eq!(plan.app_id, "term");
    assert_eq!(plan.app_name, "Terminal");
    assert_eq!(plan.verb, ShellVerb::Open);
    // make_app's exec is "/usr/bin/term" (no field codes) → single argv token.
    assert_eq!(plan.command, vec!["/usr/bin/term".to_string()]);
}

/// The association registry built from launcher apps registers exactly the
/// apps that carry an `exec` command (shell-services requires a command).
#[test]
fn build_association_registry_registers_exec_apps() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("term", "Terminal"));
    // An app with no exec must be skipped (cannot be planned).
    let mut no_exec = make_app("noexec", "No Exec");
    no_exec.exec = None;
    launcher.add_app(no_exec);

    let registry = launcher.build_association_registry();

    // The exec app resolves.
    let plan = registry
        .plan_execute(liquide_shell_services::ShellExecuteRequest {
            targets: vec![ShellTarget::Uri("https://example.com".into())],
            verb: ShellVerb::Open,
            app_id_override: Some("term".into()),
        })
        .expect("term should be registered");
    assert_eq!(plan.app_id, "term");

    // The no-exec app was skipped → unknown application.
    let err = registry
        .plan_execute(liquide_shell_services::ShellExecuteRequest {
            targets: vec![ShellTarget::Uri("https://example.com".into())],
            verb: ShellVerb::Open,
            app_id_override: Some("noexec".into()),
        })
        .unwrap_err();
    assert!(matches!(err, ShellExecuteError::UnknownApplication { .. }));
}

/// Resolving an app the launcher does not know surfaces the canonical
/// shell-services error, proving resolution flows through the real planner.
#[test]
fn resolve_launch_unknown_app_errors_via_shell_services() {
    let launcher = default_launcher();
    let err = launcher
        .resolve_launch(
            "ghost",
            ShellTarget::Uri("https://example.com".into()),
            ShellVerb::Open,
        )
        .unwrap_err();
    assert!(matches!(err, ShellExecuteError::UnknownApplication { .. }));
}

/// `open_app_window` must consult the canonical shell-services registry on every
/// launch — caching it in `chrome_shell_services` and flipping the
/// `ShellServices` wiring bit. This is the regression guard that keeps the field
/// genuinely WIRED (t177): if the live consumer in `open_app_window` were
/// removed, the field would go back to never-read and this test would fail.
#[test]
fn open_app_window_consults_shell_services_registry() {
    use crate::shell::{Shell, WiringBit};

    let mut shell = Shell::new(1920.0, 1080.0);

    // Before any launch the registry cache is dormant and the bit is unset.
    assert!(shell.chrome_shell_services.is_none());
    assert!(!shell.wiring_report().is_driven(WiringBit::ShellServices));

    // Register an exec-backed launcher app so the canonical planner resolves a
    // real spawn-free plan (the built-in apps run in-process with no Exec).
    shell.launcher_mut().add_app(make_app("term", "Terminal"));

    let _wid = shell.open_app_window("term");

    // The launch consulted + cached the canonical registry, and the wiring bit
    // flipped — the field is genuinely read on the live launch path.
    assert!(
        shell.chrome_shell_services.is_some(),
        "open_app_window must cache the canonical association registry"
    );
    assert!(
        shell.wiring_report().is_driven(WiringBit::ShellServices),
        "open_app_window must drive the ShellServices wiring bit"
    );

    // The cached registry resolves the exec-backed app to a real command plan.
    let plan = shell
        .plan_app_launch("term")
        .expect("exec-backed app resolves through shell-services");
    assert_eq!(plan.app_id, "term");
    assert_eq!(plan.command, vec!["/usr/bin/term".to_string()]);
}

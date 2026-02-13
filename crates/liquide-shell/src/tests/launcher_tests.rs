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

#[test]
fn fuzzy_score_exact_match() {
    let score = Launcher::fuzzy_score("terminal", "terminal");
    assert!((score - 1.0).abs() < 0.01);
}

#[test]
fn fuzzy_score_prefix_match() {
    let score = Launcher::fuzzy_score("fire", "firefox");
    assert!((score - 0.9).abs() < 0.01);
}

#[test]
fn fuzzy_score_substring_match() {
    let score = Launcher::fuzzy_score("fox", "firefox");
    assert!((score - 0.7).abs() < 0.01);
}

#[test]
fn fuzzy_score_subsequence_match() {
    let score = Launcher::fuzzy_score("ffx", "firefox");
    assert!((score - 0.3).abs() < 0.01);
}

#[test]
fn fuzzy_score_no_match() {
    let score = Launcher::fuzzy_score("zzz", "firefox");
    assert!((score - 0.0).abs() < 0.01);
}

#[test]
fn fuzzy_score_empty_query() {
    assert!((Launcher::fuzzy_score("", "firefox") - 0.0).abs() < 0.01);
}

#[test]
fn fuzzy_score_empty_target() {
    assert!((Launcher::fuzzy_score("fire", "") - 0.0).abs() < 0.01);
}

#[test]
fn fuzzy_score_case_insensitive() {
    let score = Launcher::fuzzy_score("FIRE", "Firefox");
    assert!((score - 0.9).abs() < 0.01);
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
fn search_empty_query_shows_no_results() {
    let mut launcher = default_launcher();
    launcher.add_app(make_app("foo", "Foo"));
    launcher.set_query("");
    assert_eq!(launcher.result_count(), 0);
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
    assert_eq!(launcher.result_count(), 0);
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

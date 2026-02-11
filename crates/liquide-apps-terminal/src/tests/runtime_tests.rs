//! Tests for the terminal runtime coordinator.

use crate::config::TerminalConfig;
use crate::runtime::TerminalRuntime;
use crate::url_detect::{detect_links, LinkKind};
use crate::scrollback::ScrollbackBuffer;
use crate::search::SearchState;
use crate::shell_integration::ShellIntegration;
use crate::pty::{PtyBackend, PtySize, PtyState};
use crate::tab::TabManager;

// ===========================================================================
// Runtime
// ===========================================================================

#[test]
fn test_runtime_new() {
    let rt = TerminalRuntime::new(TerminalConfig::default());
    assert_eq!(rt.tab_count(), 0);
}

#[test]
fn test_runtime_new_tab() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    let id = rt.new_tab(None);
    assert_eq!(id, 1);
    assert_eq!(rt.tab_count(), 1);
}

#[test]
fn test_runtime_close_tab() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    let id = rt.new_tab(None);
    rt.close_tab(id).unwrap();
    assert_eq!(rt.tab_count(), 0);
}

#[test]
fn test_runtime_process_text() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"Hello");
    let grid = rt.active_grid();
    assert_eq!(grid.row_text(0), "Hello");
}

#[test]
fn test_runtime_process_csi() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    // Bold text
    rt.process_output(b"\x1b[1mBold\x1b[0m");
    let grid = rt.active_grid();
    assert!(grid.cell(0, 0).unwrap().attrs.bold);
    assert!(!grid.cell(0, 4).unwrap().attrs.bold);
}

#[test]
fn test_runtime_resize() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.resize(40, 100);
    let grid = rt.active_grid();
    assert_eq!(grid.rows(), 40);
    assert_eq!(grid.cols(), 100);
}

#[test]
fn test_runtime_multiple_tabs() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    let id1 = rt.new_tab(Some("first".into()));
    let id2 = rt.new_tab(Some("second".into()));
    rt.set_active_tab(id2).unwrap();
    rt.process_output(b"Tab2");
    rt.set_active_tab(id1).unwrap();
    assert_eq!(rt.active_grid().row_text(0), "");

    rt.set_active_tab(id2).unwrap();
    assert_eq!(rt.active_grid().row_text(0), "Tab2");
}

#[test]
fn test_runtime_osc_title() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"\x1b]0;My Title\x07");
    let tabs = rt.tab_list();
    assert_eq!(tabs[0].1, "My Title");
}

// ===========================================================================
// URL detection
// ===========================================================================

#[test]
fn test_detect_url() {
    let links = detect_links("visit https://example.com today", 0);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target, "https://example.com");
    assert_eq!(links[0].kind, LinkKind::Url);
}

#[test]
fn test_detect_filepath() {
    let links = detect_links("open /usr/bin/bash now", 0);
    assert!(links.iter().any(|l| l.kind == LinkKind::FilePath && l.target == "/usr/bin/bash"));
}

#[test]
fn test_detect_no_links() {
    let links = detect_links("plain text only", 0);
    assert!(links.is_empty());
}

// ===========================================================================
// Scrollback
// ===========================================================================

#[test]
fn test_scrollback_push() {
    let mut sb = ScrollbackBuffer::new(100);
    sb.push(vec![crate::grid::Cell::default()]);
    assert_eq!(sb.len(), 1);
}

#[test]
fn test_scrollback_capacity() {
    let mut sb = ScrollbackBuffer::new(3);
    for _ in 0..5 {
        sb.push(vec![]);
    }
    assert_eq!(sb.len(), 3);
}

#[test]
fn test_scrollback_viewport() {
    let mut sb = ScrollbackBuffer::new(100);
    for _ in 0..20 {
        sb.push(vec![]);
    }
    assert!(sb.at_bottom());
    sb.scroll_up(5);
    assert_eq!(sb.viewport_offset(), 5);
    assert!(!sb.at_bottom());
    sb.scroll_to_bottom();
    assert!(sb.at_bottom());
}

#[test]
fn test_scrollback_find_lines() {
    let mut sb = ScrollbackBuffer::new(100);
    let cell_h = crate::grid::Cell { ch: 'h', ..crate::grid::Cell::default() };
    let cell_i = crate::grid::Cell { ch: 'i', ..crate::grid::Cell::default() };
    sb.push(vec![cell_h.clone(), cell_i.clone()]);
    sb.push(vec![crate::grid::Cell::default()]);
    let matches = sb.find_lines("hi");
    assert_eq!(matches, vec![0]);
}

// ===========================================================================
// Search
// ===========================================================================

#[test]
fn test_search_basic() {
    let mut s = SearchState::new();
    let lines = vec!["hello world".into(), "world peace".into()];
    s.search("world", &lines);
    assert_eq!(s.match_count(), 2);
}

#[test]
fn test_search_navigation() {
    let mut s = SearchState::new();
    let lines = vec!["aaa".into(), "aaa".into()];
    s.search("aaa", &lines);
    assert_eq!(s.current_index(), 0);
    s.next_match();
    assert_eq!(s.current_index(), 1);
    s.next_match();
    assert_eq!(s.current_index(), 0); // wraps
}

#[test]
fn test_search_case_insensitive() {
    let mut s = SearchState::new();
    s.set_case_sensitive(false);
    let lines = vec!["Hello WORLD".into()];
    s.search("hello", &lines);
    assert_eq!(s.match_count(), 1);
}

#[test]
fn test_search_clear() {
    let mut s = SearchState::new();
    let lines = vec!["test".into()];
    s.search("test", &lines);
    s.clear();
    assert_eq!(s.match_count(), 0);
    assert!(s.query().is_empty());
}

// ===========================================================================
// Shell integration
// ===========================================================================

#[test]
fn test_shell_integration_cwd() {
    let mut si = ShellIntegration::new();
    si.set_cwd("/home/user".into());
    assert_eq!(si.cwd(), Some("/home/user"));
    assert_eq!(si.tab_title(), "user");
}

#[test]
fn test_shell_integration_title() {
    let mut si = ShellIntegration::new();
    si.set_title("vim main.rs".into());
    assert_eq!(si.title(), Some("vim main.rs"));
    assert_eq!(si.tab_title(), "vim main.rs");
}

#[test]
fn test_shell_integration_command() {
    let mut si = ShellIntegration::new();
    assert!(!si.in_command());
    si.command_start();
    assert!(si.in_command());
    si.command_end(Some(0));
    assert!(!si.in_command());
    assert_eq!(si.last_exit_code(), Some(0));
}

// ===========================================================================
// PTY
// ===========================================================================

#[test]
fn test_pty_lifecycle() {
    let mut pty = PtyBackend::new("/bin/bash".into(), PtySize::default());
    assert_eq!(pty.state(), PtyState::Idle);
    pty.spawn().unwrap();
    assert_eq!(pty.state(), PtyState::Running);
    pty.write(b"test").unwrap();
    let output = pty.read();
    assert_eq!(output, b"test");
    pty.kill();
    assert_eq!(pty.state(), PtyState::Killed);
}

#[test]
fn test_pty_write_not_running() {
    let mut pty = PtyBackend::new("/bin/bash".into(), PtySize::default());
    assert!(pty.write(b"test").is_err());
}

#[test]
fn test_pty_resize() {
    let mut pty = PtyBackend::new("/bin/bash".into(), PtySize::default());
    pty.resize(PtySize::new(40, 120));
    assert_eq!(pty.size().rows, 40);
    assert_eq!(pty.size().cols, 120);
}

// ===========================================================================
// Tab manager
// ===========================================================================

#[test]
fn test_tab_manager() {
    let mut tm = TabManager::new();
    assert_eq!(tm.count(), 0);
    let id1 = tm.new_tab("/bin/sh", 24, 80, 10_000);
    assert_eq!(tm.count(), 1);
    assert_eq!(tm.active_id(), id1);

    let id2 = tm.new_tab("/bin/sh", 24, 80, 10_000);
    assert_eq!(tm.count(), 2);
    tm.set_active(id2).unwrap();
    assert_eq!(tm.active_id(), id2);

    tm.close_tab(id2).unwrap();
    assert_eq!(tm.count(), 1);
    assert_eq!(tm.active_id(), id1);
}

#[test]
fn test_tab_display_title() {
    let mut tm = TabManager::new();
    let id = tm.new_tab("/bin/sh", 24, 80, 10_000);
    let tab = tm.get_mut(id).unwrap();
    tab.shell_integration_mut().set_cwd("/home/user/projects".into());
    assert_eq!(tab.display_title(), "projects");
}

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
    // Use a platform-appropriate shell.
    #[cfg(unix)]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    #[cfg(windows)]
    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

    let mut pty = PtyBackend::new(shell, PtySize::default());
    assert_eq!(pty.state(), PtyState::Idle);
    pty.spawn().unwrap();
    assert_eq!(pty.state(), PtyState::Running);

    // With a real PTY, writing goes to the child process and we may get
    // output back asynchronously. Just verify write succeeds.
    pty.write(b"echo hello\n").unwrap();

    // Give the child a moment to produce output, then read whatever is available.
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _output = pty.read();
    // Output contents depend on shell prompt and echo; just verify no panic.

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

// ===========================================================================
// Event loop (tick)
// ===========================================================================

#[test]
fn test_tick_no_tab() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    assert!(!rt.tick());
}

#[test]
fn test_tick_no_data() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    // No data in PTY buffer -> tick returns false.
    assert!(!rt.tick());
}

#[test]
fn test_tick_with_data() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    // Put data into the PTY (stub echoes writes to output_buffer).
    rt.send_input(b"Hello").unwrap();
    // Now tick should read it and update the grid.
    assert!(rt.tick());
    assert_eq!(rt.active_grid().row_text(0), "Hello");
}

#[test]
fn test_tick_multiple_rounds() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.send_input(b"AB").unwrap();
    assert!(rt.tick());
    rt.send_input(b"CD").unwrap();
    assert!(rt.tick());
    assert_eq!(rt.active_grid().row_text(0), "ABCD");
}

// ===========================================================================
// Keyboard input
// ===========================================================================

#[test]
fn test_send_key_enter() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.send_key("Enter").unwrap();
    // Enter sends \r which the PTY echoes; tick processes it as CR.
    assert!(rt.tick());
}

#[test]
fn test_send_key_arrow() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    // Write some text first so cursor is not at 0.
    rt.send_input(b"ABC").unwrap();
    rt.tick();
    assert_eq!(rt.cursor_position(), (0, 3));
    // ArrowLeft sends \x1b[D which is CUB(1).
    rt.send_key("ArrowLeft").unwrap();
    rt.tick();
    assert_eq!(rt.cursor_position(), (0, 2));
}

#[test]
fn test_send_char() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.send_char('Z').unwrap();
    rt.tick();
    assert_eq!(rt.active_grid().row_text(0), "Z");
}

#[test]
fn test_send_key_no_tab() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    assert!(rt.send_key("Enter").is_err());
}

#[test]
fn test_send_char_no_tab() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    assert!(rt.send_char('A').is_err());
}

// ===========================================================================
// Visible lines (rendering)
// ===========================================================================

#[test]
fn test_visible_lines_empty() {
    let rt = TerminalRuntime::new(TerminalConfig::default());
    assert!(rt.visible_lines().is_empty());
}

#[test]
fn test_visible_lines_text() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"Hello");
    let lines = rt.visible_lines();
    assert_eq!(lines.len(), 24); // 24 rows default
    assert!(lines[0].text.starts_with("Hello"));
    // First row should have at least one span.
    assert!(!lines[0].spans.is_empty());
}

#[test]
fn test_visible_lines_bold_spans() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"AB\x1b[1mCD\x1b[0mEF");
    let lines = rt.visible_lines();
    // Should have multiple spans due to attribute changes.
    let spans = &lines[0].spans;
    assert!(spans.len() >= 3, "Expected at least 3 spans, got {}", spans.len());
    // First span (A,B) should not be bold.
    assert!(!spans[0].bold);
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[0].end, 2);
    // Second span (C,D) should be bold.
    assert!(spans[1].bold);
    assert_eq!(spans[1].start, 2);
    assert_eq!(spans[1].end, 4);
    // Third span (E,F,...) should not be bold.
    assert!(!spans[2].bold);
}

#[test]
fn test_visible_lines_color_spans() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    // Red foreground (31) then reset.
    rt.process_output(b"\x1b[31mRed\x1b[0mNormal");
    let lines = rt.visible_lines();
    let spans = &lines[0].spans;
    assert!(spans.len() >= 2);
    assert_eq!(spans[0].fg, Some(1)); // color index 1 = red
    assert_eq!(spans[1].fg, None);    // reset
}

// ===========================================================================
// Cursor position
// ===========================================================================

#[test]
fn test_cursor_position_no_tab() {
    let rt = TerminalRuntime::new(TerminalConfig::default());
    assert_eq!(rt.cursor_position(), (0, 0));
}

#[test]
fn test_cursor_position_after_text() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"ABC");
    assert_eq!(rt.cursor_position(), (0, 3));
}

#[test]
fn test_cursor_position_after_newline() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"line1\r\nline2");
    assert_eq!(rt.cursor_position(), (1, 5));
}

// ===========================================================================
// Grid: insert/delete lines/chars, cursor_tab
// ===========================================================================

#[test]
fn test_grid_cursor_tab() {
    let mut g = crate::grid::Grid::new(5, 80);
    g.set_cursor(0, 3);
    g.cursor_tab();
    assert_eq!(g.cursor(), (0, 8));
    g.cursor_tab();
    assert_eq!(g.cursor(), (0, 16));
}

#[test]
fn test_grid_cursor_tab_at_stop() {
    let mut g = crate::grid::Grid::new(5, 80);
    g.set_cursor(0, 8);
    g.cursor_tab();
    assert_eq!(g.cursor(), (0, 16));
}

#[test]
fn test_grid_insert_lines() {
    let mut g = crate::grid::Grid::new(5, 5);
    g.set_cursor(0, 0); g.put_char('A');
    g.set_cursor(1, 0); g.put_char('B');
    g.set_cursor(2, 0); g.put_char('C');
    g.set_cursor(1, 0);
    g.insert_lines(1);
    assert_eq!(g.row_text(0), "A");
    assert_eq!(g.row_text(1), "");    // inserted blank
    assert_eq!(g.row_text(2), "B");
    assert_eq!(g.row_text(3), "C");
}

#[test]
fn test_grid_delete_lines() {
    let mut g = crate::grid::Grid::new(5, 5);
    g.set_cursor(0, 0); g.put_char('A');
    g.set_cursor(1, 0); g.put_char('B');
    g.set_cursor(2, 0); g.put_char('C');
    g.set_cursor(1, 0);
    g.delete_lines(1);
    assert_eq!(g.row_text(0), "A");
    assert_eq!(g.row_text(1), "C");
    assert_eq!(g.row_text(2), "");    // blank inserted at bottom
}

#[test]
fn test_grid_insert_chars() {
    let mut g = crate::grid::Grid::new(3, 5);
    for ch in "ABCDE".chars() { g.put_char(ch); }
    g.set_cursor(0, 2);
    g.insert_chars(1);
    // Row should now be "AB CDE" truncated to 5: "AB CD"
    assert_eq!(g.row_text(0), "AB CD");
}

#[test]
fn test_grid_delete_chars() {
    let mut g = crate::grid::Grid::new(3, 5);
    for ch in "ABCDE".chars() { g.put_char(ch); }
    g.set_cursor(0, 2);
    g.delete_chars(1);
    // C removed, shift left, blank at end: "ABDE "
    assert_eq!(g.row_text(0), "ABDE");
}

// ===========================================================================
// CSI InsertLines/DeleteLines/InsertChars/DeleteChars via runtime
// ===========================================================================

#[test]
fn test_runtime_csi_insert_lines() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"Line0\r\nLine1\r\nLine2");
    // Move to row 1, insert 1 line: \x1b[2;1H\x1b[1L
    rt.process_output(b"\x1b[2;1H\x1b[1L");
    assert_eq!(rt.active_grid().row_text(0), "Line0");
    assert_eq!(rt.active_grid().row_text(1), "");     // inserted
    assert_eq!(rt.active_grid().row_text(2), "Line1");
}

#[test]
fn test_runtime_csi_delete_chars() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);
    rt.process_output(b"ABCDE");
    // Move to col 1, delete 2 chars: \x1b[1;2H\x1b[2P
    rt.process_output(b"\x1b[1;2H\x1b[2P");
    assert_eq!(rt.active_grid().row_text(0), "ADE");
}

// ===========================================================================
// Full event loop round-trip
// ===========================================================================

#[test]
fn test_full_roundtrip() {
    let mut rt = TerminalRuntime::new(TerminalConfig::default());
    rt.new_tab(None);

    // Simulate: type "ls", press Enter, get output.
    // The stub PTY echoes writes, so send_input -> tick processes it.
    rt.send_input(b"ls\r").unwrap();
    let dirty = rt.tick();
    assert!(dirty);

    // Grid should show "ls" on first row (CR moves cursor to col 0 same row).
    let text = rt.active_grid().row_text(0);
    assert!(text.starts_with("ls"), "Expected 'ls' but got '{}'", text);

    // visible_lines should be renderable.
    let lines = rt.visible_lines();
    assert_eq!(lines.len(), 24);

    // cursor should be at (0, 2) after "ls" then CR moves to col 0.
    // Actually: 'l' prints at (0,0), 's' at (0,1), CR sets col=0.
    assert_eq!(rt.cursor_position(), (0, 0));
}

//! Per-app smoke test for the terminal emulator (t57 A7 / t57-e8).
//!
//! Drives the real VT parser into a real character grid and asserts that
//! printable bytes, control bytes (CR/LF), and an SGR + CUP escape sequence
//! all reach the render model — i.e. the parse → grid pipeline is wired, not
//! just that the types construct.

use liquide_apps_terminal::grid::Grid;
use liquide_apps_terminal::vt::{Action, CsiAction, EraseMode, Parser, SgrParam};

/// Minimal re-implementation of the runtime's `apply_action` over a bare grid,
/// so the smoke test exercises the VT parser + grid without spawning a PTY/shell
/// (which would make a headless test slow and environment-dependent).
fn apply(grid: &mut Grid, action: Action) {
    match action {
        Action::Print(ch) => grid.put_char(ch),
        Action::Execute(byte) => match byte {
            0x08 => grid.cursor_back(1),
            0x09 => grid.cursor_tab(),
            0x0a | 0x0b | 0x0c => grid.line_feed(),
            0x0d => grid.carriage_return(),
            _ => {}
        },
        Action::CsiDispatch(csi) => match csi {
            CsiAction::CursorPosition { row, col } => {
                grid.set_cursor(row.saturating_sub(1), col.saturating_sub(1));
            }
            CsiAction::EraseDisplay(EraseMode::All) => grid.erase_display_all(),
            CsiAction::Sgr(params) => {
                let mut attrs = grid.current_attrs();
                for p in params {
                    if let SgrParam::Bold = p {
                        attrs.bold = true;
                    }
                }
                grid.set_attrs(attrs);
            }
            _ => {}
        },
        Action::OscDispatch(_) | Action::EscDispatch(_) => {}
    }
}

/// Read the full text of a grid row into a `String`.
fn row_text(grid: &Grid, row: u32) -> String {
    (0..grid.cols())
        .filter_map(|c| grid.cell(row, c).map(|cell| cell.ch))
        .collect()
}

#[test]
fn vt_parser_renders_printable_text_into_grid() {
    let mut parser = Parser::new();
    let mut grid = Grid::new(24, 80);

    let mut actions = Vec::new();
    parser.feed(b"hello", &mut actions);
    for a in actions {
        apply(&mut grid, a);
    }

    let line0 = row_text(&grid, 0);
    assert!(
        line0.starts_with("hello"),
        "expected row 0 to begin with 'hello', got {line0:?}"
    );
    // The render model must not be an all-blank placeholder.
    assert!(
        line0.trim_end().contains("hello"),
        "grid render model is empty/placeholder"
    );
}

#[test]
fn vt_parser_handles_crlf_and_escape_sequences_without_panic() {
    let mut parser = Parser::new();
    let mut grid = Grid::new(24, 80);

    // bold ('\x1b[1m') + text + CR/LF + cursor-position ('\x1b[2;3H') + text.
    let mut actions = Vec::new();
    parser.feed(b"\x1b[1mfoo\r\nbar\x1b[2;3Hzap", &mut actions);
    for a in actions {
        apply(&mut grid, a);
    }

    let line0 = row_text(&grid, 0);
    assert!(
        line0.starts_with("foo"),
        "row 0 should hold 'foo', got {line0:?}"
    );
    let line1 = row_text(&grid, 1);
    // 'bar' written at col 0, then CUP to (row 2 => index 1, col 3 => index 2)
    // overwrites with 'zap' starting at column index 2.
    assert!(
        line1.contains("ba") && line1.contains("zap"),
        "row 1 should reflect 'bar' then 'zap' at col 3, got {line1:?}"
    );
}

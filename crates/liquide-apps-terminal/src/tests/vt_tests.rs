//! Tests for the VT parser.

use crate::vt::{Action, CsiAction, EraseMode, OscAction, Parser, SgrParam};

fn parse(input: &[u8]) -> Vec<Action> {
    let mut parser = Parser::new();
    let mut actions = Vec::new();
    parser.feed(input, &mut actions);
    actions
}

// ===========================================================================
// Print
// ===========================================================================

#[test]
fn test_parse_plain_text() {
    let actions = parse(b"hello");
    assert_eq!(actions.len(), 5);
    assert_eq!(actions[0], Action::Print('h'));
    assert_eq!(actions[4], Action::Print('o'));
}

// ===========================================================================
// C0 controls
// ===========================================================================

#[test]
fn test_parse_cr_lf() {
    let actions = parse(b"\r\n");
    assert_eq!(actions[0], Action::Execute(0x0d));
    assert_eq!(actions[1], Action::Execute(0x0a));
}

#[test]
fn test_parse_backspace() {
    let actions = parse(b"\x08");
    assert_eq!(actions[0], Action::Execute(0x08));
}

// ===========================================================================
// CSI sequences
// ===========================================================================

#[test]
fn test_cursor_up() {
    let actions = parse(b"\x1b[3A");
    assert_eq!(actions, vec![Action::CsiDispatch(CsiAction::CursorUp(3))]);
}

#[test]
fn test_cursor_down() {
    let actions = parse(b"\x1b[B");
    assert_eq!(actions, vec![Action::CsiDispatch(CsiAction::CursorDown(1))]);
}

#[test]
fn test_cursor_forward() {
    let actions = parse(b"\x1b[5C");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::CursorForward(5))]
    );
}

#[test]
fn test_cursor_back() {
    let actions = parse(b"\x1b[2D");
    assert_eq!(actions, vec![Action::CsiDispatch(CsiAction::CursorBack(2))]);
}

#[test]
fn test_cursor_position() {
    let actions = parse(b"\x1b[10;20H");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::CursorPosition {
            row: 10,
            col: 20
        })]
    );
}

#[test]
fn test_cursor_position_default() {
    let actions = parse(b"\x1b[H");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::CursorPosition {
            row: 1,
            col: 1
        })]
    );
}

#[test]
fn test_erase_display_to_end() {
    let actions = parse(b"\x1b[J");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::EraseDisplay(
            EraseMode::ToEnd
        ))]
    );
}

#[test]
fn test_erase_display_all() {
    let actions = parse(b"\x1b[2J");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::EraseDisplay(EraseMode::All))]
    );
}

#[test]
fn test_erase_line() {
    let actions = parse(b"\x1b[1K");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::EraseLine(
            EraseMode::ToBeginning
        ))]
    );
}

// ===========================================================================
// SGR
// ===========================================================================

#[test]
fn test_sgr_reset() {
    let actions = parse(b"\x1b[0m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![SgrParam::Reset]))]
    );
}

#[test]
fn test_sgr_bold_italic() {
    let actions = parse(b"\x1b[1;3m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![
            SgrParam::Bold,
            SgrParam::Italic
        ]))]
    );
}

#[test]
fn test_sgr_foreground_color() {
    let actions = parse(b"\x1b[31m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![
            SgrParam::Foreground(1)
        ]))]
    );
}

#[test]
fn test_sgr_256_color() {
    let actions = parse(b"\x1b[38;5;200m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![
            SgrParam::Foreground(200)
        ]))]
    );
}

#[test]
fn test_sgr_truecolor() {
    let actions = parse(b"\x1b[38;2;255;128;0m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![
            SgrParam::ForegroundRgb(255, 128, 0)
        ]))]
    );
}

#[test]
fn test_sgr_empty_is_reset() {
    let actions = parse(b"\x1b[m");
    assert_eq!(
        actions,
        vec![Action::CsiDispatch(CsiAction::Sgr(vec![SgrParam::Reset]))]
    );
}

// ===========================================================================
// OSC sequences
// ===========================================================================

#[test]
fn test_osc_set_title() {
    let actions = parse(b"\x1b]0;My Title\x07");
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::SetTitle("My Title".into()))]
    );
}

#[test]
fn test_osc_set_cwd() {
    let actions = parse(b"\x1b]7;file:///home/user\x07");
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::SetWorkingDirectory(
            "file:///home/user".into()
        ))]
    );
}

#[test]
fn test_osc_command_start() {
    let actions = parse(b"\x1b]133;A\x07");
    assert_eq!(actions, vec![Action::OscDispatch(OscAction::CommandStart)]);
}

#[test]
fn test_osc_command_end() {
    let actions = parse(b"\x1b]133;D;0\x07");
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::CommandEnd(Some(0)))]
    );
}

#[test]
fn test_osc_hyperlink() {
    let actions = parse(b"\x1b]8;;https://example.com\x07");
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::Hyperlink {
            url: "https://example.com".into(),
            id: None,
        })]
    );
}

// ===========================================================================
// Mixed sequences
// ===========================================================================

#[test]
fn test_mixed_text_and_csi() {
    let actions = parse(b"AB\x1b[1mCD");
    assert_eq!(actions.len(), 5);
    assert_eq!(actions[0], Action::Print('A'));
    assert_eq!(actions[1], Action::Print('B'));
    assert_eq!(
        actions[2],
        Action::CsiDispatch(CsiAction::Sgr(vec![SgrParam::Bold]))
    );
    assert_eq!(actions[3], Action::Print('C'));
    assert_eq!(actions[4], Action::Print('D'));
}

// ===========================================================================
// Scroll
// ===========================================================================

#[test]
fn test_scroll_up() {
    let actions = parse(b"\x1b[3S");
    assert_eq!(actions, vec![Action::CsiDispatch(CsiAction::ScrollUp(3))]);
}

#[test]
fn test_scroll_down() {
    let actions = parse(b"\x1b[2T");
    assert_eq!(actions, vec![Action::CsiDispatch(CsiAction::ScrollDown(2))]);
}

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

// ===========================================================================
// Bounded buffers / malformed-stream recovery (regression for t49-e10-F8)
// ===========================================================================

#[test]
fn test_csi_overlong_param_run_aborts_and_recovers() {
    // A CSI parameter run far longer than any legitimate sequence must not
    // grow the parser's buffers without bound; the sequence is aborted and the
    // parser returns to ground so subsequent input still parses.
    let mut parser = Parser::new();
    let mut actions = Vec::new();

    // 100_000 bytes of '1' digit parameters: well beyond the internal cap and
    // never terminated by a final byte.
    let mut garbage = Vec::with_capacity(100_002);
    garbage.extend_from_slice(b"\x1b[");
    garbage.extend(std::iter::repeat(b'1').take(100_000));
    parser.feed(&garbage, &mut actions);

    // The overlong CSI was discarded: it never dispatched a CSI action. (After
    // the cap aborts the sequence the parser is back in ground, so trailing
    // digits print as ordinary characters — bounded, not accumulated.)
    assert!(
        !actions.iter().any(|a| matches!(a, Action::CsiDispatch(_))),
        "overlong CSI must not dispatch a CSI action"
    );

    // After the garbage, a normal sequence and plain text still parse: proves
    // the parser recovered to the ground state rather than staying wedged.
    actions.clear();
    parser.feed(b"\x1b[3AX", &mut actions);
    assert_eq!(
        actions,
        vec![
            Action::CsiDispatch(CsiAction::CursorUp(3)),
            Action::Print('X'),
        ]
    );
}

#[test]
fn test_osc_unterminated_overlong_aborts_and_recovers() {
    // An unterminated OSC string fed an enormous run of bytes must be discarded
    // once it exceeds the cap, and the parser must recover to ground.
    let mut parser = Parser::new();
    let mut actions = Vec::new();

    let mut garbage = Vec::with_capacity(200_002);
    garbage.extend_from_slice(b"\x1b]"); // OSC introducer, never terminated
    garbage.extend(std::iter::repeat(b'A').take(200_000));
    parser.feed(&garbage, &mut actions);

    // The overlong unterminated OSC was discarded: no OSC action dispatched.
    // (After the cap aborts the sequence, trailing bytes print as characters —
    // bounded, not accumulated into osc_buf.)
    assert!(
        !actions.iter().any(|a| matches!(a, Action::OscDispatch(_))),
        "unterminated overlong OSC must not dispatch an OSC action"
    );

    // Parser recovered: a well-formed OSC title still parses afterwards.
    actions.clear();
    parser.feed(b"\x1b]0;ok\x07", &mut actions);
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::SetTitle("ok".into()))]
    );
}

#[test]
fn test_normal_length_osc_still_parses_after_cap() {
    // A long-but-legitimate OSC 8 hyperlink (under the cap) must still parse
    // correctly — the cap must not be so small as to break real sequences.
    let url = "https://example.com/".to_string() + &"a".repeat(2000);
    let mut input = Vec::new();
    input.extend_from_slice(b"\x1b]8;;");
    input.extend_from_slice(url.as_bytes());
    input.push(0x07);

    let actions = parse(&input);
    assert_eq!(
        actions,
        vec![Action::OscDispatch(OscAction::Hyperlink { url, id: None })]
    );
}

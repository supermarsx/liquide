//! Per-app smoke test for the text editor (t57 A7 / t57-e8).
//!
//! Constructs the editor runtime, opens a document, types text through the
//! real keyboard/char event path, and asserts the buffer actually contains
//! what was typed and that undo reverses it — real behavior, not construction.

use liquide_apps_text_editor::config::EditorConfig;
use liquide_apps_text_editor::runtime::EditorRuntime;

#[test]
fn typing_inserts_text_into_active_buffer() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.new_document();
    assert_eq!(rt.document_count(), 1);

    for ch in "hi".chars() {
        assert!(rt.handle_char(ch), "handle_char should report modification");
    }
    rt.handle_key("Enter", false, false);
    for ch in "world".chars() {
        rt.handle_char(ch);
    }

    let doc = rt.document(id).expect("document must exist");
    let text = doc.buffer.text();
    assert!(
        text.contains("hi") && text.contains("world"),
        "buffer should contain typed text, got {text:?}"
    );
    assert_eq!(
        doc.buffer.line_count(),
        2,
        "Enter should have produced a second line"
    );
}

#[test]
fn undo_reverses_an_insertion() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();

    rt.handle_char('x');
    assert!(rt.active_document().unwrap().buffer.text().contains('x'));

    // Ctrl+Z
    rt.handle_key("z", true, false);
    let text = rt.active_document().unwrap().buffer.text();
    assert!(
        !text.contains('x'),
        "undo should have removed the inserted char, got {text:?}"
    );
}

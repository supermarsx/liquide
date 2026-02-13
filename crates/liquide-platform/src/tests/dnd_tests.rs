use crate::dnd::{NativeDragDrop, NullDragDrop};

#[test]
fn start_drag_returns_ok() {
    let mut dnd = NullDragDrop;
    let mime_types = vec!["text/plain".to_string()];
    assert!(dnd.start_drag(&mime_types, b"hello").is_ok());
}

#[test]
fn start_drag_empty_data_returns_ok() {
    let mut dnd = NullDragDrop;
    let mime_types = vec!["application/octet-stream".to_string()];
    assert!(dnd.start_drag(&mime_types, &[]).is_ok());
}

#[test]
fn accept_drop_returns_true() {
    let mut dnd = NullDragDrop;
    let result = dnd.accept_drop("text/plain").unwrap();
    assert!(result);
}

#[test]
fn accept_drop_any_mime_returns_true() {
    let mut dnd = NullDragDrop;
    assert!(dnd.accept_drop("image/png").unwrap());
    assert!(dnd.accept_drop("application/json").unwrap());
}

#[test]
fn cancel_drag_returns_ok() {
    let mut dnd = NullDragDrop;
    assert!(dnd.cancel_drag().is_ok());
}

#[test]
fn null_drag_drop_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullDragDrop>();
}

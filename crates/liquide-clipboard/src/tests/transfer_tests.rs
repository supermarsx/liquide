use crate::format::ClipboardFormat;
use crate::offer::ClipboardOffer;
use crate::transfer::*;

#[test]
fn transfer_initial_idle() {
    let t = ClipboardTransfer::new(1024);
    assert!(matches!(t.state(), TransferState::Idle));
    assert_eq!(t.received_bytes(), 0);
    assert!(!t.is_complete());
}

#[test]
fn transfer_begin_offer() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    assert!(matches!(t.state(), TransferState::Offered { .. }));
}

#[test]
fn transfer_request_format() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(
        1,
        vec![ClipboardFormat::PlainText, ClipboardFormat::Html],
        0,
        1,
    );
    t.begin_offer(offer);
    assert!(t.request_format(ClipboardFormat::PlainText).is_ok());
    assert!(matches!(t.state(), TransferState::Requested { .. }));
}

#[test]
fn transfer_receive_chunk() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    assert!(t.receive_chunk(b"hello").is_ok());
    assert_eq!(t.received_bytes(), 5);
    assert!(matches!(t.state(), TransferState::Transferring { .. }));
}

#[test]
fn transfer_complete() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    t.receive_chunk(b"hello world").unwrap();
    let data = t.complete().unwrap();
    assert_eq!(data, b"hello world");
    assert!(t.is_complete());
}

#[test]
fn transfer_too_large_rejected() {
    let mut t = ClipboardTransfer::new(10);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    let result = t.receive_chunk(b"this is way too large for the buffer");
    assert!(result.is_err());
}

#[test]
fn transfer_reset_returns_idle() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.reset();
    assert!(matches!(t.state(), TransferState::Idle));
    assert_eq!(t.received_bytes(), 0);
}

#[test]
fn transfer_multiple_chunks() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    t.receive_chunk(b"hello ").unwrap();
    t.receive_chunk(b"world").unwrap();
    assert_eq!(t.received_bytes(), 11);
}

#[test]
fn transfer_complete_returns_data() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    t.receive_chunk(b"test data").unwrap();
    let data = t.complete().unwrap();
    assert_eq!(&data, b"test data");
}

#[test]
fn transfer_state_transitions() {
    let mut t = ClipboardTransfer::new(1024);
    assert!(matches!(t.state(), TransferState::Idle));

    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    assert!(matches!(t.state(), TransferState::Offered { .. }));

    t.request_format(ClipboardFormat::PlainText).unwrap();
    assert!(matches!(t.state(), TransferState::Requested { .. }));

    t.receive_chunk(b"data").unwrap();
    assert!(matches!(t.state(), TransferState::Transferring { .. }));

    t.complete().unwrap();
    assert!(matches!(t.state(), TransferState::Complete));
}

use std::collections::HashMap;

use crate::format::*;
use crate::manager::*;
use crate::offer::*;
use crate::store::*;
use crate::transfer::*;

// --- Display impls ---
#[test]
fn format_display() {
    assert_eq!(format!("{}", ClipboardFormat::PlainText), "PlainText");
    assert_eq!(format!("{}", ClipboardFormat::Html), "HTML");
    assert_eq!(format!("{}", ClipboardFormat::Png), "PNG");
    assert_eq!(
        format!("{}", ClipboardFormat::Custom("x/y".into())),
        "Custom(x/y)"
    );
}

#[test]
fn transfer_state_display() {
    assert_eq!(format!("{}", TransferState::Idle), "Idle");
    assert_eq!(format!("{}", TransferState::Complete), "Complete");
    assert_eq!(
        format!("{}", TransferState::Failed("oops".into())),
        "Failed(oops)"
    );

    let ts = TransferState::Transferring {
        received: 100,
        total: Some(1000),
    };
    assert_eq!(format!("{ts}"), "Transferring(100/1000)");

    let ts2 = TransferState::Transferring {
        received: 50,
        total: None,
    };
    assert_eq!(format!("{ts2}"), "Transferring(50/?)");
}

// --- Format edge cases ---
#[test]
fn custom_format_is_not_text() {
    let fmt = ClipboardFormat::Custom("application/octet-stream".to_string());
    assert!(!fmt.is_text());
    assert!(!fmt.is_image());
}

#[test]
fn custom_format_mime_preserved() {
    let mime = "application/x-custom-type";
    let fmt = ClipboardFormat::Custom(mime.to_string());
    assert_eq!(fmt.mime_type(), mime);
}

#[test]
fn from_mime_svg() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_SVG),
        Some(ClipboardFormat::Svg)
    );
}

#[test]
fn from_mime_jpeg() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_JPEG),
        Some(ClipboardFormat::Jpeg)
    );
}

#[test]
fn from_mime_rich_text() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_RICH_TEXT),
        Some(ClipboardFormat::RichText)
    );
}

#[test]
fn from_mime_uri_list() {
    assert_eq!(
        ClipboardFormat::from_mime(MIME_FILE_URI_LIST),
        Some(ClipboardFormat::FileUriList)
    );
}

#[test]
fn format_equality() {
    assert_eq!(ClipboardFormat::PlainText, ClipboardFormat::PlainText);
    assert_ne!(ClipboardFormat::PlainText, ClipboardFormat::Html);
    assert_eq!(
        ClipboardFormat::Custom("a".into()),
        ClipboardFormat::Custom("a".into())
    );
    assert_ne!(
        ClipboardFormat::Custom("a".into()),
        ClipboardFormat::Custom("b".into())
    );
}

// --- Offer edge cases ---
#[test]
fn offer_empty_formats() {
    let offer = ClipboardOffer::new(1, vec![], 0, 1);
    assert!(!offer.has_format(&ClipboardFormat::PlainText));
    assert_eq!(offer.preferred_text_format(), None);
    assert_eq!(offer.preferred_image_format(), None);
}

#[test]
fn offer_only_custom_format() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::Custom("x/y".into())], 0, 1);
    assert!(offer.has_format(&ClipboardFormat::Custom("x/y".into())));
    assert_eq!(offer.preferred_text_format(), None);
    assert_eq!(offer.preferred_image_format(), None);
}

#[test]
fn offer_serde_roundtrip() {
    let offer = ClipboardOffer::new(
        42,
        vec![ClipboardFormat::PlainText, ClipboardFormat::Png],
        1000,
        5,
    );
    let json = serde_json::to_string(&offer).unwrap();
    let back: ClipboardOffer = serde_json::from_str(&json).unwrap();
    assert_eq!(offer, back);
}

#[test]
fn request_serde_roundtrip() {
    let req = ClipboardRequest::new(ClipboardFormat::Html, 99);
    let json = serde_json::to_string(&req).unwrap();
    let back: ClipboardRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req, back);
}

#[test]
fn offer_preferred_text_richtext_only() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::RichText], 0, 1);
    assert_eq!(
        offer.preferred_text_format(),
        Some(&ClipboardFormat::RichText)
    );
}

#[test]
fn offer_preferred_image_svg_only() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::Svg], 0, 1);
    assert_eq!(offer.preferred_image_format(), Some(&ClipboardFormat::Svg));
}

// --- Transfer edge cases ---
#[test]
fn transfer_empty_chunk() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    assert!(t.receive_chunk(b"").is_ok()); // empty chunk is valid
    assert_eq!(t.received_bytes(), 0);
}

#[test]
fn transfer_request_unavailable_format() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    let result = t.request_format(ClipboardFormat::Png);
    assert!(result.is_err());
}

#[test]
fn transfer_request_not_in_offered_state() {
    let mut t = ClipboardTransfer::new(1024);
    // Still in Idle state
    let result = t.request_format(ClipboardFormat::PlainText);
    assert!(result.is_err());
}

#[test]
fn transfer_begin_offer_resets_data() {
    let mut t = ClipboardTransfer::new(1024);
    let offer1 = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer1);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    t.receive_chunk(b"old data").unwrap();
    assert_eq!(t.received_bytes(), 8);

    // New offer should clear
    let offer2 = ClipboardOffer::new(2, vec![ClipboardFormat::Html], 0, 2);
    t.begin_offer(offer2);
    assert_eq!(t.received_bytes(), 0);
}

#[test]
fn transfer_format_accessor() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::Html], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::Html).unwrap();
    assert_eq!(*t.format(), ClipboardFormat::Html);
}

#[test]
fn transfer_abort() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    t.receive_chunk(b"partial").unwrap();
    t.abort("user cancelled");
    assert!(matches!(t.state(), TransferState::Failed(_)));
    assert_eq!(t.received_bytes(), 0);
}

#[test]
fn transfer_complete_empty() {
    let mut t = ClipboardTransfer::new(1024);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    // Complete without receiving any data
    let data = t.complete().unwrap();
    assert!(data.is_empty());
    assert!(t.is_complete());
}

#[test]
fn transfer_max_size_zero() {
    let mut t = ClipboardTransfer::new(0);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    // Even 1 byte should fail
    let result = t.receive_chunk(b"x");
    assert!(result.is_err());
}

#[test]
fn transfer_max_size_exact() {
    let mut t = ClipboardTransfer::new(5);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    t.begin_offer(offer);
    t.request_format(ClipboardFormat::PlainText).unwrap();
    assert!(t.receive_chunk(b"hello").is_ok());
    // One more byte should fail
    let result = t.receive_chunk(b"!");
    assert!(result.is_err());
}

// --- Store edge cases ---
#[test]
fn store_set_empty_data() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, vec![], 1, 0).unwrap();
    assert_eq!(store.get(&ClipboardFormat::PlainText), Some([].as_ref()));
    assert_eq!(store.total_bytes(), 0);
}

#[test]
fn store_get_nonexistent() {
    let store = ClipboardStore::new(1024);
    assert_eq!(store.get(&ClipboardFormat::Png), None);
}

#[test]
fn store_entry_count() {
    let mut store = ClipboardStore::new(1024);
    assert_eq!(store.entry_count(), 0);
    store
        .set(ClipboardFormat::PlainText, b"a".to_vec(), 1, 0)
        .unwrap();
    assert_eq!(store.entry_count(), 1);
    store
        .set(ClipboardFormat::Html, b"b".to_vec(), 1, 0)
        .unwrap();
    assert_eq!(store.entry_count(), 2);
    // Overwrite doesn't increase count
    store
        .set(ClipboardFormat::PlainText, b"c".to_vec(), 1, 0)
        .unwrap();
    assert_eq!(store.entry_count(), 2);
}

#[test]
fn store_get_entry() {
    let mut store = ClipboardStore::new(1024);
    store
        .set(ClipboardFormat::PlainText, b"hello".to_vec(), 42, 12345)
        .unwrap();
    let entry = store.get_entry(&ClipboardFormat::PlainText).unwrap();
    assert_eq!(entry.format, ClipboardFormat::PlainText);
    assert_eq!(entry.data, b"hello");
    assert_eq!(entry.timestamp_us, 12345);
}

#[test]
fn store_clear_preserves_serial() {
    let mut store = ClipboardStore::new(1024);
    store
        .set(ClipboardFormat::PlainText, b"a".to_vec(), 1, 0)
        .unwrap();
    store
        .set(ClipboardFormat::Html, b"b".to_vec(), 1, 0)
        .unwrap();
    let serial_before = store.serial();
    store.clear();
    assert_eq!(store.serial(), serial_before); // serial is not reset
    assert_eq!(store.entry_count(), 0);
}

#[test]
fn store_max_zero_rejects_any() {
    let mut store = ClipboardStore::new(0);
    let result = store.set(ClipboardFormat::PlainText, vec![1], 1, 0);
    assert!(result.is_err());
}

#[test]
fn store_max_zero_accepts_empty() {
    let mut store = ClipboardStore::new(0);
    // Empty data should fit even with max=0
    assert!(store.set(ClipboardFormat::PlainText, vec![], 1, 0).is_ok());
}

#[test]
fn store_multiple_formats_total_bytes() {
    let mut store = ClipboardStore::new(100);
    store
        .set(ClipboardFormat::PlainText, vec![0; 30], 1, 0)
        .unwrap();
    store.set(ClipboardFormat::Html, vec![0; 30], 1, 0).unwrap();
    store.set(ClipboardFormat::Png, vec![0; 30], 1, 0).unwrap();
    assert_eq!(store.total_bytes(), 90);
    // Adding 20 more should exceed 100
    let result = store.set(ClipboardFormat::Jpeg, vec![0; 20], 1, 0);
    assert!(result.is_err());
}

// --- Manager edge cases ---
#[test]
fn manager_get_remote() {
    let mut mgr = ClipboardManager::new(ClipboardPolicy::default());
    mgr.receive_remote_data(ClipboardFormat::PlainText, b"remote data".to_vec())
        .unwrap();
    assert_eq!(
        mgr.get_remote(&ClipboardFormat::PlainText),
        Some(b"remote data".as_ref())
    );
}

#[test]
fn manager_clear_local() {
    let mut mgr = ClipboardManager::new(ClipboardPolicy::default());
    let mut data = HashMap::new();
    data.insert(ClipboardFormat::PlainText, b"hello".to_vec());
    mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data)
        .unwrap();
    assert!(mgr.get_local(&ClipboardFormat::PlainText).is_some());
    mgr.clear_local();
    assert!(mgr.get_local(&ClipboardFormat::PlainText).is_none());
}

#[test]
fn manager_clear_remote() {
    let mut mgr = ClipboardManager::new(ClipboardPolicy::default());
    mgr.receive_remote_data(ClipboardFormat::PlainText, b"data".to_vec())
        .unwrap();
    assert!(mgr.get_remote(&ClipboardFormat::PlainText).is_some());
    mgr.clear_remote();
    assert!(mgr.get_remote(&ClipboardFormat::PlainText).is_none());
}

#[test]
fn manager_serial_increments() {
    let mut mgr = ClipboardManager::new(ClipboardPolicy::default());
    let s0 = mgr.serial();
    let mut data = HashMap::new();
    data.insert(ClipboardFormat::PlainText, b"a".to_vec());
    mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data)
        .unwrap();
    assert!(mgr.serial() > s0);
}

#[test]
fn manager_local_offer_empty_formats_after_filter() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 1024,
        allowed_formats: Some(vec![ClipboardFormat::Png]),
        bidirectional: true,
    };
    let mut mgr = ClipboardManager::new(policy);
    let data = HashMap::new();
    // Offer PlainText but only Png is allowed
    let result = mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data);
    assert!(result.is_err());
}

#[test]
fn manager_local_offer_data_too_large() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 10,
        allowed_formats: None,
        bidirectional: true,
    };
    let mut mgr = ClipboardManager::new(policy);
    let mut data = HashMap::new();
    data.insert(ClipboardFormat::PlainText, vec![0u8; 100]);
    let result = mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data);
    assert!(result.is_err());
}

#[test]
fn manager_request_blocked_format() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 1024,
        allowed_formats: Some(vec![ClipboardFormat::PlainText]),
        bidirectional: true,
    };
    let mut mgr = ClipboardManager::new(policy);
    let result = mgr.request_remote(ClipboardFormat::Png);
    assert!(result.is_err());
}

#[test]
fn manager_default_policy() {
    let policy = ClipboardPolicy::default();
    assert_eq!(policy.max_payload_bytes, 16 * 1024 * 1024);
    assert!(policy.allowed_formats.is_none());
    assert!(policy.bidirectional);
}

#[test]
fn manager_transfer_state() {
    let mut mgr = ClipboardManager::new(ClipboardPolicy::default());
    assert!(matches!(mgr.transfer_state(), TransferState::Idle));
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    mgr.handle_remote_offer(offer).unwrap();
    assert!(matches!(
        mgr.transfer_state(),
        TransferState::Offered { .. }
    ));
}

use std::collections::HashMap;

use crate::format::ClipboardFormat;
use crate::offer::ClipboardOffer;
use crate::manager::*;

fn permissive_policy() -> ClipboardPolicy {
    ClipboardPolicy {
        max_payload_bytes: 1024 * 1024,
        allowed_formats: None,
        bidirectional: true,
    }
}

#[test]
fn manager_local_offer() {
    let mut mgr = ClipboardManager::new(permissive_policy());
    let mut data = HashMap::new();
    data.insert(ClipboardFormat::PlainText, b"hello".to_vec());
    let offer = mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data).unwrap();
    assert_eq!(offer.formats, vec![ClipboardFormat::PlainText]);
    assert!(offer.serial > 0);
}

#[test]
fn manager_remote_offer() {
    let mut mgr = ClipboardManager::new(permissive_policy());
    let offer = ClipboardOffer::new(99, vec![ClipboardFormat::PlainText], 0, 1);
    assert!(mgr.handle_remote_offer(offer).is_ok());
}

#[test]
fn manager_policy_blocks_format() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 1024,
        allowed_formats: Some(vec![ClipboardFormat::PlainText]),
        bidirectional: true,
    };
    let mgr = ClipboardManager::new(policy);
    assert!(mgr.is_format_allowed(&ClipboardFormat::PlainText));
    assert!(!mgr.is_format_allowed(&ClipboardFormat::Png));
}

#[test]
fn manager_policy_max_size() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 10,
        allowed_formats: None,
        bidirectional: true,
    };
    let mut mgr = ClipboardManager::new(policy);
    let result = mgr.receive_remote_data(ClipboardFormat::PlainText, vec![0u8; 100]);
    assert!(result.is_err());
}

#[test]
fn manager_request_remote() {
    let mut mgr = ClipboardManager::new(permissive_policy());
    let req = mgr.request_remote(ClipboardFormat::Html).unwrap();
    assert_eq!(req.target_format, ClipboardFormat::Html);
}

#[test]
fn manager_receive_remote() {
    let mut mgr = ClipboardManager::new(permissive_policy());
    assert!(mgr.receive_remote_data(ClipboardFormat::PlainText, b"data".to_vec()).is_ok());
}

#[test]
fn manager_bidirectional_disabled() {
    let policy = ClipboardPolicy {
        max_payload_bytes: 1024,
        allowed_formats: None,
        bidirectional: false,
    };
    let mut mgr = ClipboardManager::new(policy);
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    assert!(mgr.handle_remote_offer(offer).is_err());
}

#[test]
fn manager_get_local() {
    let mut mgr = ClipboardManager::new(permissive_policy());
    let mut data = HashMap::new();
    data.insert(ClipboardFormat::PlainText, b"stored".to_vec());
    mgr.handle_local_offer(vec![ClipboardFormat::PlainText], data).unwrap();
    assert_eq!(mgr.get_local(&ClipboardFormat::PlainText), Some(b"stored".as_ref()));
}

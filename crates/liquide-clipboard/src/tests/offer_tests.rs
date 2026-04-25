use crate::format::ClipboardFormat;
use crate::offer::*;

#[test]
fn offer_create() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 1000, 1);
    assert_eq!(offer.source_id, 1);
    assert_eq!(offer.formats.len(), 1);
    assert_eq!(offer.timestamp_us, 1000);
    assert_eq!(offer.serial, 1);
}

#[test]
fn offer_has_format_true() {
    let offer = ClipboardOffer::new(
        1,
        vec![ClipboardFormat::PlainText, ClipboardFormat::Html],
        0,
        1,
    );
    assert!(offer.has_format(&ClipboardFormat::PlainText));
    assert!(offer.has_format(&ClipboardFormat::Html));
}

#[test]
fn offer_has_format_false() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
    assert!(!offer.has_format(&ClipboardFormat::Png));
}

#[test]
fn offer_preferred_text() {
    let offer = ClipboardOffer::new(
        1,
        vec![ClipboardFormat::Html, ClipboardFormat::PlainText],
        0,
        1,
    );
    // PlainText has higher priority than Html
    assert_eq!(
        offer.preferred_text_format(),
        Some(&ClipboardFormat::PlainText)
    );
}

#[test]
fn offer_preferred_image() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::Jpeg, ClipboardFormat::Png], 0, 1);
    // Png has higher priority than Jpeg
    assert_eq!(offer.preferred_image_format(), Some(&ClipboardFormat::Png));
}

#[test]
fn offer_no_preferred_text() {
    let offer = ClipboardOffer::new(1, vec![ClipboardFormat::Png], 0, 1);
    assert_eq!(offer.preferred_text_format(), None);
}

#[test]
fn request_create() {
    let req = ClipboardRequest::new(ClipboardFormat::PlainText, 42);
    assert_eq!(req.target_format, ClipboardFormat::PlainText);
    assert_eq!(req.serial, 42);
}

//! Tests for the clipboard manager crate.

use crate::entry::*;
use crate::history::ClipboardHistory;
use crate::manager::ClipboardManager;
use crate::persistence;
use crate::platform::NullClipboard;
use crate::platform::PlatformClipboard;
use crate::sensitive::SensitiveClipboardPolicy;
use crate::sync::{ClipboardSync, ClipboardSyncBackend, LocalSyncStub};

// -----------------------------------------------------------------------
// ClipboardContent tests
// -----------------------------------------------------------------------

#[test]
fn content_size_bytes_text() {
    let c = ClipboardContent::Text("hello".into());
    assert_eq!(c.size_bytes(), 5);
}

#[test]
fn content_size_bytes_rich_text() {
    let c = ClipboardContent::RichText {
        html: "<b>hi</b>".into(),
        plain_fallback: "hi".into(),
    };
    assert_eq!(c.size_bytes(), 9 + 2);
}

#[test]
fn content_size_bytes_image() {
    let c = ClipboardContent::Image {
        width: 2,
        height: 2,
        data: vec![0u8; 16],
        format: ImageFormat::Rgba32,
    };
    assert_eq!(c.size_bytes(), 16);
}

#[test]
fn content_size_bytes_file_paths() {
    let c = ClipboardContent::FilePaths(vec!["/a/b".into(), "/c".into()]);
    assert_eq!(c.size_bytes(), 4 + 2);
}

#[test]
fn content_size_bytes_color() {
    let c = ClipboardContent::Color {
        r: 255,
        g: 0,
        b: 128,
        a: 255,
    };
    assert_eq!(c.size_bytes(), 4);
}

#[test]
fn content_size_bytes_custom() {
    let c = ClipboardContent::Custom {
        mime_type: "application/x-foo".into(),
        data: vec![1, 2, 3],
    };
    assert_eq!(c.size_bytes(), 17 + 3);
}

#[test]
fn content_category_mapping() {
    assert_eq!(
        ClipboardContent::Text("".into()).category(),
        ContentCategory::Text
    );
    assert_eq!(
        ClipboardContent::RichText {
            html: "".into(),
            plain_fallback: "".into()
        }
        .category(),
        ContentCategory::Text
    );
    assert_eq!(
        ClipboardContent::Image {
            width: 1,
            height: 1,
            data: vec![0; 4],
            format: ImageFormat::Rgba32
        }
        .category(),
        ContentCategory::Images
    );
    assert_eq!(
        ClipboardContent::FilePaths(vec![]).category(),
        ContentCategory::Files
    );
    assert_eq!(
        ClipboardContent::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        }
        .category(),
        ContentCategory::Colors
    );
    assert_eq!(
        ClipboardContent::Custom {
            mime_type: "".into(),
            data: vec![]
        }
        .category(),
        ContentCategory::Other
    );
}

#[test]
fn content_searchable_text() {
    let t = ClipboardContent::Text("hello world".into());
    assert_eq!(t.as_searchable_text(), Some("hello world"));

    let rt = ClipboardContent::RichText {
        html: "<b>hi</b>".into(),
        plain_fallback: "hi".into(),
    };
    assert_eq!(rt.as_searchable_text(), Some("hi"));

    let img = ClipboardContent::Image {
        width: 1,
        height: 1,
        data: vec![0; 4],
        format: ImageFormat::Png,
    };
    assert_eq!(img.as_searchable_text(), None);
}

#[test]
fn content_eq_dedup() {
    let a = ClipboardContent::Text("same".into());
    let b = ClipboardContent::Text("same".into());
    let c = ClipboardContent::Text("different".into());
    assert!(a.content_eq(&b));
    assert!(!a.content_eq(&c));
}

#[test]
fn content_as_file_paths() {
    let fp = ClipboardContent::FilePaths(vec!["/a".into(), "/b".into()]);
    assert_eq!(fp.as_file_paths(), Some(&["/a".to_string(), "/b".to_string()][..]));
    let text = ClipboardContent::Text("not paths".into());
    assert!(text.as_file_paths().is_none());
}

// -----------------------------------------------------------------------
// ClipboardEntry preview & label tests
// -----------------------------------------------------------------------

#[test]
fn entry_text_preview_short() {
    let e = ClipboardEntry::new(1, ClipboardContent::Text("hello".into()), 0, None);
    assert_eq!(e.text_preview(100), "hello");
}

#[test]
fn entry_text_preview_truncated() {
    let e = ClipboardEntry::new(1, ClipboardContent::Text("hello world".into()), 0, None);
    let preview = e.text_preview(8);
    assert!(preview.len() <= 10); // 7 chars + ellipsis (3 bytes in UTF-8)
    assert!(preview.ends_with('\u{2026}'));
}

#[test]
fn entry_text_preview_newlines_sanitised() {
    let e = ClipboardEntry::new(1, ClipboardContent::Text("line1\nline2".into()), 0, None);
    let preview = e.text_preview(100);
    assert!(!preview.contains('\n'));
    assert!(preview.contains('\u{21b5}'));
}

#[test]
fn entry_image_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::Image {
            width: 800,
            height: 600,
            data: vec![0; 1920000],
            format: ImageFormat::Png,
        },
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert!(preview.contains("PNG"));
    assert!(preview.contains("800"));
    assert!(preview.contains("600"));
}

#[test]
fn entry_file_paths_preview_single() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::FilePaths(vec!["/home/user/document.txt".into()]),
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert_eq!(preview, "/home/user/document.txt");
}

#[test]
fn entry_file_paths_preview_multiple() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::FilePaths(vec!["/a".into(), "/b".into(), "/c".into()]),
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert!(preview.contains("/a"));
    assert!(preview.contains("+2 more"));
}

#[test]
fn entry_color_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::Color {
            r: 255,
            g: 128,
            b: 0,
            a: 255,
        },
        0,
        None,
    );
    assert_eq!(e.text_preview(100), "#ff8000ff");
}

#[test]
fn entry_custom_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::Custom {
            mime_type: "application/json".into(),
            data: vec![1, 2, 3, 4, 5],
        },
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert!(preview.contains("application/json"));
    assert!(preview.contains("5 bytes"));
}

#[test]
fn entry_content_type_labels() {
    let text = ClipboardEntry::new(1, ClipboardContent::Text("t".into()), 0, None);
    assert_eq!(text.content_type_label(), "Text");

    let rt = ClipboardEntry::new(
        2,
        ClipboardContent::RichText {
            html: "<b>h</b>".into(),
            plain_fallback: "h".into(),
        },
        0,
        None,
    );
    assert_eq!(rt.content_type_label(), "Rich Text");

    let img = ClipboardEntry::new(
        3,
        ClipboardContent::Image {
            width: 1,
            height: 1,
            data: vec![0; 4],
            format: ImageFormat::Rgba32,
        },
        0,
        None,
    );
    assert_eq!(img.content_type_label(), "Image");

    let fp = ClipboardEntry::new(
        4,
        ClipboardContent::FilePaths(vec!["/a".into()]),
        0,
        None,
    );
    assert_eq!(fp.content_type_label(), "Files");

    let col = ClipboardEntry::new(
        5,
        ClipboardContent::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        },
        0,
        None,
    );
    assert_eq!(col.content_type_label(), "Color");

    let custom = ClipboardEntry::new(
        6,
        ClipboardContent::Custom {
            mime_type: "image/svg+xml".into(),
            data: vec![],
        },
        0,
        None,
    );
    assert_eq!(custom.content_type_label(), "image/svg+xml");
}

#[test]
fn entry_sensitive_field_defaults_false() {
    let e = ClipboardEntry::new(1, ClipboardContent::Text("x".into()), 0, None);
    assert!(!e.sensitive);
}

// -----------------------------------------------------------------------
// ClipboardHistory tests
// -----------------------------------------------------------------------

fn make_text_entry(text: &str, ts: u64) -> ClipboardEntry {
    ClipboardEntry::new(0, ClipboardContent::Text(text.into()), ts, None)
}

#[test]
fn history_push_and_recent() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("first", 1));
    h.push(make_text_entry("second", 2));
    h.push(make_text_entry("third", 3));

    let recent = h.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(
        recent[0].content.as_searchable_text(),
        Some("third")
    );
    assert_eq!(
        recent[1].content.as_searchable_text(),
        Some("second")
    );
}

#[test]
fn history_dedup_refreshes_timestamp() {
    let mut h = ClipboardHistory::new();
    let id1 = h.push(make_text_entry("dup", 100)).unwrap();
    let id2 = h.push(make_text_entry("dup", 200)).unwrap();
    assert_eq!(id1, id2, "dedup should reuse same id");
    assert_eq!(h.len(), 1);
    assert_eq!(h.get(id1).unwrap().timestamp, 200);
}

#[test]
fn history_get_by_id() {
    let mut h = ClipboardHistory::new();
    let id = h.push(make_text_entry("findme", 1)).unwrap();
    assert!(h.get(id).is_some());
    assert!(h.get(9999).is_none());
}

#[test]
fn history_search_case_insensitive() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("Hello World", 1));
    h.push(make_text_entry("goodbye", 2));
    h.push(make_text_entry("HELLO again", 3));

    let results = h.search("hello");
    assert_eq!(results.len(), 2);
}

#[test]
fn history_search_empty_query() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("something", 1));
    assert!(h.search("").is_empty());
}

#[test]
fn history_pin_survives_clear() {
    let mut h = ClipboardHistory::new();
    let id = h.push(make_text_entry("keep me", 1)).unwrap();
    h.push(make_text_entry("trash", 2));
    h.pin(id);
    h.clear();
    assert_eq!(h.len(), 1);
    assert_eq!(h.get(id).unwrap().pinned, true);
}

#[test]
fn history_unpin() {
    let mut h = ClipboardHistory::new();
    let id = h.push(make_text_entry("toggle", 1)).unwrap();
    h.pin(id);
    assert!(h.get(id).unwrap().pinned);
    h.unpin(id);
    assert!(!h.get(id).unwrap().pinned);
}

#[test]
fn history_delete_even_pinned() {
    let mut h = ClipboardHistory::new();
    let id = h.push(make_text_entry("doomed", 1)).unwrap();
    h.pin(id);
    h.delete(id);
    assert!(h.get(id).is_none());
    assert_eq!(h.len(), 0);
}

#[test]
fn history_eviction_respects_max() {
    let mut h = ClipboardHistory::with_limits(3, 10_000_000);
    h.push(make_text_entry("a", 1));
    h.push(make_text_entry("b", 2));
    h.push(make_text_entry("c", 3));
    h.push(make_text_entry("d", 4));
    // "a" should have been evicted.
    assert_eq!(h.len(), 3);
    assert!(h.search("a").is_empty());
    assert_eq!(h.recent(3).len(), 3);
}

#[test]
fn history_eviction_keeps_pinned() {
    let mut h = ClipboardHistory::with_limits(2, 10_000_000);
    let id = h.push(make_text_entry("pinned", 1)).unwrap();
    h.pin(id);
    h.push(make_text_entry("b", 2));
    h.push(make_text_entry("c", 3));
    h.push(make_text_entry("d", 4));
    // Pinned entry should survive; only 2 unpinned kept.
    assert!(h.get(id).is_some());
    // Total: 1 pinned + 2 unpinned = 3.
    assert_eq!(h.len(), 3);
}

#[test]
fn history_rejects_oversized_entry() {
    let mut h = ClipboardHistory::with_limits(500, 10);
    let big = ClipboardContent::Text("this is longer than 10 bytes".into());
    let entry = ClipboardEntry::new(0, big, 1, None);
    assert!(h.push(entry).is_none());
    assert!(h.is_empty());
}

#[test]
fn history_latest() {
    let mut h = ClipboardHistory::new();
    assert!(h.latest().is_none());
    h.push(make_text_entry("first", 1));
    h.push(make_text_entry("second", 2));
    assert_eq!(
        h.latest().unwrap().content.as_searchable_text(),
        Some("second")
    );
}

#[test]
fn history_pinned_list() {
    let mut h = ClipboardHistory::new();
    let id1 = h.push(make_text_entry("a", 1)).unwrap();
    h.push(make_text_entry("b", 2));
    let id3 = h.push(make_text_entry("c", 3)).unwrap();
    h.pin(id1);
    h.pin(id3);
    let pinned = h.pinned();
    assert_eq!(pinned.len(), 2);
}

#[test]
fn history_filter_by_category() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("text", 1));
    h.push(ClipboardEntry::new(
        0,
        ClipboardContent::Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        },
        2,
        None,
    ));
    h.push(make_text_entry("more text", 3));

    let texts = h.filter_by_category(ContentCategory::Text);
    assert_eq!(texts.len(), 2);
    let colors = h.filter_by_category(ContentCategory::Colors);
    assert_eq!(colors.len(), 1);
    let images = h.filter_by_category(ContentCategory::Images);
    assert_eq!(images.len(), 0);
}

#[test]
fn history_merge_text() {
    let mut h = ClipboardHistory::new();
    let id1 = h.push(make_text_entry("alpha", 1)).unwrap();
    let id2 = h.push(make_text_entry("beta", 2)).unwrap();
    let id_color = h
        .push(ClipboardEntry::new(
            0,
            ClipboardContent::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            3,
            None,
        ))
        .unwrap();

    let merged = h.merge_text(&[id1, id2], ", ").unwrap();
    assert_eq!(merged, ClipboardContent::Text("alpha, beta".into()));

    // Non-text entries are skipped.
    let merged2 = h.merge_text(&[id1, id_color, id2], " ").unwrap();
    assert_eq!(merged2, ClipboardContent::Text("alpha beta".into()));

    // All unknown ids -> None.
    assert!(h.merge_text(&[9999], " ").is_none());
}

#[test]
fn history_set_max_entries_evicts() {
    let mut h = ClipboardHistory::new();
    for i in 0..10 {
        h.push(make_text_entry(&format!("entry{i}"), i as u64));
    }
    assert_eq!(h.len(), 10);
    h.set_max_entries(5);
    assert_eq!(h.len(), 5);
}

#[test]
fn history_search_all_matches_text() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("hello world", 1));
    h.push(make_text_entry("goodbye", 2));
    let results = h.search_all("hello");
    assert_eq!(results.len(), 1);
}

#[test]
fn history_search_all_matches_file_paths() {
    let mut h = ClipboardHistory::new();
    h.push(ClipboardEntry::new(
        0,
        ClipboardContent::FilePaths(vec!["/home/user/documents/report.pdf".into()]),
        1,
        None,
    ));
    h.push(make_text_entry("unrelated", 2));
    let results = h.search_all("report");
    assert_eq!(results.len(), 1);
}

#[test]
fn history_search_all_matches_color_hex() {
    let mut h = ClipboardHistory::new();
    h.push(ClipboardEntry::new(
        0,
        ClipboardContent::Color {
            r: 255,
            g: 0,
            b: 128,
            a: 255,
        },
        1,
        None,
    ));
    // The hex would be #ff0080ff
    let results = h.search_all("ff00");
    assert_eq!(results.len(), 1);
}

#[test]
fn history_search_all_matches_source_app() {
    let mut h = ClipboardHistory::new();
    h.push(ClipboardEntry::new(
        0,
        ClipboardContent::Text("secret".into()),
        1,
        Some("Firefox".into()),
    ));
    let results = h.search_all("firefox");
    assert_eq!(results.len(), 1);
}

#[test]
fn history_search_all_empty_query() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("anything", 1));
    assert!(h.search_all("").is_empty());
}

#[test]
fn history_expire_sensitive() {
    let mut h = ClipboardHistory::new();
    let id1 = h.push(make_text_entry("normal", 100)).unwrap();
    let id2 = h.push(make_text_entry("secret1", 100)).unwrap();
    if let Some(e) = h.get_mut(id2) {
        e.sensitive = true;
    }
    let id3 = h.push(make_text_entry("secret2", 200)).unwrap();
    if let Some(e) = h.get_mut(id3) {
        e.sensitive = true;
    }

    // Expire entries older than ts 150.
    let removed = h.expire_sensitive(150);
    assert_eq!(removed, 1); // secret1 (ts=100) expired
    assert!(h.get(id1).is_some()); // normal survived
    assert!(h.get(id2).is_none()); // expired
    assert!(h.get(id3).is_some()); // too recent
}

#[test]
fn history_expire_sensitive_preserves_pinned() {
    let mut h = ClipboardHistory::new();
    let id = h.push(make_text_entry("pinned-secret", 50)).unwrap();
    if let Some(e) = h.get_mut(id) {
        e.sensitive = true;
    }
    h.pin(id);

    let removed = h.expire_sensitive(200);
    assert_eq!(removed, 0);
    assert!(h.get(id).is_some());
}

#[test]
fn history_clear_sensitive() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("normal", 1));
    let id2 = h.push(make_text_entry("secret", 2)).unwrap();
    if let Some(e) = h.get_mut(id2) {
        e.sensitive = true;
    }
    h.push(make_text_entry("also normal", 3));

    let removed = h.clear_sensitive();
    assert_eq!(removed, 1);
    assert_eq!(h.len(), 2);
}

#[test]
fn history_iter() {
    let mut h = ClipboardHistory::new();
    h.push(make_text_entry("a", 1));
    h.push(make_text_entry("b", 2));
    h.push(make_text_entry("c", 3));

    let texts: Vec<_> = h
        .iter()
        .filter_map(|e| e.content.as_searchable_text())
        .collect();
    assert_eq!(texts, vec!["c", "b", "a"]);
}

// -----------------------------------------------------------------------
// ClipboardManager tests
// -----------------------------------------------------------------------

#[test]
fn manager_on_copy_and_paste_latest() {
    let mut mgr = ClipboardManager::new();
    mgr.on_copy(ClipboardContent::Text("hello".into()), Some("app1".into()));
    let content = mgr.paste_latest().unwrap();
    assert_eq!(*content, ClipboardContent::Text("hello".into()));
}

#[test]
fn manager_paste_increments_counter() {
    let mut mgr = ClipboardManager::new();
    let id = mgr
        .on_copy(ClipboardContent::Text("count me".into()), None)
        .unwrap();
    mgr.paste(id);
    mgr.paste(id);
    assert_eq!(mgr.history().get(id).unwrap().times_pasted, 2);
}

#[test]
fn manager_sensitive_mode() {
    let mut mgr = ClipboardManager::new();
    mgr.sensitive_mode = true;
    let result = mgr.on_copy(ClipboardContent::Text("secret".into()), None);
    assert!(result.is_none());
    assert!(mgr.history().is_empty());
}

#[test]
fn manager_category_filter() {
    let mut mgr = ClipboardManager::new();
    mgr.on_copy(ClipboardContent::Text("text".into()), None);
    mgr.on_copy(
        ClipboardContent::Color {
            r: 1,
            g: 2,
            b: 3,
            a: 4,
        },
        None,
    );
    mgr.on_copy(
        ClipboardContent::FilePaths(vec!["/tmp/f".into()]),
        None,
    );

    assert_eq!(mgr.category_filter(ContentCategory::Text).len(), 1);
    assert_eq!(mgr.category_filter(ContentCategory::Colors).len(), 1);
    assert_eq!(mgr.category_filter(ContentCategory::Files).len(), 1);
    assert_eq!(mgr.category_filter(ContentCategory::Images).len(), 0);
}

#[test]
fn manager_set_max_history() {
    let mut mgr = ClipboardManager::new();
    for i in 0..20 {
        mgr.on_copy(ClipboardContent::Text(format!("item{i}")), None);
    }
    mgr.set_max_history(5);
    assert_eq!(mgr.history().len(), 5);
}

#[test]
fn manager_merge_text() {
    let mut mgr = ClipboardManager::new();
    let id1 = mgr
        .on_copy(ClipboardContent::Text("foo".into()), None)
        .unwrap();
    let id2 = mgr
        .on_copy(ClipboardContent::Text("bar".into()), None)
        .unwrap();
    let merged = mgr.merge_text(&[id1, id2], "-").unwrap();
    assert_eq!(merged, ClipboardContent::Text("foo-bar".into()));
}

#[test]
fn manager_paste_unknown_id() {
    let mut mgr = ClipboardManager::new();
    assert!(mgr.paste(42).is_none());
}

#[test]
fn manager_paste_latest_empty() {
    let mut mgr = ClipboardManager::new();
    assert!(mgr.paste_latest().is_none());
}

#[test]
fn manager_history_pin_unpin_clear() {
    let mut mgr = ClipboardManager::new();
    let id = mgr
        .on_copy(ClipboardContent::Text("pin me".into()), None)
        .unwrap();
    mgr.on_copy(ClipboardContent::Text("ephemeral".into()), None);

    mgr.history_mut().pin(id);
    mgr.history_mut().clear();
    assert_eq!(mgr.history().len(), 1);
    assert!(mgr.history().get(id).is_some());
}

#[test]
fn manager_sensitive_policy_marks_app() {
    let mut mgr = ClipboardManager::new();
    mgr.sensitive_policy_mut().add_excluded_app("KeePassXC");

    let id = mgr
        .on_copy(ClipboardContent::Text("password123".into()), Some("KeePassXC".into()))
        .unwrap();
    assert!(mgr.history().get(id).unwrap().sensitive);
}

#[test]
fn manager_sensitive_policy_ignores_other_apps() {
    let mut mgr = ClipboardManager::new();
    mgr.sensitive_policy_mut().add_excluded_app("KeePassXC");

    let id = mgr
        .on_copy(ClipboardContent::Text("normal text".into()), Some("Firefox".into()))
        .unwrap();
    assert!(!mgr.history().get(id).unwrap().sensitive);
}

#[test]
fn manager_on_screen_lock_clears_sensitive() {
    let mut mgr = ClipboardManager::new();
    mgr.sensitive_policy_mut().add_excluded_app("bitwarden");
    mgr.sensitive_policy_mut().clear_on_lock = true;

    mgr.on_copy(
        ClipboardContent::Text("normal".into()),
        Some("terminal".into()),
    );
    mgr.on_copy(
        ClipboardContent::Text("secret".into()),
        Some("bitwarden".into()),
    );
    assert_eq!(mgr.history().len(), 2);

    let removed = mgr.on_screen_lock();
    assert_eq!(removed, 1);
    assert_eq!(mgr.history().len(), 1);
}

#[test]
fn manager_on_screen_lock_disabled() {
    let mut mgr = ClipboardManager::new();
    mgr.sensitive_policy_mut().clear_on_lock = false;
    mgr.sensitive_policy_mut().add_excluded_app("1password");

    mgr.on_copy(
        ClipboardContent::Text("secret".into()),
        Some("1password".into()),
    );
    let removed = mgr.on_screen_lock();
    assert_eq!(removed, 0);
    assert_eq!(mgr.history().len(), 1);
}

// -----------------------------------------------------------------------
// SensitiveClipboardPolicy tests
// -----------------------------------------------------------------------

#[test]
fn sensitive_policy_defaults() {
    let p = SensitiveClipboardPolicy::new();
    assert_eq!(p.auto_clear_timeout_secs, 30);
    assert!(p.clear_on_lock);
    assert!(p.excluded_apps().is_empty());
}

#[test]
fn sensitive_policy_disabled() {
    let p = SensitiveClipboardPolicy::disabled();
    assert_eq!(p.auto_clear_timeout_secs, 0);
    assert!(!p.clear_on_lock);
}

#[test]
fn sensitive_policy_add_remove_excluded_app() {
    let mut p = SensitiveClipboardPolicy::new();
    p.add_excluded_app("KeePassXC");
    p.add_excluded_app("Bitwarden");
    assert_eq!(p.excluded_apps().len(), 2);

    // Duplicate add is a no-op.
    p.add_excluded_app("keepassxc");
    assert_eq!(p.excluded_apps().len(), 2);

    p.remove_excluded_app("KeePassXC");
    assert_eq!(p.excluded_apps().len(), 1);
}

#[test]
fn sensitive_policy_should_mark_case_insensitive() {
    let mut p = SensitiveClipboardPolicy::new();
    p.add_excluded_app("KeePassXC");
    assert!(p.should_mark_sensitive("keepassxc"));
    assert!(p.should_mark_sensitive("KEEPASSXC"));
    assert!(!p.should_mark_sensitive("firefox"));
}

#[test]
fn sensitive_policy_is_expired() {
    let p = SensitiveClipboardPolicy::new(); // 30s timeout
    let mut e = ClipboardEntry::new(1, ClipboardContent::Text("x".into()), 100, None);
    e.sensitive = true;

    // Not yet expired at ts 120 (only 20s elapsed).
    assert!(!p.is_expired(&e, 120));
    // Expired at ts 130 (30s elapsed).
    assert!(p.is_expired(&e, 130));
    // Expired at ts 200 (100s elapsed).
    assert!(p.is_expired(&e, 200));
}

#[test]
fn sensitive_policy_non_sensitive_never_expires() {
    let p = SensitiveClipboardPolicy::new();
    let e = ClipboardEntry::new(1, ClipboardContent::Text("x".into()), 100, None);
    assert!(!e.sensitive);
    assert!(!p.is_expired(&e, 9999));
}

#[test]
fn sensitive_policy_zero_timeout_never_expires() {
    let mut p = SensitiveClipboardPolicy::new();
    p.auto_clear_timeout_secs = 0;
    let mut e = ClipboardEntry::new(1, ClipboardContent::Text("x".into()), 100, None);
    e.sensitive = true;
    assert!(!p.is_expired(&e, 9999));
}

#[test]
fn sensitive_policy_cutoff_timestamp() {
    let p = SensitiveClipboardPolicy::new(); // 30s
    assert_eq!(p.cutoff_timestamp(100), Some(70));
    assert_eq!(p.cutoff_timestamp(20), Some(0)); // saturating_sub

    let mut p2 = SensitiveClipboardPolicy::new();
    p2.auto_clear_timeout_secs = 0;
    assert_eq!(p2.cutoff_timestamp(100), None);
}

// -----------------------------------------------------------------------
// Sync tests
// -----------------------------------------------------------------------

#[test]
fn local_sync_stub_disabled_by_default() {
    let stub = LocalSyncStub::new();
    assert!(!stub.is_sync_enabled());
    assert!(!stub.is_connected());
}

#[test]
fn local_sync_stub_queue_while_disabled() {
    let mut stub = LocalSyncStub::new();
    let e = ClipboardEntry::new(1, ClipboardContent::Text("hi".into()), 0, None);
    stub.queue_outgoing(&e);
    assert!(stub.pending_outgoing().is_empty());
}

#[test]
fn local_sync_stub_queue_while_enabled() {
    let mut stub = LocalSyncStub::new();
    stub.set_sync_enabled(true);
    let e = ClipboardEntry::new(1, ClipboardContent::Text("hi".into()), 0, None);
    stub.queue_outgoing(&e);
    assert_eq!(stub.pending_outgoing().len(), 1);
}

#[test]
fn local_sync_stub_receive_while_disabled() {
    let mut stub = LocalSyncStub::new();
    stub.inject_incoming(ClipboardEntry::new(
        1,
        ClipboardContent::Text("hello".into()),
        0,
        None,
    ));
    let incoming = stub.receive_incoming();
    assert!(incoming.is_empty());
}

#[test]
fn local_sync_stub_receive_while_enabled() {
    let mut stub = LocalSyncStub::new();
    stub.set_sync_enabled(true);
    stub.inject_incoming(ClipboardEntry::new(
        1,
        ClipboardContent::Text("hello".into()),
        0,
        None,
    ));
    let incoming = stub.receive_incoming();
    assert_eq!(incoming.len(), 1);

    // Second call returns empty (consumed).
    let incoming2 = stub.receive_incoming();
    assert!(incoming2.is_empty());
}

#[test]
fn local_sync_stub_loopback() {
    let mut stub = LocalSyncStub::new();
    stub.set_sync_enabled(true);
    let e = ClipboardEntry::new(1, ClipboardContent::Text("round-trip".into()), 0, None);
    stub.queue_outgoing(&e);
    let looped = stub.loopback();
    assert_eq!(looped, 1);
    assert!(stub.pending_outgoing().is_empty());

    let incoming = stub.receive_incoming();
    assert_eq!(incoming.len(), 1);
    assert_eq!(
        incoming[0].content.as_searchable_text(),
        Some("round-trip")
    );
}

#[test]
fn clipboard_sync_coordinator() {
    let stub = LocalSyncStub::new();
    let mut sync = ClipboardSync::new(stub);
    assert!(!sync.is_connected());

    sync.backend_mut().set_sync_enabled(true);
    assert!(sync.is_connected());

    let e = ClipboardEntry::new(1, ClipboardContent::Text("sync me".into()), 0, None);
    sync.queue_outgoing(&e);
    assert_eq!(sync.backend().pending_outgoing().len(), 1);

    sync.backend_mut().loopback();
    let incoming = sync.receive_incoming();
    assert_eq!(incoming.len(), 1);
}

// -----------------------------------------------------------------------
// Persistence tests
// -----------------------------------------------------------------------

#[test]
fn persistence_roundtrip_text() {
    let entries = vec![
        ClipboardEntry::new(1, ClipboardContent::Text("hello".into()), 1000, Some("app".into())),
        ClipboardEntry::new(2, ClipboardContent::Text("world".into()), 2000, None),
    ];
    let mut buf = Vec::new();
    let written = persistence::save_entries(&entries, &mut buf).unwrap();
    assert_eq!(written, 2);

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content.as_searchable_text(), Some("hello"));
    assert_eq!(loaded[0].source_app.as_deref(), Some("app"));
    assert_eq!(loaded[1].content.as_searchable_text(), Some("world"));
    assert_eq!(loaded[1].timestamp, 2000);
}

#[test]
fn persistence_roundtrip_rich_text() {
    let entries = vec![ClipboardEntry::new(
        1,
        ClipboardContent::RichText {
            html: "<b>bold</b>".into(),
            plain_fallback: "bold".into(),
        },
        500,
        None,
    )];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert_eq!(loaded.len(), 1);
    match &loaded[0].content {
        ClipboardContent::RichText { html, plain_fallback } => {
            assert_eq!(html, "<b>bold</b>");
            assert_eq!(plain_fallback, "bold");
        }
        _ => panic!("expected RichText"),
    }
}

#[test]
fn persistence_roundtrip_image() {
    let entries = vec![ClipboardEntry::new(
        1,
        ClipboardContent::Image {
            width: 10,
            height: 20,
            data: vec![0xAB; 800],
            format: ImageFormat::Png,
        },
        100,
        None,
    )];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert_eq!(loaded.len(), 1);
    match &loaded[0].content {
        ClipboardContent::Image {
            width,
            height,
            data,
            format,
        } => {
            assert_eq!(*width, 10);
            assert_eq!(*height, 20);
            assert_eq!(data.len(), 800);
            assert_eq!(*format, ImageFormat::Png);
        }
        _ => panic!("expected Image"),
    }
}

#[test]
fn persistence_roundtrip_file_paths() {
    let entries = vec![ClipboardEntry::new(
        1,
        ClipboardContent::FilePaths(vec!["/a/b".into(), "/c/d".into()]),
        300,
        None,
    )];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert_eq!(loaded[0].content.as_file_paths().unwrap().len(), 2);
}

#[test]
fn persistence_roundtrip_color() {
    let entries = vec![ClipboardEntry::new(
        1,
        ClipboardContent::Color {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        },
        400,
        None,
    )];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    match &loaded[0].content {
        ClipboardContent::Color { r, g, b, a } => {
            assert_eq!((*r, *g, *b, *a), (10, 20, 30, 40));
        }
        _ => panic!("expected Color"),
    }
}

#[test]
fn persistence_roundtrip_custom() {
    let entries = vec![ClipboardEntry::new(
        1,
        ClipboardContent::Custom {
            mime_type: "application/octet-stream".into(),
            data: vec![1, 2, 3, 4, 5],
        },
        600,
        None,
    )];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    match &loaded[0].content {
        ClipboardContent::Custom { mime_type, data } => {
            assert_eq!(mime_type, "application/octet-stream");
            assert_eq!(data, &[1, 2, 3, 4, 5]);
        }
        _ => panic!("expected Custom"),
    }
}

#[test]
fn persistence_skips_sensitive_entries() {
    let mut sensitive_entry =
        ClipboardEntry::new(1, ClipboardContent::Text("secret".into()), 100, None);
    sensitive_entry.sensitive = true;
    let normal_entry =
        ClipboardEntry::new(2, ClipboardContent::Text("normal".into()), 200, None);

    let entries = vec![sensitive_entry, normal_entry];
    let mut buf = Vec::new();
    let written = persistence::save_entries(&entries, &mut buf).unwrap();
    assert_eq!(written, 1); // only the normal entry

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].content.as_searchable_text(), Some("normal"));
}

#[test]
fn persistence_skips_oversized_images() {
    let big_image = ClipboardEntry::new(
        1,
        ClipboardContent::Image {
            width: 1000,
            height: 1000,
            data: vec![0; 3_000_000], // > 2 MB limit
            format: ImageFormat::Rgba32,
        },
        100,
        None,
    );
    let entries = vec![big_image];
    let mut buf = Vec::new();
    let written = persistence::save_entries(&entries, &mut buf).unwrap();
    assert_eq!(written, 0);
}

#[test]
fn persistence_preserves_pinned_flag() {
    let mut entry = ClipboardEntry::new(1, ClipboardContent::Text("pinned".into()), 100, None);
    entry.pinned = true;
    entry.times_pasted = 42;

    let entries = vec![entry];
    let mut buf = Vec::new();
    persistence::save_entries(&entries, &mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let loaded = persistence::load_entries(&mut cursor).unwrap();
    assert!(loaded[0].pinned);
    assert_eq!(loaded[0].times_pasted, 42);
    assert!(!loaded[0].sensitive); // sensitive always false on load
}

#[test]
fn persistence_invalid_magic_header() {
    let bad_data = b"BADXsome garbage";
    let mut cursor = std::io::Cursor::new(bad_data.to_vec());
    let result = persistence::load_entries(&mut cursor);
    assert!(result.is_err());
}

#[test]
fn persistence_should_persist_logic() {
    let normal = ClipboardEntry::new(1, ClipboardContent::Text("ok".into()), 0, None);
    assert!(persistence::should_persist(&normal));

    let mut sensitive = ClipboardEntry::new(2, ClipboardContent::Text("secret".into()), 0, None);
    sensitive.sensitive = true;
    assert!(!persistence::should_persist(&sensitive));

    let big_img = ClipboardEntry::new(
        3,
        ClipboardContent::Image {
            width: 100,
            height: 100,
            data: vec![0; 3_000_000],
            format: ImageFormat::Rgba32,
        },
        0,
        None,
    );
    assert!(!persistence::should_persist(&big_img));

    let small_img = ClipboardEntry::new(
        4,
        ClipboardContent::Image {
            width: 10,
            height: 10,
            data: vec![0; 400],
            format: ImageFormat::Png,
        },
        0,
        None,
    );
    assert!(persistence::should_persist(&small_img));
}

#[test]
fn manager_save_and_load_history() {
    let mut mgr = ClipboardManager::new();
    mgr.on_copy(ClipboardContent::Text("item1".into()), None);
    mgr.on_copy(ClipboardContent::Text("item2".into()), None);

    let mut buf = Vec::new();
    let saved = mgr.save_history(&mut buf).unwrap();
    assert_eq!(saved, 2);

    let mut mgr2 = ClipboardManager::new();
    let mut cursor = std::io::Cursor::new(buf);
    let loaded = mgr2.load_history(&mut cursor).unwrap();
    assert_eq!(loaded, 2);
    assert_eq!(mgr2.history().len(), 2);
}

// -----------------------------------------------------------------------
// Platform bridge tests (NullClipboard)
// -----------------------------------------------------------------------

#[test]
fn null_clipboard_read_returns_error() {
    let cb = NullClipboard;
    assert!(cb.read().is_err());
}

#[test]
fn null_clipboard_write_succeeds() {
    let cb = NullClipboard;
    assert!(cb.write(&ClipboardContent::Text("test".into())).is_ok());
}

#[test]
fn null_clipboard_has_no_content() {
    let cb = NullClipboard;
    assert!(!cb.has_content());
}

// -----------------------------------------------------------------------
// Richtext entry preview tests
// -----------------------------------------------------------------------

#[test]
fn rich_text_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::RichText {
            html: "<p>Hello <b>world</b></p>".into(),
            plain_fallback: "Hello world".into(),
        },
        0,
        None,
    );
    assert_eq!(e.text_preview(100), "Hello world");
}

#[test]
fn entry_file_paths_preview_empty() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::FilePaths(vec![]),
        0,
        None,
    );
    assert_eq!(e.text_preview(100), "[no files]");
}

#[test]
fn entry_bmp_image_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::Image {
            width: 1920,
            height: 1080,
            data: vec![0; 100],
            format: ImageFormat::Bmp,
        },
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert!(preview.contains("BMP"));
    assert!(preview.contains("1920"));
    assert!(preview.contains("1080"));
}

#[test]
fn entry_rgba32_image_preview() {
    let e = ClipboardEntry::new(
        1,
        ClipboardContent::Image {
            width: 64,
            height: 64,
            data: vec![0; 64 * 64 * 4],
            format: ImageFormat::Rgba32,
        },
        0,
        None,
    );
    let preview = e.text_preview(100);
    assert!(preview.contains("RGBA"));
}

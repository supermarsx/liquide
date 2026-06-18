//! `<lq-file-embed>` real-pipeline gallery tests.
//!
//! These drive REAL `std::fs` metadata reads: a temp file written to the test's
//! temp dir (asserting the laid-out size/name reflect the real bytes) and a
//! missing path (asserting the graceful error state + inert affordances).
#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;

use crate::file_embed::{FileEmbed, FileState, DOWNLOAD_ACTION, OPEN_ACTION};
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;

const W: u32 = 460;
const H: u32 = 160;

fn as_fe<'a>(g: &'a Gallery, id: &str) -> &'a FileEmbed {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<FileEmbed>()
        .unwrap()
}

/// The concatenated text under a node (depth-first).
fn text_under(doc: &liquide_dom::Document, node: liquide_dom::NodeId) -> String {
    let mut out = String::new();
    fn rec(doc: &liquide_dom::Document, node: liquide_dom::NodeId, out: &mut String) {
        if let Some(t) = doc.get(node).and_then(|n| n.text_content()) {
            out.push_str(t);
        }
        for &c in doc.children(node) {
            rec(doc, c, out);
        }
    }
    rec(doc, node, &mut out);
    out
}

/// The text of the `data-part="sub"` line under the file embed.
fn sub_text(g: &Gallery, id: &str) -> String {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let sub = q.find_part(root, "sub").expect("sub node");
    text_under(g.doc(), sub)
}

/// Write `bytes` to a uniquely-named temp file and return its path. The caller is
/// responsible for cleanup (these tests remove it at the end).
fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = format!(
        "lq_file_embed_{}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        name
    );
    path.push(unique);
    let mut f = std::fs::File::create(&path).expect("create temp file");
    f.write_all(bytes).expect("write temp file");
    f.flush().expect("flush temp file");
    path
}

/// A real present file reports its REAL size + name via std::fs metadata.
#[test]
fn present_file_reads_real_metadata() {
    let bytes = vec![0u8; 4096]; // exactly 4 KiB
    let path = write_temp("report.pdf", &bytes);

    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    let fe = as_fe(&g, "fe");
    assert!(fe.is_present(), "the temp file must probe as present");
    assert_eq!(fe.size(), Some(4096), "size reflects the 4096 real bytes");
    assert_eq!(fe.state(), &FileState::Present { size: 4096 });
    // The display name is the file's own name.
    assert!(fe.display_name().ends_with("report.pdf"), "name from the path: {}", fe.display_name());
    // The type class is derived from the .pdf extension.
    assert_eq!(fe.type_class(), "pdf");

    let _ = std::fs::remove_file(&path);
}

/// The human size string reflects the real byte count in the laid-out sub line.
#[test]
fn size_string_reflects_real_bytes() {
    // 1536 bytes = 1.5 KB.
    let path = write_temp("data.bin", &vec![7u8; 1536]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    assert_eq!(sub_text(&g, "fe"), "1.5 KB");

    let _ = std::fs::remove_file(&path);
}

/// A MISSING path probes to a graceful error state (no panic) + inert affordances.
#[test]
fn missing_file_shows_error_state() {
    let mut path = std::env::temp_dir();
    path.push(format!("lq_definitely_missing_{}.zzz", std::process::id()));
    // Ensure it really does not exist.
    let _ = std::fs::remove_file(&path);

    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    let fe = as_fe(&g, "fe");
    assert!(!fe.is_present(), "missing file is not present");
    assert!(
        matches!(fe.state(), FileState::Error { .. }),
        "missing file resolves to a graceful Error state (got {:?})",
        fe.state()
    );
    assert_eq!(fe.size(), None);

    // The error message reaches the laid-out sub line.
    assert_eq!(sub_text(&g, "fe"), "File not found");
}

/// Clicking Open on a present file emits Action(open) with the path.
#[test]
fn open_affordance_emits_with_path() {
    let path = write_temp("clip.mp4", &vec![1u8; 64]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    let root = g.host.root_of("fe").unwrap();
    let open = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "open").expect("open box")
    };
    g.left_click(open.x + open.width / 2.0, open.y + open.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, OPEN_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some(path.to_string_lossy().as_ref()));

    let _ = std::fs::remove_file(&path);
}

/// Clicking Download emits Action(download); the Download box is hit-tested from
/// layout (distinct from Open — a constant could not separate the two zones).
#[test]
fn download_affordance_emits_from_its_own_box() {
    let path = write_temp("song.mp3", &vec![2u8; 64]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    let root = g.host.root_of("fe").unwrap();
    let (open, download) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "open").expect("open box"),
            q.box_of_part(root, "download").expect("download box"),
        )
    };
    // The two affordance boxes are genuinely distinct (no overlap on x).
    assert!(
        download.x >= open.right() - 1.0,
        "download box is right of open (open.right={}, download.x={})",
        open.right(),
        download.x
    );
    g.left_click(download.x + download.width / 2.0, download.y + download.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].name, DOWNLOAD_ACTION);

    let _ = std::fs::remove_file(&path);
}

/// An errored (missing) file's affordances are INERT — clicking Open emits nothing.
#[test]
fn errored_file_affordances_are_inert() {
    let mut path = std::env::temp_dir();
    path.push(format!("lq_missing_inert_{}.zzz", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();

    let root = g.host.root_of("fe").unwrap();
    let open = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "open").expect("open box still laid out (disabled)")
    };
    g.left_click(open.x + open.width / 2.0, open.y + open.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "a missing file's Open affordance is inert");
}

/// A directory path is classified as an error, not a present file (covers the
/// is_dir branch of the std::fs handling).
#[test]
fn directory_path_is_an_error() {
    let dir = std::env::temp_dir();
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&dir)));
    g.relayout();
    let fe = as_fe(&g, "fe");
    assert!(!fe.is_present(), "a directory is not a present file");
    assert!(matches!(fe.state(), FileState::Error { .. }), "directory → error");
}

/// The error state restyles pixels (the error tint vs. a present card): sample
/// inside the type-icon box, whose colour differs between a present file (type
/// colour) and the error state (red).
#[test]
fn error_state_changes_pixels() {
    let present = write_temp("ok.png", &vec![9u8; 128]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&present)));
    g.relayout();
    let icon = {
        let root = g.host.root_of("fe").unwrap();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "icon").expect("icon box")
    };
    let (sx, sy) = ((icon.x + icon.width / 2.0) as u32, (icon.y + icon.height / 2.0) as u32);
    let ok_px = Gallery::pixel(&g.rasterize(), sx, sy);

    let mut missing = std::env::temp_dir();
    missing.push(format!("lq_err_px_{}.zzz", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    g.mount("fe", Box::new(FileEmbed::probed(&missing)));
    g.relayout();
    let err_px = Gallery::pixel(&g.rasterize(), sx, sy);

    assert!(ok_px != err_px, "the error state must retint the icon");
    let _ = std::fs::remove_file(&present);
}

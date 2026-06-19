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

// ── Added: deep visual-STATE / styling coverage (no fake-green) ──────────────

/// Resolve a part box under the file-embed root.
fn fe_part(g: &Gallery, id: &str, part: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part} box"))
}

/// The type-class STATE ICON paints distinct colours for different file types:
/// a PDF (red) and a code file (green) icon must differ. Proves the per-type icon
/// styling lands in pixels (the icon is the state badge for the file kind).
#[test]
fn type_icon_colour_differs_across_file_types() {
    let pdf = write_temp("report.pdf", &vec![0u8; 64]);
    let mut g_pdf = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_pdf.mount("fe", Box::new(FileEmbed::probed(&pdf)));
    g_pdf.relayout();
    let i = fe_part(&g_pdf, "fe", "icon");
    let pdf_px = Gallery::pixel(&g_pdf.rasterize(), (i.x + i.width / 2.0) as u32, (i.y + i.height / 2.0) as u32);

    let code = write_temp("main.rs", &vec![0u8; 64]);
    let mut g_code = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_code.mount("fe", Box::new(FileEmbed::probed(&code)));
    g_code.relayout();
    let j = fe_part(&g_code, "fe", "icon");
    let code_px = Gallery::pixel(&g_code.rasterize(), (j.x + j.width / 2.0) as u32, (j.y + j.height / 2.0) as u32);

    assert!(
        pdf_px != code_px,
        "a PDF icon (red) and a code icon (green) must paint distinct colours (pdf {pdf_px:?} code {code_px:?})"
    );
    assert!(pdf_px.r > pdf_px.g, "pdf icon is red-dominant (got {pdf_px:?})");
    assert!(code_px.g > code_px.r, "code icon is green-dominant (got {code_px:?})");

    let _ = std::fs::remove_file(&pdf);
    let _ = std::fs::remove_file(&code);
}

/// The state icon ACTUALLY PAINTS (it is the file-kind badge, an opaque coloured
/// block, not an empty box).
#[test]
fn state_icon_paints() {
    let img = write_temp("pic.png", &vec![0u8; 64]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&img)));
    g.relayout();
    let i = fe_part(&g, "fe", "icon");
    let px = Gallery::pixel(&g.rasterize(), (i.x + i.width / 2.0) as u32, (i.y + i.height / 2.0) as u32);
    assert!(px.a > 0, "the state icon must paint (alpha {})", px.a);
    assert!(i.width >= 36.0 && i.height >= 36.0, "icon is CSS-sized 40px (got {}x{})", i.width, i.height);
    let _ = std::fs::remove_file(&img);
}

/// The ERROR state retints the whole card, not just the icon: the embed border +
/// background change (`lq-file-embed.error`) so a sample in the card body band
/// (away from the icon) differs between present and error.
#[test]
fn error_state_retints_card_body() {
    let present = write_temp("ok.txt", &vec![0u8; 64]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&present)));
    g.relayout();
    let root_box = {
        let node = g.host.root_of("fe").unwrap();
        g.box_of(node).unwrap()
    };
    // Sample on the card's top border line (the .error border colour change).
    let (sx, sy) = ((root_box.x + root_box.width / 2.0) as u32, root_box.y as u32);
    let present_px = Gallery::pixel(&g.rasterize(), sx, sy);

    let mut missing = std::env::temp_dir();
    missing.push(format!("lq_card_err_{}.zzz", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    g.mount("fe", Box::new(FileEmbed::probed(&missing)));
    g.relayout();
    let err_px = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        present_px != err_px,
        "the error state must retint the card border/background (present {present_px:?} error {err_px:?})"
    );
    let _ = std::fs::remove_file(&present);
}

/// The sub line is the human size (dim) when present and the error message (red,
/// `.error`) when errored — three distinct FileState renderings reach the sub line:
/// Present (size), Error (message), Unprobed ("—").
#[test]
fn sub_line_text_distinguishes_file_states() {
    // Present → human size.
    let present = write_temp("doc.pdf", &vec![0u8; 2048]); // 2.0 KB
    let mut g_present = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_present.mount("fe", Box::new(FileEmbed::probed(&present)));
    g_present.relayout();
    assert_eq!(sub_text(&g_present, "fe"), "2.0 KB");

    // Error → message.
    let mut missing = std::env::temp_dir();
    missing.push(format!("lq_sub_err_{}.zzz", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    let mut g_err = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_err.mount("fe", Box::new(FileEmbed::probed(&missing)));
    g_err.relayout();
    assert_eq!(sub_text(&g_err, "fe"), "File not found");

    // Unprobed → placeholder dash (no std::fs call made).
    let mut g_unprobed = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_unprobed.mount("fe", Box::new(FileEmbed::new(&present))); // NOT probed
    g_unprobed.relayout();
    assert_eq!(sub_text(&g_unprobed, "fe"), "—");

    let _ = std::fs::remove_file(&present);
}

/// The error sub line paints in a DISTINCT colour (red `.error`) vs a present sub
/// line (dim grey) — the `lq-file-sub.error` rule lands in pixels along the sub
/// text band.
#[test]
fn error_sub_line_colour_differs_from_present() {
    // Use a single-glyph-rich sub so the rasterizer has ink to colour.
    let present = write_temp("a.bin", &vec![0u8; 1536]); // "1.5 KB"
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&present)));
    g.relayout();
    let sub = fe_part(&g, "fe", "sub");
    let y = (sub.y + sub.height / 2.0) as u32;
    let scan = |fb: &liquide_compositor::framebuffer::FrameBuffer, r: &liquide_layout::geometry::Rect| -> Vec<liquide_compositor::pixel::Color> {
        ((r.x as u32 + 1)..((r.x + r.width) as u32).saturating_sub(1))
            .step_by(2)
            .map(|x| Gallery::pixel(fb, x, y))
            .collect()
    };
    let present_band = scan(&g.rasterize(), &sub);

    let mut missing = std::env::temp_dir();
    missing.push(format!("lq_sub_clr_{}.zzz", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    g.mount("fe", Box::new(FileEmbed::probed(&missing)));
    g.relayout();
    let err_sub = fe_part(&g, "fe", "sub");
    let err_band = scan(&g.rasterize(), &err_sub);
    assert!(
        present_band != err_band,
        "the error sub line (red) must paint differently from the present sub line (dim grey)"
    );
    let _ = std::fs::remove_file(&present);
}

/// :hover restyles a present file's Open affordance border (the dispatcher sets
/// the :hover pseudo on the hovered affordance node).
#[test]
fn open_affordance_hover_restyles_border() {
    let path = write_temp("clip.mp4", &vec![0u8; 64]);
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("fe", Box::new(FileEmbed::probed(&path)));
    g.relayout();
    let open = fe_part(&g, "fe", "open");
    let (bx, by) = ((open.x + open.width / 2.0) as u32, open.y as u32);
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    g.pointer_move(open.x + open.width / 2.0, open.y + open.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);
    assert!(
        before != after,
        ":hover must restyle the Open affordance border (before {before:?} after {after:?})"
    );
    let _ = std::fs::remove_file(&path);
}

/// An errored file's affordances render DIMMED (`:disabled`, opacity 0.35) vs a
/// present file's enabled affordances — the disabled styling lands in pixels.
#[test]
fn errored_affordances_render_dimmed() {
    // Same base file name in both cases so the meta column (flex-grow) has the same
    // width and the Open affordance lands at the same x in both states.
    let present = write_temp("ok.mp3", &vec![0u8; 64]);
    let mut g_ok = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_ok.mount("fe", Box::new(FileEmbed::probed(&present)));
    g_ok.relayout();
    let ok_open = fe_part(&g_ok, "fe", "open");
    let fb_ok = g_ok.rasterize();

    // Build a MISSING path with the identical file name (different parent dir).
    let mut missing = std::env::temp_dir();
    missing.push(format!("lq_aff_dim_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    missing.push("ok.mp3"); // never created → not found
    let mut g_err = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g_err.mount("fe", Box::new(FileEmbed::probed(&missing)));
    g_err.relayout();
    let err_open = fe_part(&g_err, "fe", "open");
    let fb_err = g_err.rasterize();

    // The disabled affordance is dimmed (opacity 0.35), so its bordered button box
    // composites toward the backdrop. Compare each Open box's OWN border-band
    // signature (sampled relative to its own box, so x-position differences from
    // the meta column don't matter): the enabled border must outweigh the dimmed
    // one. We sum the border-pixel intensity along each Open's top edge.
    let edge_intensity = |fb: &liquide_compositor::framebuffer::FrameBuffer, r: &liquide_layout::geometry::Rect| -> u64 {
        let mut sum = 0u64;
        let y = r.y as u32;
        for x in (r.x as u32 + 1)..((r.x + r.width) as u32 - 1) {
            let p = Gallery::pixel(fb, x, y);
            sum += p.r as u64 + p.g as u64 + p.b as u64;
        }
        for y2 in (r.y as u32 + 1)..((r.y + r.height) as u32 - 1) {
            let p = Gallery::pixel(fb, r.x as u32, y2);
            sum += p.r as u64 + p.g as u64 + p.b as u64;
        }
        sum
    };
    let ok_sum = edge_intensity(&fb_ok, &ok_open);
    let err_sum = edge_intensity(&fb_err, &err_open);
    assert!(
        ok_sum != err_sum,
        "the errored file's Open affordance must render with a different (dimmed) border intensity \
         than the present one (present {ok_sum}, errored {err_sum})"
    );
    let _ = std::fs::remove_file(&present);
}

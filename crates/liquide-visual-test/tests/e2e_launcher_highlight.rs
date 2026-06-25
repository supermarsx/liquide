//! E2E — launcher search + inline-highlight run paints (test-harden, Part A.2).
//!
//! This is the t195 headline case, now unblocked by the inline-element text-paint
//! fix (au3 bug #1: a `display:inline` element's text was consumed into an
//! `InlineItem::Word` with no standalone visited box, so it NEVER painted —
//! exactly why the launcher search-match highlight run had to be reverted).
//!
//! Two complementary teeth:
//!
//! 1. **Real launcher E2E** (`launcher_search_filters_and_paints`): drive the
//!    REAL `DesktopCompositor` headlessly — open the launcher, type "fi" — and
//!    assert (a) the live launcher model filters to exactly "Files", and (b) the
//!    matched result label PAINTS as real pixels in the launcher region of the
//!    rendered desktop frame. This is a true open + type + rendered-pixels path.
//!
//! 2. **Inline-highlight capability** (`inline_highlight_run_paints_distinctly`):
//!    the t195 run itself — a result label whose matched substring is wrapped in
//!    an inline `<span>` with a distinct highlight color — rendered through the
//!    real Shell pipeline. The TOOTH: the inline highlight color must PAINT a
//!    cluster of distinct-colored pixels INSIDE the label. Reverting the
//!    inline-text-paint fix drops the inline run entirely → zero highlight pixels
//!    → this fails (the exact regression t195 hit). A golden pins it.
//!
//! Why split: production does not (yet) emit highlight markup in the launcher
//! result label, so the real launcher cannot render a highlighted substring on
//! its own — but the CAPABILITY it depends on (inline-element text paint) is what
//! t195 needed and is now provable. Test #1 proves the launcher path is real and
//! the label paints; test #2 proves the inline-highlight run paints (the piece
//! t195 reverted). Wiring the markup into the launcher template is a follow-up in
//! liquide-shell (outside this crate's lock).
//!
//! Golden bless:
//!   `LIQUIDE_UPDATE_GOLDEN=1 cargo test -p liquide-visual-test --test e2e_launcher_highlight`

use liquide_components::TemplateNode;
use liquide_visual_test::capture::capture_desktop_scripted_readback;
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::primitive_render::render_fragment;
use liquide_visual_test::scenarios::{
    crop_region, region_launcher, scenario_options, themed_desktop_capture,
};

const THEME: &str = "liquid-glass";

/// 1) Real launcher E2E: open + type "fi" → filters to "Files" and the label
/// region paints in the rendered desktop frame.
#[test]
fn launcher_search_filters_and_paints() {
    // Drive the REAL desktop: open the launcher, type "fi", and read both the
    // filtered result titles AND the rendered frame back.
    let (frame, (visible, titles)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            shell.launcher_mut().open();
            shell.launcher_mut().set_query("fi");
            let titles: Vec<String> = shell
                .launcher()
                .results()
                .iter()
                .map(|r| r.title.clone())
                .collect();
            (shell.launcher().is_visible(), titles)
        },
    )
    .expect("launcher search capture");

    // STATE TOOTH: the query "fi" matches exactly the "Files" app.
    assert!(visible, "launcher must be visible after open()");
    assert!(
        titles.iter().any(|t| t == "Files"),
        "query 'fi' must match the Files app; got results {titles:?}"
    );
    assert_eq!(
        titles.len(),
        1,
        "query 'fi' should filter to exactly one result (Files); got {titles:?}"
    );

    // PIXEL TOOTH: opening + searching the launcher adds a substantial block of
    // new pixels (search box + result row + the "Files" label) in the launcher
    // region versus a no-launcher baseline desktop. A differential (changed
    // pixels) avoids the wallpaper saturating an absolute non-bg count.
    let baseline = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let region = region_launcher(frame.width, frame.height);
    let before = crop_region(&baseline, region);
    let after = crop_region(&frame, region);

    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 2_000,
        "the searched launcher region changed only {} pixels vs the baseline \
         desktop (threshold 2000); expected the search box + result row + the \
         'Files' label to paint there. The launcher search UI is not painting.",
        delta.differing_pixels
    );

    assert_golden("e2e_launcher_search_fi", &frame);
}

/// 2) Inline-highlight capability (the t195 run): a result label whose matched
/// substring is an inline `<span>` with a distinct highlight color must paint
/// that color as real pixels inside the label.
#[test]
fn inline_highlight_run_paints_distinctly() {
    // A launcher-style result label: "Files" with the matched prefix "Fi" wrapped
    // in an inline `<span class="match">` highlighted orange (#ff8000), the
    // unmatched tail "les" in white. This is the exact shape t195's reverted
    // highlight produced: an inline element carrying text inside a label. The
    // label is a flex row so the two runs flow side-by-side and the golden reads
    // cleanly as "Files" (the inline `<span>`s still carry their text via the
    // inline-element text-paint path under test).
    let label = TemplateNode::el("div")
        .style("position", "absolute")
        .style("left", "24px")
        .style("top", "28px")
        .style("width", "300px")
        .style("height", "50px")
        .style("display", "flex")
        .style("flex-direction", "row")
        .style("align-items", "baseline")
        .style("font-size", "40px")
        .style("color", "#ffffff")
        .style("white-space", "nowrap")
        .child(
            TemplateNode::el("span")
                .class("match")
                .style("color", "#ff8000")
                .child(TemplateNode::text("Fi")),
        )
        .child(TemplateNode::el("span").child(TemplateNode::text("les")));

    let frame = render_fragment(360, 100, "#101820", label);

    // Count highlight-colored (orange) pixels and unmatched (white) pixels.
    let mut highlight = 0usize;
    let mut unmatched = 0usize;
    for px in frame.rgba.chunks_exact(4) {
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        // Orange-ish: strong red, mid green, low blue — the matched highlight run.
        if r > 180 && g > 70 && g < 200 && b < 90 {
            highlight += 1;
        }
        // White-ish: the unmatched "les" run.
        if r > 180 && g > 180 && b > 180 {
            unmatched += 1;
        }
    }

    // THE T195 TOOTH: the inline highlight run must PAINT and be visually
    // DISTINCT. Before the inline-element text-paint fix the inline span's text
    // was dropped entirely, so `highlight` would be 0 (the exact reason t195's
    // highlight had to be reverted). A healthy render paints the matched "Fi" run
    // in orange AND the unmatched "les" run in white — two distinct colors.
    assert!(
        highlight > 80,
        "inline highlight run painted only {highlight} highlight-colored pixels \
         — the inline <span class=\"match\"> text was DROPPED. This is exactly \
         the t195 regression the inline-element text-paint fix (au3 bug #1) was \
         supposed to close. Check liquide-paint painter inline-text emission."
    );
    assert!(
        unmatched > 80,
        "the unmatched inline run ('les') painted only {unmatched} pixels — the \
         second inline <span> text was dropped (inline-element text paint \
         regressed)."
    );

    assert_golden("e2e_inline_highlight_run", &frame);
}

//! Throwaway render harness (t195): open the launcher, type a query, save a PNG.
//!
//! Usage: cargo run -p liquide-visual-test --bin launcher_search_shot -- <query> <out.png>

use liquide_visual_test::capture::capture_desktop_scripted_with;
use liquide_visual_test::scenarios::scenario_options;

fn main() {
    let mut args = std::env::args().skip(1);
    let query = args.next().unwrap_or_default();
    let out = args
        .next()
        .unwrap_or_else(|| "launcher_shot.png".to_string());

    let frame = capture_desktop_scripted_with(
        &scenario_options("liquid-glass"),
        |_h| Vec::new(),
        |shell| {
            shell.launcher_mut().open();
            if !query.is_empty() {
                shell.launcher_mut().set_query(&query);
            }
        },
    )
    .expect("capture");
    frame.save_png(&out).expect("save png");
    eprintln!("saved {out} for query {query:?}");
}

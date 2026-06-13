//! `snapshot` — render the current desktop to a PNG on demand (t56-f7).
//!
//! The fast eyeball-debug loop the user asked for: change CSS/code, run one
//! command, see a PNG of the headless desktop in seconds — no Win32 window, no
//! GPU, no live session. Uses the same deterministic capture path as the golden
//! tests, so what you see here is what the goldens assert against.
//!
//! Usage (directly, or via `./scripts/dev/dev.{ps1,sh} snapshot ...`):
//!
//! ```text
//!   snapshot                                    # 1280x720 liquid-glass -> PNG
//!   snapshot --theme night                      # night theme
//!   snapshot --width 800 --height 600           # custom size
//!   snapshot --scenario context_menu            # right-click context menu
//!   snapshot --scenario status_bar              # cropped status-bar band
//!   snapshot --out target/visual-test/my.png    # custom output path
//! ```
//!
//! Default output: `target/visual-test/snapshot.png` (absolute path printed).

use std::path::PathBuf;
use std::process::ExitCode;

use liquide_visual_test::scenarios::{
    SCENARIO_HEIGHT, SCENARIO_WIDTH, context_menu_capture, scenario_options, status_bar_capture,
    themed_desktop_capture,
};
use liquide_visual_test::{Frame, capture_desktop};

fn print_usage() {
    eprintln!(
        "snapshot — render the current desktop to a PNG\n\
         \n\
         Options:\n\
           --theme <name>       theme: liquid-glass | night | sunset | midday (default liquid-glass)\n\
           --width <px>         surface width  (default {SCENARIO_WIDTH})\n\
           --height <px>        surface height (default {SCENARIO_HEIGHT})\n\
           --scenario <name>    desktop | status_bar | context_menu (default desktop)\n\
           --out <path>         output PNG path (default target/visual-test/snapshot.png)\n\
           -h, --help           show this help"
    );
}

fn main() -> ExitCode {
    let mut theme = "liquid-glass".to_string();
    let mut width = SCENARIO_WIDTH;
    let mut height = SCENARIO_HEIGHT;
    let mut scenario = "desktop".to_string();
    let mut out: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => theme = next_val(&mut args, "--theme"),
            "--width" => width = next_val(&mut args, "--width").parse().unwrap_or(width),
            "--height" => height = next_val(&mut args, "--height").parse().unwrap_or(height),
            "--scenario" => scenario = next_val(&mut args, "--scenario"),
            "--out" => out = Some(PathBuf::from(next_val(&mut args, "--out"))),
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("snapshot: unknown argument '{other}'\n");
                print_usage();
                return ExitCode::FAILURE;
            }
        }
    }

    let frame = match render(&scenario, &theme, width, height) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("snapshot: capture failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    let out = out.unwrap_or_else(default_out_path);
    if let Err(e) = frame.save_png(&out) {
        eprintln!("snapshot: failed to write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }

    let abs = std::fs::canonicalize(&out).unwrap_or(out);
    println!(
        "snapshot: wrote {}x{} {} ({}) -> {}",
        frame.width,
        frame.height,
        scenario,
        theme,
        abs.display()
    );
    ExitCode::SUCCESS
}

fn render(
    scenario: &str,
    theme: &str,
    width: u32,
    height: u32,
) -> Result<Frame, liquide_visual_test::VisualTestError> {
    match scenario {
        "desktop" => {
            // Honour custom width/height for the plain desktop snapshot.
            if width == SCENARIO_WIDTH && height == SCENARIO_HEIGHT {
                themed_desktop_capture(theme)
            } else {
                let opts = scenario_options(theme).size(width, height);
                capture_desktop(&opts)
            }
        }
        "status_bar" => status_bar_capture(theme),
        "context_menu" => {
            // Right-click near the centre of the desktop area (below the bar).
            context_menu_capture(theme, (width / 2) as f32, (height / 2) as f32)
        }
        other => Err(liquide_visual_test::VisualTestError::Platform(format!(
            "unknown scenario '{other}' (expected desktop | status_bar | context_menu)"
        ))),
    }
}

fn default_out_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("visual-test")
        .join("snapshot.png")
}

fn next_val(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("snapshot: {flag} requires a value");
        std::process::exit(2);
    })
}

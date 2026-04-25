//! Reusable bootstrap glue for LiquiDE-built applications.
//!
//! This crate provides `AppBootstrap` — a single entry point that wires
//! together the subsystems every LiquiDE app needs:
//!
//! - A platform event loop (`liquide-platform`)
//! - A native top-level window
//! - UI theme + widget dispatch (`liquide-ui-core`)
//! - Input translation from native `MouseEvent`/`KeyEvent` into
//!   widget-level `Event`s
//! - A CPU paint/present tick loop using the platform's frame presenter
//!
//! Apps are expected to migrate their `main()` to:
//!
//! ```no_run
//! # use liquide_app_harness::AppBootstrap;
//! # use liquide_ui_core::widget::Widget;
//! # fn build_root() -> Box<dyn Widget> { unimplemented!() }
//! fn main() -> anyhow::Result<()> {
//!     AppBootstrap::new("com.liquide.apps.files", "Files")
//!         .run(|_cx| build_root())
//! }
//! ```
//!
//! # Intentional scope limits
//!
//! - Single top-level window only. [`AppCx::spawn_window`] is a stub.
//! - CPU paint-command pipeline only; real GPU wiring is reserved under
//!   the `real-gpu` feature flag.
//! - IME support is a feature toggle; key events still flow through
//!   even when enabled (a full `liquide-ime` bridge is deferred).
//! - Font manager is not constructed eagerly — apps that need a shared
//!   `liquide-fonts` handle should instantiate one themselves for now.

pub mod bootstrap;
pub mod event_loop;

pub use bootstrap::{AppBootstrap, AppCx, Size};
pub use event_loop::{AppRunReport, EventLoop, FrameCapture, FrameResult, FrameStats};

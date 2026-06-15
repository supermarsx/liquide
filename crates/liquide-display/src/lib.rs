//! Display/output management for LiquiDE.
//!
//! Provides monitor enumeration, resolution control, multi-monitor arrangement,
//! display profiles, wallpaper management, DPMS power management, ICC color
//! profiles, and night light (blue-light filter) support.

pub mod arrangement;
pub mod color_profile;
pub mod display;
pub mod dpms;
pub mod layout;
pub mod night_light;
pub mod output_profile;
pub mod platform;
pub mod profile;
pub mod wallpaper;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use arrangement::{
    ArrangementPolicy, DisplayArrangement, GapInfo, MonitorArrangement, MonitorPosition,
    auto_arrange, auto_arrange_default, fix_gaps, primary_monitor, snap_to_grid,
};
pub use color_profile::{ColorProfile, ColorSpace, IccProfileStore};
pub use display::{DisplayId, DisplayInfo, Resolution, Rotation};
pub use dpms::{DpmsController, DpmsPolicy, DpmsState};
pub use layout::{DesktopLayout, Rect as LayoutRect, WorkAreaInsets};
pub use night_light::{NightLight, NightLightSchedule, color_temperature_matrix};
pub use output_profile::{
    OutputProfile, ProfileStore, builtin_docked, builtin_laptop_only, builtin_presentation,
};
pub use platform::{PlatformError, enumerate_displays};
pub use profile::{DisplayConfig, DisplayProfile, detect_matching_profile};
pub use wallpaper::{
    SlideshowConfig, SlideshowOrder, WallpaperConfig, WallpaperMode, WallpaperTransform,
    compute_span_crop, compute_wallpaper_transform,
};

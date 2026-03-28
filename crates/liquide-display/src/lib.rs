//! Display/output management for LiquiDE.
//!
//! Provides monitor enumeration, resolution control, multi-monitor arrangement,
//! display profiles, wallpaper management, DPMS power management, ICC color
//! profiles, and night light (blue-light filter) support.

pub mod arrangement;
pub mod color_profile;
pub mod display;
pub mod dpms;
pub mod night_light;
pub mod output_profile;
pub mod platform;
pub mod profile;
pub mod wallpaper;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use arrangement::{
    auto_arrange, auto_arrange_default, fix_gaps, primary_monitor, snap_to_grid,
    ArrangementPolicy, DisplayArrangement, GapInfo, MonitorArrangement, MonitorPosition,
};
pub use color_profile::{ColorProfile, ColorSpace, IccProfileStore};
pub use display::{DisplayId, DisplayInfo, Resolution, Rotation};
pub use dpms::{DpmsController, DpmsPolicy, DpmsState};
pub use night_light::{color_temperature_matrix, NightLight, NightLightSchedule};
pub use output_profile::{
    builtin_docked, builtin_laptop_only, builtin_presentation, OutputProfile, ProfileStore,
};
pub use platform::{enumerate_displays, PlatformError};
pub use profile::{detect_matching_profile, DisplayConfig, DisplayProfile};
pub use wallpaper::{
    compute_span_crop, compute_wallpaper_transform, SlideshowConfig, SlideshowOrder,
    WallpaperConfig, WallpaperMode, WallpaperTransform,
};

//! Platform-specific accessibility preference detection.
//!
//! Each platform reads OS settings through native APIs:
//! - **Windows**: `SystemParametersInfoW` + registry
//! - **Linux**: `gsettings` + environment variables
//! - **macOS**: `defaults read`
//!
//! On unsupported platforms, [`detect()`] returns
//! [`AccessibilityPreferences::default()`].

use crate::prefs::{AccessibilityPreferences, CursorSize};

/// Detect the current accessibility preferences from the operating system.
///
/// This function is safe to call from any thread. On platforms where detection
/// requires spawning child processes (`gsettings`, `defaults`), it will block
/// briefly while those complete.
#[must_use]
pub fn detect() -> AccessibilityPreferences {
    let mut prefs = AccessibilityPreferences::default();
    detect_platform(&mut prefs);
    prefs
}

// ── Windows ─────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn detect_platform(prefs: &mut AccessibilityPreferences) {
    detect_windows(prefs);
}

#[cfg(target_os = "windows")]
fn detect_windows(prefs: &mut AccessibilityPreferences) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, SPI_GETSCREENREADER, SystemParametersInfoW,
    };

    // --- High contrast ---
    // HIGHCONTRASTW struct layout:
    //   cbSize: u32 (4 bytes)
    //   dwFlags: u32 (4 bytes)
    //   lpszDefaultScheme: *mut u16 (pointer-sized)
    // We only need dwFlags, so we use a raw buffer.
    const HCF_HIGHCONTRASTON: u32 = 0x0000_0001;
    // Size of HIGHCONTRASTW: 4 + 4 + pointer
    let hc_size = 4u32 + 4 + std::mem::size_of::<*mut u16>() as u32;
    let mut hc_buf = vec![0u8; hc_size as usize];
    // Write cbSize into the first 4 bytes (little-endian).
    hc_buf[..4].copy_from_slice(&hc_size.to_le_bytes());

    let ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc_size, hc_buf.as_mut_ptr().cast(), 0)
    };
    if ok != 0 {
        let flags = u32::from_le_bytes([hc_buf[4], hc_buf[5], hc_buf[6], hc_buf[7]]);
        prefs.high_contrast = (flags & HCF_HIGHCONTRASTON) != 0;
    }

    // --- Client-area animation (inverse of reduced-motion) ---
    let mut animation_enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            std::ptr::addr_of_mut!(animation_enabled).cast(),
            0,
        )
    };
    if ok != 0 {
        prefs.reduced_motion = animation_enabled == 0;
    }

    // --- Screen reader ---
    let mut reader_active: i32 = 0;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETSCREENREADER,
            0,
            std::ptr::addr_of_mut!(reader_active).cast(),
            0,
        )
    };
    if ok != 0 {
        prefs.screen_reader_active = reader_active != 0;
    }

    // --- Cursor size from registry ---
    // HKEY_CURRENT_USER\Control Panel\Cursors  -> CursorBaseSize (REG_DWORD)
    detect_windows_cursor_size(prefs);

    // --- Text scale factor from registry ---
    // HKEY_CURRENT_USER\SOFTWARE\Microsoft\Accessibility -> TextScaleFactor (REG_DWORD, percentage)
    detect_windows_text_scale(prefs);

    // --- Keyboard accessibility from registry ---
    detect_windows_keyboard_a11y(prefs);

    // High contrast implies increased contrast.
    if prefs.high_contrast {
        prefs.increase_contrast = true;
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_cursor_size(prefs: &mut AccessibilityPreferences) {
    // Read CursorBaseSize via `reg query` — avoids pulling in the full
    // registry crate. The value is a DWORD (pixel size, default 32).
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Control Panel\Cursors",
            "/v",
            "CursorBaseSize",
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            // Output line looks like:
            //     CursorBaseSize    REG_DWORD    0x20
            for line in text.lines() {
                if line.contains("CursorBaseSize") {
                    if let Some(hex) = line.split_whitespace().last() {
                        let val = if let Some(stripped) = hex.strip_prefix("0x") {
                            u32::from_str_radix(stripped, 16).unwrap_or(32)
                        } else {
                            hex.parse::<u32>().unwrap_or(32)
                        };
                        prefs.cursor_size = CursorSize::from_pixels(val);
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_text_scale(prefs: &mut AccessibilityPreferences) {
    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\SOFTWARE\Microsoft\Accessibility",
            "/v",
            "TextScaleFactor",
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains("TextScaleFactor") {
                    if let Some(val_str) = line.split_whitespace().last() {
                        let pct = if let Some(stripped) = val_str.strip_prefix("0x") {
                            u32::from_str_radix(stripped, 16).unwrap_or(100)
                        } else {
                            val_str.parse::<u32>().unwrap_or(100)
                        };
                        prefs.text_scale_factor = pct as f32 / 100.0;
                        prefs.large_text = pct > 120;
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_keyboard_a11y(prefs: &mut AccessibilityPreferences) {
    // StickyKeys: HKCU\Control Panel\Accessibility\StickyKeys -> Flags
    //   Bit 0 (0x01) = SKF_STICKYKEYSON
    read_a11y_flags(
        r"HKCU\Control Panel\Accessibility\StickyKeys",
        "Flags",
        0x01,
        &mut prefs.sticky_keys,
    );

    // FilterKeys (covers slow keys + bounce keys): Flags bit 0 = FKF_FILTERKEYSON
    let mut filter_keys_on = false;
    read_a11y_flags(
        r"HKCU\Control Panel\Accessibility\Keyboard Response",
        "Flags",
        0x01,
        &mut filter_keys_on,
    );
    if filter_keys_on {
        // FilterKeys enables either slow keys or bounce keys depending on config.
        // We report both as enabled when FilterKeys is on.
        prefs.slow_keys = true;
        prefs.bounce_keys = true;
    }
}

#[cfg(target_os = "windows")]
fn read_a11y_flags(subkey: &str, value_name: &str, mask: u32, out: &mut bool) {
    let output = std::process::Command::new("reg")
        .args(["query", subkey, "/v", value_name])
        .output();

    if let Ok(out_result) = output {
        if out_result.status.success() {
            let text = String::from_utf8_lossy(&out_result.stdout);
            for line in text.lines() {
                if line.contains(value_name) {
                    if let Some(val_str) = line.split_whitespace().last() {
                        let val = val_str.parse::<u32>().unwrap_or(0);
                        *out = (val & mask) != 0;
                    }
                }
            }
        }
    }
}

// ── Linux ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn detect_platform(prefs: &mut AccessibilityPreferences) {
    detect_linux(prefs);
}

#[cfg(target_os = "linux")]
fn detect_linux(prefs: &mut AccessibilityPreferences) {
    // --- High contrast via gsettings ---
    // org.gnome.desktop.a11y.interface high-contrast (boolean)
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.a11y.interface", "high-contrast") {
        prefs.high_contrast = val;
        if val {
            prefs.increase_contrast = true;
        }
    }
    // Fallback: check GTK theme name for "HighContrast"
    if !prefs.high_contrast {
        if let Some(theme) = gsettings_get_string("org.gnome.desktop.interface", "gtk-theme") {
            if theme.contains("HighContrast") {
                prefs.high_contrast = true;
                prefs.increase_contrast = true;
            }
        }
    }

    // --- Text scaling factor ---
    if let Some(factor) = gsettings_get_double("org.gnome.desktop.interface", "text-scaling-factor")
    {
        prefs.text_scale_factor = factor as f32;
        prefs.large_text = factor > 1.2;
    }

    // --- Cursor size ---
    if let Some(size) = gsettings_get_int("org.gnome.desktop.interface", "cursor-size") {
        prefs.cursor_size = CursorSize::from_pixels(size as u32);
    }

    // --- Reduced motion (env var or gsettings) ---
    if std::env::var("PREFERS_REDUCED_MOTION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        prefs.reduced_motion = true;
    }
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.interface", "enable-animations") {
        if !val {
            prefs.reduced_motion = true;
        }
    }

    // --- Screen reader ---
    if let Some(val) = gsettings_get_bool(
        "org.gnome.desktop.a11y.applications",
        "screen-reader-enabled",
    ) {
        prefs.screen_reader_active = val;
    }

    // --- Keyboard a11y ---
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.a11y.keyboard", "stickykeys-enable") {
        prefs.sticky_keys = val;
    }
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.a11y.keyboard", "slowkeys-enable") {
        prefs.slow_keys = val;
    }
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.a11y.keyboard", "bouncekeys-enable") {
        prefs.bounce_keys = val;
    }

    // --- Reduced transparency (GNOME 42+) ---
    if let Some(val) = gsettings_get_bool("org.gnome.desktop.a11y.interface", "reduce-transparency")
    {
        prefs.reduced_transparency = val;
    }
}

#[cfg(target_os = "linux")]
fn gsettings_get_string(schema: &str, key: &str) -> Option<String> {
    let output = std::process::Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // gsettings wraps strings in single quotes: 'Adwaita'
    Some(raw.trim_matches('\'').to_string())
}

#[cfg(target_os = "linux")]
fn gsettings_get_bool(schema: &str, key: &str) -> Option<bool> {
    let val = gsettings_get_string(schema, key)?;
    match val.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn gsettings_get_double(schema: &str, key: &str) -> Option<f64> {
    let val = gsettings_get_string(schema, key)?;
    val.parse::<f64>().ok()
}

#[cfg(target_os = "linux")]
fn gsettings_get_int(schema: &str, key: &str) -> Option<i64> {
    let val = gsettings_get_string(schema, key)?;
    val.parse::<i64>().ok()
}

// ── macOS ───────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn detect_platform(prefs: &mut AccessibilityPreferences) {
    detect_macos(prefs);
}

#[cfg(target_os = "macos")]
fn detect_macos(prefs: &mut AccessibilityPreferences) {
    // --- Reduced motion ---
    if defaults_read_bool("com.apple.universalaccess", "reduceMotion")
        .or_else(|| defaults_read_bool("NSGlobalDomain", "AppleReduceMotion"))
        .unwrap_or(false)
    {
        prefs.reduced_motion = true;
    }

    // --- Reduced transparency ---
    if defaults_read_bool("com.apple.universalaccess", "reduceTransparency")
        .or_else(|| defaults_read_bool("NSGlobalDomain", "AppleReduceTransparency"))
        .unwrap_or(false)
    {
        prefs.reduced_transparency = true;
    }

    // --- Increase contrast ---
    if defaults_read_bool("com.apple.universalaccess", "increaseContrast").unwrap_or(false) {
        prefs.increase_contrast = true;
    }

    // --- Inverted colors ---
    if defaults_read_bool("com.apple.universalaccess", "whiteOnBlack").unwrap_or(false) {
        prefs.inverted_colors = true;
    }

    // --- High contrast (macOS combines increase-contrast + specific themes) ---
    if prefs.increase_contrast {
        prefs.high_contrast = true;
    }

    // --- Screen reader (VoiceOver) ---
    if defaults_read_bool("com.apple.universalaccess", "voiceOverOnOffKey").unwrap_or(false) {
        prefs.screen_reader_active = true;
    }

    // --- Sticky keys ---
    if defaults_read_bool("com.apple.universalaccess", "stickyKey").unwrap_or(false) {
        prefs.sticky_keys = true;
    }

    // --- Slow keys ---
    if defaults_read_bool("com.apple.universalaccess", "slowKey").unwrap_or(false) {
        prefs.slow_keys = true;
    }

    // --- Cursor size ---
    if let Some(size) = defaults_read_float("com.apple.universalaccess", "mouseDriverCursorSize") {
        // macOS cursor size is a float multiplier (1.0 = normal, up to 4.0).
        let px = (size * 32.0) as u32;
        prefs.cursor_size = CursorSize::from_pixels(px);
    }

    // --- Text scale (macOS doesn't have a single global text scale, but
    //     we can check the sidebar icon size as a rough proxy) ---
    if let Some(size) = defaults_read_int("NSGlobalDomain", "AppleSideBarIconSize") {
        // 1 = small, 2 = medium (default), 3 = large
        match size {
            3 => {
                prefs.text_scale_factor = 1.25;
                prefs.large_text = true;
            }
            1 => {
                prefs.text_scale_factor = 0.85;
            }
            _ => {} // default is 1.0
        }
    }
}

#[cfg(target_os = "macos")]
fn defaults_read_bool(domain: &str, key: &str) -> Option<bool> {
    let output = std::process::Command::new("defaults")
        .args(["read", domain, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match val.as_str() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn defaults_read_float(domain: &str, key: &str) -> Option<f64> {
    let output = std::process::Command::new("defaults")
        .args(["read", domain, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    val.parse::<f64>().ok()
}

#[cfg(target_os = "macos")]
fn defaults_read_int(domain: &str, key: &str) -> Option<i64> {
    let output = std::process::Command::new("defaults")
        .args(["read", domain, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    val.parse::<i64>().ok()
}

// ── Fallback (unsupported platforms) ────────────────────────────────────

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn detect_platform(_prefs: &mut AccessibilityPreferences) {
    // No platform-specific detection available — defaults are fine.
    tracing::debug!("accessibility preference detection not available on this platform");
}

use crate::error::TimeError;

/// Simple NTP time-check interface.
///
/// This does not implement a full NTP client. Instead it uses platform-specific
/// commands to query NTP synchronization status and time offset from the system.
pub struct NtpSync;

impl NtpSync {
    /// Probe the system's NTP offset using platform commands.
    ///
    /// Returns the estimated offset in milliseconds (positive means the system
    /// clock is ahead of NTP time).
    ///
    /// - **Linux**: runs `timedatectl timesync-status` and parses the Offset line.
    /// - **Windows**: runs `w32tm /query /status` and parses the Phase Offset.
    /// - **macOS**: runs `sntp -d pool.ntp.org` and parses the offset.
    ///
    /// Returns `Err` if the platform is unsupported or the command fails.
    pub fn time_offset_from_system() -> Result<i64, TimeError> {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_ntp_offset();
        }
        #[cfg(target_os = "windows")]
        {
            return Self::windows_ntp_offset();
        }
        #[cfg(target_os = "macos")]
        {
            return Self::macos_ntp_offset();
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(TimeError::PlatformError(
                "unsupported platform for NTP query".into(),
            ))
        }
    }

    /// Check whether automatic time synchronization is enabled.
    ///
    /// - **Linux**: parses `timedatectl show --property=NTP --value`.
    /// - **Windows**: checks `w32tm /query /status` for running status.
    /// - **macOS**: checks `systemsetup -getusingnetworktime`.
    pub fn is_auto_sync_enabled() -> bool {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_ntp_enabled();
        }
        #[cfg(target_os = "windows")]
        {
            return Self::windows_ntp_enabled();
        }
        #[cfg(target_os = "macos")]
        {
            return Self::macos_ntp_enabled();
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            false
        }
    }

    // ---- Linux ----

    #[cfg(target_os = "linux")]
    fn linux_ntp_offset() -> Result<i64, TimeError> {
        let output = std::process::Command::new("timedatectl")
            .args(["timesync-status"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("timedatectl failed: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout);
        // Look for a line like "Offset: +12.345ms" or "Offset: -0.123s"
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Offset:") {
                let val = trimmed.trim_start_matches("Offset:").trim();
                return parse_ntp_offset_value(val);
            }
        }
        Err(TimeError::PlatformError(
            "could not parse timedatectl offset".into(),
        ))
    }

    #[cfg(target_os = "linux")]
    fn linux_ntp_enabled() -> bool {
        let output = std::process::Command::new("timedatectl")
            .args(["show", "--property=NTP", "--value"])
            .output();
        match output {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                text.trim().eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        }
    }

    // ---- Windows ----

    #[cfg(target_os = "windows")]
    fn windows_ntp_offset() -> Result<i64, TimeError> {
        let output = std::process::Command::new("w32tm")
            .args(["/query", "/status"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("w32tm failed: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout);
        // Look for "Phase Offset: 0.0012345s"
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Phase Offset:") || trimmed.starts_with("Phase offset:") {
                let val = trimmed.split(':').nth(1).unwrap_or("").trim();
                return parse_ntp_offset_value(val);
            }
        }
        Err(TimeError::PlatformError(
            "could not parse w32tm offset".into(),
        ))
    }

    #[cfg(target_os = "windows")]
    fn windows_ntp_enabled() -> bool {
        let output = std::process::Command::new("w32tm")
            .args(["/query", "/status"])
            .output();
        match output {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                // If the command succeeds and mentions "Leap Indicator" or "Source",
                // the service is running.
                text.contains("Source:") || text.contains("source:")
            }
            Err(_) => false,
        }
    }

    // ---- macOS ----

    #[cfg(target_os = "macos")]
    fn macos_ntp_offset() -> Result<i64, TimeError> {
        let output = std::process::Command::new("sntp")
            .args(["-d", "pool.ntp.org"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("sntp failed: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout);
        // sntp output includes lines like "+0.012345 +/- 0.001234"
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('+') || trimmed.starts_with('-') {
                // First token is the offset in seconds
                if let Some(secs_str) = trimmed.split_whitespace().next() {
                    if let Ok(secs) = secs_str.parse::<f64>() {
                        return Ok((secs * 1000.0) as i64);
                    }
                }
            }
        }
        Err(TimeError::PlatformError(
            "could not parse sntp offset".into(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn macos_ntp_enabled() -> bool {
        let output = std::process::Command::new("systemsetup")
            .args(["-getusingnetworktime"])
            .output();
        match output {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                text.to_ascii_lowercase().contains("on")
            }
            Err(_) => false,
        }
    }
}

/// Parse a time offset value like "+12.345ms", "-0.5s", "0.001234s".
/// Returns milliseconds.
fn parse_ntp_offset_value(val: &str) -> Result<i64, TimeError> {
    let val = val.trim();
    if val.is_empty() {
        return Err(TimeError::PlatformError("empty offset value".into()));
    }

    // Strip sign
    let (sign, rest) = if val.starts_with('+') {
        (1i64, &val[1..])
    } else if val.starts_with('-') {
        (-1i64, &val[1..])
    } else {
        (1i64, val)
    };

    // Determine unit and numeric part
    let (num_str, multiplier) = if let Some(s) = rest.strip_suffix("ms") {
        (s, 1.0f64)
    } else if let Some(s) = rest.strip_suffix("us") {
        (s, 0.001)
    } else if let Some(s) = rest.strip_suffix('s') {
        (s, 1000.0)
    } else {
        // Assume seconds if no unit
        (rest, 1000.0)
    };

    let num: f64 = num_str.trim().parse().map_err(|_| {
        TimeError::PlatformError(format!("cannot parse offset number: '{}'", num_str))
    })?;

    Ok(sign * (num * multiplier) as i64)
}

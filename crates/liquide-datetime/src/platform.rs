use crate::error::TimeError;

/// Platform bridge for querying and setting the system timezone.
///
/// Each method dispatches to the appropriate platform tool:
/// - **Linux**: `timedatectl`
/// - **Windows**: PowerShell `Get-TimeZone` / `Set-TimeZone`
/// - **macOS**: `systemsetup -gettimezone` / `-settimezone`
pub struct PlatformTimeBridge;

impl PlatformTimeBridge {
    /// Query the current system timezone (returns an IANA ID on Linux/macOS,
    /// or a Windows timezone ID on Windows).
    pub fn get_system_timezone() -> Result<String, TimeError> {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_get_timezone();
        }
        #[cfg(target_os = "windows")]
        {
            return Self::windows_get_timezone();
        }
        #[cfg(target_os = "macos")]
        {
            return Self::macos_get_timezone();
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(TimeError::PlatformError("unsupported platform".into()))
        }
    }

    /// Set the system timezone (requires appropriate privileges).
    pub fn set_system_timezone(tz_id: &str) -> Result<(), TimeError> {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_set_timezone(tz_id);
        }
        #[cfg(target_os = "windows")]
        {
            return Self::windows_set_timezone(tz_id);
        }
        #[cfg(target_os = "macos")]
        {
            return Self::macos_set_timezone(tz_id);
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = tz_id;
            Err(TimeError::PlatformError("unsupported platform".into()))
        }
    }

    /// Get the current UTC offset in minutes for the system timezone.
    /// This uses the platform's knowledge of DST rules.
    pub fn get_utc_offset_minutes() -> Result<i32, TimeError> {
        #[cfg(target_os = "linux")]
        {
            return Self::linux_get_utc_offset();
        }
        #[cfg(target_os = "windows")]
        {
            return Self::windows_get_utc_offset();
        }
        #[cfg(target_os = "macos")]
        {
            return Self::macos_get_utc_offset();
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(TimeError::PlatformError("unsupported platform".into()))
        }
    }

    // ---- Linux ----

    #[cfg(target_os = "linux")]
    fn linux_get_timezone() -> Result<String, TimeError> {
        // Try reading /etc/timezone first (Debian/Ubuntu)
        if let Ok(tz) = std::fs::read_to_string("/etc/timezone") {
            let trimmed = tz.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
        // Fall back to timedatectl
        let output = std::process::Command::new("timedatectl")
            .args(["show", "--property=Timezone", "--value"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("timedatectl: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            Err(TimeError::PlatformError("could not determine timezone".into()))
        } else {
            Ok(text)
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_set_timezone(tz_id: &str) -> Result<(), TimeError> {
        let status = std::process::Command::new("timedatectl")
            .args(["set-timezone", tz_id])
            .status()
            .map_err(|e| TimeError::PlatformError(format!("timedatectl set-timezone: {}", e)))?;
        if status.success() {
            Ok(())
        } else {
            Err(TimeError::PlatformError(format!(
                "timedatectl set-timezone exited with {}",
                status
            )))
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_get_utc_offset() -> Result<i32, TimeError> {
        // Use `date +%z` which returns e.g. "+0530" or "-0500"
        let output = std::process::Command::new("date")
            .args(["+%z"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("date: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        parse_offset_string(&text)
    }

    // ---- Windows ----

    #[cfg(target_os = "windows")]
    fn windows_get_timezone() -> Result<String, TimeError> {
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-TimeZone).Id"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("Get-TimeZone: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            Err(TimeError::PlatformError("Get-TimeZone returned empty".into()))
        } else {
            Ok(text)
        }
    }

    #[cfg(target_os = "windows")]
    fn windows_set_timezone(tz_id: &str) -> Result<(), TimeError> {
        let cmd = format!("Set-TimeZone -Id '{}'", tz_id);
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &cmd])
            .status()
            .map_err(|e| TimeError::PlatformError(format!("Set-TimeZone: {}", e)))?;
        if status.success() {
            Ok(())
        } else {
            Err(TimeError::PlatformError(format!(
                "Set-TimeZone exited with {}",
                status
            )))
        }
    }

    #[cfg(target_os = "windows")]
    fn windows_get_utc_offset() -> Result<i32, TimeError> {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-TimeZone).BaseUtcOffset.TotalMinutes",
            ])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("Get-TimeZone: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        text.parse::<f64>()
            .map(|v| v as i32)
            .map_err(|_| TimeError::PlatformError(format!("cannot parse offset: '{}'", text)))
    }

    // ---- macOS ----

    #[cfg(target_os = "macos")]
    fn macos_get_timezone() -> Result<String, TimeError> {
        let output = std::process::Command::new("systemsetup")
            .args(["-gettimezone"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("systemsetup: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout);
        // Output: "Time Zone: America/Los_Angeles"
        if let Some(pos) = text.find(':') {
            let tz = text[pos + 1..].trim().to_string();
            if !tz.is_empty() {
                return Ok(tz);
            }
        }
        Err(TimeError::PlatformError("could not parse systemsetup output".into()))
    }

    #[cfg(target_os = "macos")]
    fn macos_set_timezone(tz_id: &str) -> Result<(), TimeError> {
        let status = std::process::Command::new("systemsetup")
            .args(["-settimezone", tz_id])
            .status()
            .map_err(|e| TimeError::PlatformError(format!("systemsetup: {}", e)))?;
        if status.success() {
            Ok(())
        } else {
            Err(TimeError::PlatformError(format!(
                "systemsetup -settimezone exited with {}",
                status
            )))
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_get_utc_offset() -> Result<i32, TimeError> {
        let output = std::process::Command::new("date")
            .args(["+%z"])
            .output()
            .map_err(|e| TimeError::PlatformError(format!("date: {}", e)))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        parse_offset_string(&text)
    }
}

/// Parse an offset string like "+0530" or "-0500" to minutes.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_offset_string(s: &str) -> Result<i32, TimeError> {
    let s = s.trim();
    if s.len() < 5 {
        return Err(TimeError::PlatformError(format!("invalid offset format: '{}'", s)));
    }
    let sign = match s.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Err(TimeError::PlatformError(format!("invalid offset sign: '{}'", s))),
    };
    let hours: i32 = s[1..3].parse().map_err(|_| {
        TimeError::PlatformError(format!("invalid offset hours: '{}'", s))
    })?;
    let mins: i32 = s[3..5].parse().map_err(|_| {
        TimeError::PlatformError(format!("invalid offset minutes: '{}'", s))
    })?;
    Ok(sign * (hours * 60 + mins))
}

#[cfg(test)]
mod platform_tests {
    use super::*;

    #[test]
    fn parse_positive_offset() {
        assert_eq!(parse_offset_string("+0530").unwrap(), 330);
    }

    #[test]
    fn parse_negative_offset() {
        assert_eq!(parse_offset_string("-0500").unwrap(), -300);
    }

    #[test]
    fn parse_zero_offset() {
        assert_eq!(parse_offset_string("+0000").unwrap(), 0);
    }
}

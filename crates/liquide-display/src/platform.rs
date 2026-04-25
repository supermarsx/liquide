use crate::display::{DisplayId, DisplayInfo, Resolution, Rotation};

/// Errors from platform display enumeration.
#[derive(Debug)]
pub enum PlatformError {
    /// The platform command failed to execute.
    CommandFailed(String),
    /// The command output could not be parsed.
    ParseError(String),
    /// Platform not supported for display enumeration.
    Unsupported,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::CommandFailed(msg) => write!(f, "command failed: {}", msg),
            PlatformError::ParseError(msg) => write!(f, "parse error: {}", msg),
            PlatformError::Unsupported => write!(f, "platform not supported"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// Enumerate connected displays using platform-specific methods.
pub fn enumerate_displays() -> Result<Vec<DisplayInfo>, PlatformError> {
    #[cfg(target_os = "windows")]
    {
        enumerate_displays_windows()
    }
    #[cfg(target_os = "linux")]
    {
        enumerate_displays_linux()
    }
    #[cfg(target_os = "macos")]
    {
        enumerate_displays_macos()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(PlatformError::Unsupported)
    }
}

// ---------------------------------------------------------------------------
// Windows: PowerShell bridge using Get-CimInstance
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn enumerate_displays_windows() -> Result<Vec<DisplayInfo>, PlatformError> {
    use std::process::Command;

    // Use PowerShell to query WMI for monitor information and current display
    // settings. The script outputs one JSON array with both monitor metadata
    // and devmode settings merged.
    let ps_script = r#"
$monitors = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue
$settings = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue
$desktops = Get-CimInstance Win32_DesktopMonitor -ErrorAction SilentlyContinue

# Collect EnumDisplaySettings via .NET interop
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class DisplayEnum {
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Ansi)]
    public struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string dmDeviceName;
        public short dmSpecVersion, dmDriverVersion;
        public short dmSize, dmDriverExtra;
        public int dmFields;
        public int dmPositionX, dmPositionY;
        public int dmDisplayOrientation, dmDisplayFixedOutput;
        public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string dmFormName;
        public short dmLogPixels, dmBitsPerPel;
        public int dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
        public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType;
        public int dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
    }
    [DllImport("user32.dll")] public static extern bool EnumDisplayDevicesA(string d, uint i, ref DISPLAY_DEVICE f, uint fl);
    [DllImport("user32.dll")] public static extern bool EnumDisplaySettingsA(string d, int m, ref DEVMODE dm);
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Ansi)]
    public struct DISPLAY_DEVICE {
        public int cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceString;
        public int StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceID;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceKey;
    }
}
"@ -ErrorAction SilentlyContinue

$result = @()
$i = 0
$dd = New-Object DisplayEnum+DISPLAY_DEVICE
$dd.cb = [System.Runtime.InteropServices.Marshal]::SizeOf($dd)
while ([DisplayEnum]::EnumDisplayDevicesA($null, $i, [ref]$dd, 0)) {
    $active = ($dd.StateFlags -band 1) -ne 0
    $primary = ($dd.StateFlags -band 4) -ne 0
    $dm = New-Object DisplayEnum+DEVMODE
    $dm.dmSize = [System.Runtime.InteropServices.Marshal]::SizeOf($dm)
    $hasCurrent = [DisplayEnum]::EnumDisplaySettingsA($dd.DeviceName, -1, [ref]$dm)
    $modes = @()
    $mi = 0
    $em = New-Object DisplayEnum+DEVMODE
    $em.dmSize = [System.Runtime.InteropServices.Marshal]::SizeOf($em)
    while ([DisplayEnum]::EnumDisplaySettingsA($dd.DeviceName, $mi, [ref]$em)) {
        $modes += @{ w=$em.dmPelsWidth; h=$em.dmPelsHeight; hz=$em.dmDisplayFrequency }
        $mi++
    }
    $obj = @{
        id=$i; name=$dd.DeviceString; connector=$dd.DeviceName
        w=if($hasCurrent){$dm.dmPelsWidth}else{0}
        h=if($hasCurrent){$dm.dmPelsHeight}else{0}
        hz=if($hasCurrent){$dm.dmDisplayFrequency}else{0}
        x=if($hasCurrent){$dm.dmPositionX}else{0}
        y=if($hasCurrent){$dm.dmPositionY}else{0}
        rot=if($hasCurrent){$dm.dmDisplayOrientation}else{0}
        primary=$primary; enabled=$active; modes=$modes
    }
    $result += $obj
    $i++
    $dd.cb = [System.Runtime.InteropServices.Marshal]::SizeOf($dd)
}
$result | ConvertTo-Json -Depth 3
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
        .output()
        .map_err(|e| PlatformError::CommandFailed(format!("powershell: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PlatformError::CommandFailed(format!(
            "powershell exit {}: {}",
            output.status, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_windows_json(&stdout)
}

#[cfg(target_os = "windows")]
fn parse_windows_json(json_str: &str) -> Result<Vec<DisplayInfo>, PlatformError> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // PowerShell may return a single object (not array) if only one display.
    let values: Vec<serde_json::Value> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)
            .map_err(|e| PlatformError::ParseError(format!("json array: {}", e)))?
    } else {
        let single: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| PlatformError::ParseError(format!("json object: {}", e)))?;
        vec![single]
    };

    let mut displays = Vec::new();
    for v in &values {
        let id = v["id"].as_u64().unwrap_or(0) as DisplayId;
        let name = v["name"].as_str().unwrap_or("Unknown").to_string();
        let connector = v["connector"].as_str().unwrap_or("").to_string();
        let w = v["w"].as_u64().unwrap_or(0) as u32;
        let h = v["h"].as_u64().unwrap_or(0) as u32;
        let hz = v["hz"].as_u64().unwrap_or(60) as f32;
        let x = v["x"].as_i64().unwrap_or(0) as i32;
        let y = v["y"].as_i64().unwrap_or(0) as i32;
        let rot_val = v["rot"].as_u64().unwrap_or(0);
        let primary = v["primary"].as_bool().unwrap_or(false);
        let enabled = v["enabled"].as_bool().unwrap_or(false);

        let rotation = match rot_val {
            1 => Rotation::Right,
            2 => Rotation::Inverted,
            3 => Rotation::Left,
            _ => Rotation::Normal,
        };

        // Parse available modes.
        let mut available_resolutions = Vec::new();
        let mut available_refresh_rates = Vec::new();
        if let Some(modes) = v["modes"].as_array() {
            for m in modes {
                let mw = m["w"].as_u64().unwrap_or(0) as u32;
                let mh = m["h"].as_u64().unwrap_or(0) as u32;
                let mhz = m["hz"].as_u64().unwrap_or(0) as f32;
                let res = Resolution::new(mw, mh);
                if !available_resolutions.contains(&res) {
                    available_resolutions.push(res);
                }
                if !available_refresh_rates
                    .iter()
                    .any(|&r: &f32| (r - mhz).abs() < 0.5)
                {
                    available_refresh_rates.push(mhz);
                }
            }
        }

        displays.push(DisplayInfo {
            id,
            name,
            connector,
            resolution: Resolution::new(w, h),
            available_resolutions,
            refresh_rate: hz,
            available_refresh_rates,
            position: (x, y),
            rotation,
            scale: 1.0, // Windows DPI scaling needs additional registry query
            primary,
            enabled,
            physical_size_mm: None,
            connected: enabled || w > 0,
        });
    }

    Ok(displays)
}

// ---------------------------------------------------------------------------
// Linux: xrandr --query parsing
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn enumerate_displays_linux() -> Result<Vec<DisplayInfo>, PlatformError> {
    use std::process::Command;

    let output = Command::new("xrandr")
        .arg("--query")
        .output()
        .map_err(|e| PlatformError::CommandFailed(format!("xrandr: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PlatformError::CommandFailed(format!(
            "xrandr exit {}: {}",
            output.status, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_xrandr_output(&stdout)
}

/// Parse xrandr --query output into DisplayInfo entries.
///
/// Example output:
/// ```text
/// Screen 0: minimum 8 x 8, current 3840 x 1080, maximum 32767 x 32767
/// DP-1 connected primary 1920x1080+0+0 (normal left inverted right ...) 600mm x 340mm
///    1920x1080     60.00*+  144.00    120.00
///    2560x1440     59.95
/// HDMI-0 connected 1920x1080+1920+0 (normal ...) 530mm x 300mm
///    1920x1080     60.00*+
/// DP-2 disconnected (normal left inverted right ...)
/// ```
#[cfg(target_os = "linux")]
fn parse_xrandr_output(output: &str) -> Result<Vec<DisplayInfo>, PlatformError> {
    let mut displays = Vec::new();
    let mut current: Option<DisplayInfo> = None;
    let mut next_id: DisplayId = 0;

    for line in output.lines() {
        // Output line: "CONNECTOR STATUS [primary] [WxH+X+Y] [(rotations)] [WMM x HMM]"
        if !line.starts_with(' ') && !line.starts_with('\t') && !line.starts_with("Screen") {
            // Flush previous display.
            if let Some(d) = current.take() {
                displays.push(d);
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let connector = parts[0].to_string();
            let connected = parts[1] == "connected";
            let primary = parts.iter().any(|&p| p == "primary");

            // Parse geometry: look for WxH+X+Y pattern.
            let mut resolution = Resolution::new(0, 0);
            let mut position = (0i32, 0i32);
            let mut rotation = Rotation::Normal;
            let mut physical_size_mm = None;

            for (pi, &part) in parts.iter().enumerate() {
                // WxH+X+Y
                if part.contains('x') && part.contains('+') {
                    if let Some((res_str, pos_str)) = part.split_once('+') {
                        if let Some((ws, hs)) = res_str.split_once('x') {
                            if let (Ok(w), Ok(h)) = (ws.parse::<u32>(), hs.parse::<u32>()) {
                                resolution = Resolution::new(w, h);
                            }
                        }
                        // Parse +X+Y from remaining.
                        let coords: Vec<&str> = pos_str.split('+').collect();
                        if coords.len() >= 2 {
                            position = (
                                coords[0].parse().unwrap_or(0),
                                coords[1].parse().unwrap_or(0),
                            );
                        }
                    }
                }

                // Physical size: "600mm" followed by "x" followed by "340mm"
                if part.ends_with("mm") && pi + 2 < parts.len() && parts[pi + 1] == "x" {
                    if let Some(w_str) = part.strip_suffix("mm") {
                        if let Some(h_str) = parts[pi + 2].strip_suffix("mm") {
                            if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                                physical_size_mm = Some((w, h));
                            }
                        }
                    }
                }
            }

            // Rotation from parenthesized list — the first word is current.
            if let Some(paren_start) = line.find('(') {
                if let Some(paren_end) = line[paren_start..].find(')') {
                    let inside = &line[paren_start + 1..paren_start + paren_end];
                    let first_word = inside.split_whitespace().next().unwrap_or("normal");
                    rotation = match first_word {
                        "left" => Rotation::Left,
                        "right" => Rotation::Right,
                        "inverted" => Rotation::Inverted,
                        _ => Rotation::Normal,
                    };
                }
            }

            current = Some(DisplayInfo {
                id: next_id,
                name: connector.clone(),
                connector,
                resolution,
                available_resolutions: Vec::new(),
                refresh_rate: 0.0,
                available_refresh_rates: Vec::new(),
                position,
                rotation,
                scale: 1.0,
                primary,
                enabled: connected && resolution.width > 0,
                physical_size_mm,
                connected,
            });
            next_id += 1;
        } else if (line.starts_with(' ') || line.starts_with('\t')) && !line.starts_with("Screen") {
            // Mode line: "   1920x1080     60.00*+  144.00    120.00"
            if let Some(ref mut d) = current {
                let trimmed = line.trim();
                let mut parts_iter = trimmed.split_whitespace();
                if let Some(res_str) = parts_iter.next() {
                    if let Some((ws, hs)) = res_str.split_once('x') {
                        if let (Ok(w), Ok(h)) = (ws.parse::<u32>(), hs.parse::<u32>()) {
                            let res = Resolution::new(w, h);
                            if !d.available_resolutions.contains(&res) {
                                d.available_resolutions.push(res);
                            }

                            for rate_str in parts_iter {
                                let clean = rate_str.trim_end_matches('*').trim_end_matches('+');
                                if let Ok(hz) = clean.parse::<f32>() {
                                    // If this rate is marked with *, it's the current.
                                    if rate_str.contains('*') {
                                        d.refresh_rate = hz;
                                        d.resolution = res;
                                    }
                                    if !d
                                        .available_refresh_rates
                                        .iter()
                                        .any(|&r| (r - hz).abs() < 0.01)
                                    {
                                        d.available_refresh_rates.push(hz);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Flush last display.
    if let Some(d) = current.take() {
        displays.push(d);
    }

    Ok(displays)
}

// ---------------------------------------------------------------------------
// macOS: system_profiler SPDisplaysDataType
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn enumerate_displays_macos() -> Result<Vec<DisplayInfo>, PlatformError> {
    use std::process::Command;

    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .map_err(|e| PlatformError::CommandFailed(format!("system_profiler: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PlatformError::CommandFailed(format!(
            "system_profiler exit {}: {}",
            output.status, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_macos_json(&stdout)
}

#[cfg(target_os = "macos")]
fn parse_macos_json(json_str: &str) -> Result<Vec<DisplayInfo>, PlatformError> {
    let root: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| PlatformError::ParseError(format!("json: {}", e)))?;

    let mut displays = Vec::new();
    let mut next_id: DisplayId = 0;

    // SPDisplaysDataType is an array of GPU entries, each containing "spdisplays_ndrvs".
    let gpu_list = root["SPDisplaysDataType"]
        .as_array()
        .ok_or_else(|| PlatformError::ParseError("missing SPDisplaysDataType".into()))?;

    for gpu in gpu_list {
        let monitors = match gpu["spdisplays_ndrvs"].as_array() {
            Some(arr) => arr,
            None => continue,
        };

        for mon in monitors {
            let name = mon["_name"].as_str().unwrap_or("Unknown").to_string();

            // Resolution string like "3840 x 2160 (4K/UHD)" or "2560 x 1440".
            let res_str = mon["_spdisplays_resolution"]
                .as_str()
                .or_else(|| mon["spdisplays_resolution"].as_str())
                .unwrap_or("0 x 0");

            let resolution = parse_macos_resolution(res_str);

            // Determine if main display.
            let primary = mon["spdisplays_main"]
                .as_str()
                .map(|s| s == "spdisplays_yes")
                .unwrap_or(false);

            // Scale — Retina displays report a "spdisplays_retina" key.
            let retina = mon["spdisplays_retina"]
                .as_str()
                .map(|s| s == "spdisplays_yes")
                .unwrap_or(false);
            let scale = if retina { 2.0 } else { 1.0 };

            // Connection type.
            let connector = mon["spdisplays_connection_type"]
                .as_str()
                .unwrap_or("built-in")
                .to_string();

            displays.push(DisplayInfo {
                id: next_id,
                name,
                connector,
                resolution,
                available_resolutions: vec![resolution],
                refresh_rate: 60.0, // system_profiler doesn't always report Hz
                available_refresh_rates: vec![60.0],
                position: (0, 0),
                rotation: Rotation::Normal,
                scale,
                primary,
                enabled: true,
                physical_size_mm: None,
                connected: true,
            });
            next_id += 1;
        }
    }

    Ok(displays)
}

#[cfg(target_os = "macos")]
fn parse_macos_resolution(s: &str) -> Resolution {
    // Format: "3840 x 2160 (4K/UHD)" or "2560 x 1440"
    let cleaned = if let Some(paren) = s.find('(') {
        s[..paren].trim()
    } else {
        s.trim()
    };
    let parts: Vec<&str> = cleaned.split('x').collect();
    if parts.len() >= 2 {
        let w = parts[0].trim().parse().unwrap_or(0);
        let h = parts[1].trim().parse().unwrap_or(0);
        Resolution::new(w, h)
    } else {
        Resolution::new(0, 0)
    }
}

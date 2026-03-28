//! Platform-specific printer discovery.
//!
//! Each platform has its own mechanism to enumerate printers:
//! - Linux/macOS: `lpstat -p -d` (CUPS)
//! - Windows: PowerShell `Get-Printer`
//!
//! The discovery functions parse command output and return [`Printer`] structs.

use crate::printer::{Printer, PrinterCapabilities, PrinterId, PrinterStatus};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::paper::{PAPER_A4, PAPER_LETTER};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::process::Command;

/// Discover printers on the current platform.
///
/// Returns an empty `Vec` if discovery fails or no printers are found.
pub fn discover_printers() -> Vec<Printer> {
    #[cfg(target_os = "linux")]
    {
        discover_linux()
    }
    #[cfg(target_os = "macos")]
    {
        discover_macos()
    }
    #[cfg(target_os = "windows")]
    {
        discover_windows()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Discover printers on Linux using `lpstat`.
#[cfg(target_os = "linux")]
fn discover_linux() -> Vec<Printer> {
    discover_cups()
}

/// Discover printers on macOS using `lpstat`.
#[cfg(target_os = "macos")]
fn discover_macos() -> Vec<Printer> {
    discover_cups()
}

/// Shared CUPS-based discovery for Linux and macOS.
///
/// Runs `lpstat -p -d` to list printers and identify the default.
/// Then queries each printer with `lpoptions -p <name> -l` for capabilities.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn discover_cups() -> Vec<Printer> {
    let output = match Command::new("lpstat").args(["-p", "-d"]).output() {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("Failed to run lpstat: {}", e);
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();
    let mut default_name: Option<String> = None;
    let mut id_counter: u64 = 1;

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("system default destination: ") {
            default_name = Some(rest.trim().to_string());
        }
    }

    for line in stdout.lines() {
        // Lines look like: "printer <name> is idle." or "printer <name> disabled since ..."
        if let Some(rest) = line.strip_prefix("printer ") {
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }

            let status = if rest.contains("idle") {
                PrinterStatus::Idle
            } else if rest.contains("disabled") || rest.contains("not accepting") {
                PrinterStatus::Offline
            } else if rest.contains("printing") {
                PrinterStatus::Printing
            } else {
                PrinterStatus::Idle
            };

            let caps = query_cups_capabilities(&name);
            let is_default = default_name.as_deref() == Some(&name);
            let is_network = name.contains('@') || name.contains("://");

            printers.push(Printer {
                id: PrinterId(id_counter),
                name: name.clone(),
                location: None,
                driver: "CUPS".to_string(),
                status,
                capabilities: caps,
                is_default,
                is_network,
            });
            id_counter += 1;
        }
    }

    printers
}

/// Query CUPS printer capabilities using `lpoptions`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn query_cups_capabilities(printer_name: &str) -> PrinterCapabilities {
    let output = Command::new("lpoptions")
        .args(["-p", printer_name, "-l"])
        .output();

    let mut caps = PrinterCapabilities::default();

    let stdout = match output {
        Ok(ref o) => String::from_utf8_lossy(&o.stdout),
        Err(_) => return caps,
    };

    for line in stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("duplex") || lower.contains("sides") {
            if lower.contains("two-sided") || lower.contains("duplex") {
                caps.supports_duplex = true;
            }
        }
        if lower.contains("colormodel") || lower.contains("color") {
            if lower.contains("color") && !lower.contains("gray") {
                caps.supports_color = true;
            }
        }
        if lower.contains("resolution") {
            // Parse DPI values like "600dpi" or "1200x1200dpi"
            for token in line.split_whitespace() {
                let stripped = token
                    .trim_matches('*')
                    .trim_end_matches("dpi")
                    .trim_end_matches("DPI");
                if let Some(dpi_str) = stripped.split('x').next() {
                    if let Ok(dpi) = dpi_str.parse::<u32>() {
                        if dpi > caps.max_dpi {
                            caps.max_dpi = dpi;
                        }
                    }
                }
            }
        }
        if lower.contains("pagesize") || lower.contains("media") {
            // Detect supported paper sizes from option values
            let mut sizes = Vec::new();
            if lower.contains("a4") {
                sizes.push(PAPER_A4.clone());
            }
            if lower.contains("letter") {
                sizes.push(PAPER_LETTER.clone());
            }
            if lower.contains("a3") {
                sizes.push(crate::paper::PAPER_A3.clone());
            }
            if lower.contains("a5") {
                sizes.push(crate::paper::PAPER_A5.clone());
            }
            if lower.contains("legal") {
                sizes.push(crate::paper::PAPER_LEGAL.clone());
            }
            if !sizes.is_empty() {
                caps.paper_sizes = sizes;
            }
        }
    }

    caps
}

/// Discover printers on Windows using PowerShell `Get-Printer`.
#[cfg(target_os = "windows")]
fn discover_windows() -> Vec<Printer> {
    let output = match Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Printer | Select-Object Name, DriverName, PrinterStatus, PortName, Type | ConvertTo-Csv -NoTypeInformation",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("Failed to run Get-Printer: {}", e);
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();
    let mut id_counter: u64 = 1;

    // Find the default printer name.
    let default_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance -ClassName Win32_Printer | Where-Object { $_.Default -eq $true }).Name",
        ])
        .output();
    let default_name = default_output
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let mut lines = stdout.lines();
    // Skip header row.
    let _header = lines.next();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = parse_csv_line(line);
        if fields.len() < 5 {
            continue;
        }

        let name = fields[0].trim_matches('"').to_string();
        let driver = fields[1].trim_matches('"').to_string();
        let status_str = fields[2].trim_matches('"');
        let _port = fields[3].trim_matches('"');
        let type_str = fields[4].trim_matches('"');

        let status = match status_str {
            "Normal" | "0" => PrinterStatus::Idle,
            "Printing" | "1" => PrinterStatus::Printing,
            "Offline" | "5" => PrinterStatus::Offline,
            "PaperJam" | "6" => PrinterStatus::PaperJam,
            "TonerLow" | "18" => PrinterStatus::LowToner,
            other => {
                if other.contains("Error") {
                    PrinterStatus::Error(other.to_string())
                } else {
                    PrinterStatus::Idle
                }
            }
        };

        let is_network = type_str.contains("Connection") || type_str.contains("Network");
        let is_default = default_name.as_deref() == Some(name.as_str());

        printers.push(Printer {
            id: PrinterId(id_counter),
            name,
            location: None,
            driver,
            status,
            capabilities: PrinterCapabilities::default(),
            is_default,
            is_network,
        });
        id_counter += 1;
    }

    printers
}

/// Parse a simple CSV line, respecting quoted fields.
#[cfg(target_os = "windows")]
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quote = false;
    let bytes = line.as_bytes();

    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_quote = !in_quote,
            b',' if !in_quote => {
                fields.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(&line[start..]);
    fields
}

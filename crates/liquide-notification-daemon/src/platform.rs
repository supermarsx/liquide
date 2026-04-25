//! Platform-specific IPC bridges for receiving and sending notifications.
//!
//! Each platform implementation uses `std::process::Command` to invoke native
//! notification tools:
//! - **Linux**: `gdbus` / `dbus-send` for D-Bus `org.freedesktop.Notifications`
//! - **Windows**: PowerShell toast notification commands
//! - **macOS**: `osascript` for Notification Center
//!
//! These bridges are cfg-gated so only the relevant platform code compiles.

use crate::spec::Notification;
use std::io;

/// Result type for platform IPC operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Errors from platform IPC operations.
#[derive(Debug)]
pub enum PlatformError {
    /// The platform tool was not found on this system.
    ToolNotFound(String),
    /// The platform tool exited with a non-zero status.
    CommandFailed { tool: String, stderr: String },
    /// I/O error during command execution.
    Io(io::Error),
    /// The platform is not supported for this operation.
    Unsupported,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::ToolNotFound(t) => write!(f, "platform tool not found: {}", t),
            PlatformError::CommandFailed { tool, stderr } => {
                write!(f, "{} failed: {}", tool, stderr)
            }
            PlatformError::Io(e) => write!(f, "I/O error: {}", e),
            PlatformError::Unsupported => write!(f, "platform not supported"),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for PlatformError {
    fn from(e: io::Error) -> Self {
        PlatformError::Io(e)
    }
}

// ── Linux: D-Bus via gdbus ──────────────────────────────────────────────

/// Linux D-Bus notification bridge using `gdbus`.
#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::process::Command;

    /// Sends a notification via D-Bus using `gdbus call`.
    ///
    /// Invokes `org.freedesktop.Notifications.Notify` on the session bus.
    /// Returns the notification ID assigned by the D-Bus notification server.
    pub fn send_notification(notification: &Notification) -> PlatformResult<u32> {
        // Build the actions array for GVariant format.
        let actions_parts: Vec<String> = notification
            .actions
            .iter()
            .flat_map(|(k, v)| {
                vec![
                    format!("'{}'", escape_gvariant(k)),
                    format!("'{}'", escape_gvariant(v)),
                ]
            })
            .collect();
        let actions_str = format!("[{}]", actions_parts.join(", "));

        // Build the hints dict for GVariant format.
        let mut hints_parts = Vec::new();
        if let Some(urgency) = notification.hints.urgency {
            hints_parts.push(format!("'urgency': <byte {}>", urgency as u8));
        }
        if let Some(ref cat) = notification.hints.category {
            hints_parts.push(format!("'category': <'{}'>", escape_gvariant(cat)));
        }
        if let Some(ref de) = notification.hints.desktop_entry {
            hints_parts.push(format!("'desktop-entry': <'{}'>", escape_gvariant(de)));
        }
        if notification.hints.suppress_sound {
            hints_parts.push("'suppress-sound': <true>".to_string());
        }
        if notification.hints.transient {
            hints_parts.push("'transient': <true>".to_string());
        }
        let hints_str = format!("{{{}}}", hints_parts.join(", "));

        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.Notify",
                &escape_gvariant(&notification.app_name),
                &notification.replaces_id.to_string(),
                &escape_gvariant(&notification.icon),
                &escape_gvariant(&notification.summary),
                &escape_gvariant(&notification.body),
                &actions_str,
                &hints_str,
                &notification.expire_timeout.to_string(),
            ])
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                tool: "gdbus".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Parse the response "(uint32 N,)\n" to extract the ID.
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_dbus_uint32_response(&stdout).ok_or_else(|| PlatformError::CommandFailed {
            tool: "gdbus".to_string(),
            stderr: format!("unexpected response: {}", stdout),
        })
    }

    /// Closes a notification via D-Bus.
    pub fn close_notification(id: u32) -> PlatformResult<()> {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.CloseNotification",
                &id.to_string(),
            ])
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                tool: "gdbus".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Queries the D-Bus notification server capabilities.
    pub fn get_capabilities() -> PlatformResult<Vec<String>> {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.GetCapabilities",
            ])
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                tool: "gdbus".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_dbus_string_array(&stdout))
    }

    /// Escapes a string for use in GVariant string arguments.
    fn escape_gvariant(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    /// Parses a D-Bus response like "(uint32 42,)\n" to extract the u32 value.
    fn parse_dbus_uint32_response(s: &str) -> Option<u32> {
        // Format: "(uint32 N,)"
        let s = s.trim();
        let s = s.strip_prefix('(')?;
        let s = s.strip_suffix(')')?;
        let s = s.strip_prefix("uint32 ")?;
        let s = s.strip_suffix(',')?;
        s.trim().parse().ok()
    }

    /// Parses a D-Bus string array response like "(['cap1', 'cap2'],)".
    fn parse_dbus_string_array(s: &str) -> Vec<String> {
        let s = s.trim();
        // Strip outer parens and trailing comma.
        let s = s.strip_prefix('(').unwrap_or(s);
        let s = s.strip_suffix(')').unwrap_or(s);
        let s = s.trim().strip_suffix(',').unwrap_or(s);
        // Strip brackets.
        let s = s.strip_prefix('[').unwrap_or(s);
        let s = s.strip_suffix(']').unwrap_or(s);
        // Split by comma and strip quotes.
        s.split(',')
            .map(|part| {
                let part = part.trim();
                let part = part.strip_prefix('\'').unwrap_or(part);
                let part = part.strip_suffix('\'').unwrap_or(part);
                part.to_string()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

// ── Windows: PowerShell toast notifications ─────────────────────────────

/// Windows toast notification bridge using PowerShell.
#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use std::process::Command;

    /// Sends a toast notification via PowerShell.
    ///
    /// Uses the `BurntToast` module if available, otherwise falls back to
    /// raw `[Windows.UI.Notifications]` API via PowerShell.
    pub fn send_notification(notification: &Notification) -> PlatformResult<()> {
        let template = build_toast_xml(notification);

        let script = format!(
            r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

$template = @'
{template}
'@

$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml($template)
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
$notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('LiquiDE')
$notifier.Show($toast)
"#,
            template = template,
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                tool: "powershell".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    fn build_toast_xml(notification: &Notification) -> String {
        let actions_xml = notification
            .actions
            .iter()
            .map(|(key, label)| {
                format!(
                    "<action content='{}' arguments='{}' />",
                    escape_xml(label),
                    escape_xml(key)
                )
            })
            .collect::<Vec<_>>()
            .join("");

        let actions_block = if actions_xml.is_empty() {
            String::new()
        } else {
            format!("<actions>{}</actions>", actions_xml)
        };

        format!(
            "<toast>\n    <visual>\n        <binding template='ToastGeneric'>\n            <text>{}</text>\n            <text>{}</text>\n        </binding>\n    </visual>\n    {}\n</toast>",
            escape_xml(&notification.summary),
            escape_xml(&notification.body),
            actions_block,
        )
    }

    /// Escapes a string for embedding in XML attribute values.
    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn toast_xml_escapes_text_nodes_and_actions() {
            let notification = Notification::new("<sync & save>")
                .with_body("Body with <b>markup</b> & 'quotes'")
                .with_action("open&launch", "Open <Now>");

            let xml = build_toast_xml(&notification);

            assert!(xml.contains("<text>&lt;sync &amp; save&gt;</text>"));
            assert!(xml.contains("<text>Body with &lt;b&gt;markup&lt;/b&gt; &amp; &apos;quotes&apos;</text>"));
            assert!(xml.contains("content='Open &lt;Now&gt;'"));
            assert!(xml.contains("arguments='open&amp;launch'"));
            assert!(!xml.contains("<text><sync & save></text>"));
            assert!(!xml.contains("<text>Body with <b>markup</b>"));
        }
    }
}

// ── macOS: osascript ────────────────────────────────────────────────────

/// macOS notification bridge using `osascript`.
#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::process::Command;

    /// Sends a notification via macOS Notification Center using `osascript`.
    pub fn send_notification(notification: &Notification) -> PlatformResult<()> {
        let mut script = format!(
            "display notification \"{}\"",
            escape_applescript(&notification.body)
        );

        script.push_str(&format!(
            " with title \"{}\"",
            escape_applescript(&notification.summary)
        ));

        if !notification.app_name.is_empty() {
            script.push_str(&format!(
                " subtitle \"{}\"",
                escape_applescript(&notification.app_name)
            ));
        }

        if let Some(ref sound) = notification.hints.sound_name {
            if !notification.hints.suppress_sound {
                script.push_str(&format!(" sound name \"{}\"", escape_applescript(sound)));
            }
        }

        let output = Command::new("osascript").args(["-e", &script]).output()?;

        if !output.status.success() {
            return Err(PlatformError::CommandFailed {
                tool: "osascript".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Escapes a string for embedding in AppleScript.
    fn escape_applescript(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
}

/// Sends a notification using the current platform's native mechanism.
///
/// This is a convenience function that dispatches to the correct platform module.
pub fn send_native_notification(notification: &Notification) -> PlatformResult<()> {
    #[cfg(target_os = "linux")]
    {
        linux::send_notification(notification)?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        windows::send_notification(notification)?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        macos::send_notification(notification)?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(PlatformError::Unsupported)
    }
}

//! Serialization and persistence for [`SessionState`].
//!
//! Uses a simple INI-like text format with `[section]` headers and `key=value`
//! pairs. No external serialization crate required.

use crate::state::{DisplayState, SessionState, WindowState, WindowVisualState, WorkspaceState};
use crate::SessionError;
use std::fmt::Write as FmtWrite;

/// Handles saving and loading session state.
pub struct SessionStore;

impl SessionStore {
    // ── Serialization ───────────────────────────────────────────────

    /// Serialize a [`SessionState`] to a string.
    pub fn save(state: &SessionState) -> Result<String, SessionError> {
        let mut out = String::with_capacity(2048);

        // Global section
        writeln!(out, "[session]").unwrap();
        writeln!(out, "timestamp={}", state.timestamp).unwrap();
        writeln!(out, "active_workspace={}", state.active_workspace).unwrap();
        match state.focused_window {
            Some(id) => writeln!(out, "focused_window={}", id).unwrap(),
            None => writeln!(out, "focused_window=none").unwrap(),
        }
        writeln!(out, "theme_id={}", state.theme_id).unwrap();

        // Displays
        for (i, d) in state.display_config.iter().enumerate() {
            writeln!(out).unwrap();
            writeln!(out, "[display.{}]", i).unwrap();
            writeln!(out, "connector={}", d.connector).unwrap();
            writeln!(out, "resolution={}x{}", d.resolution.0, d.resolution.1).unwrap();
            writeln!(out, "position={},{}", d.position.0, d.position.1).unwrap();
            writeln!(out, "scale={}", d.scale).unwrap();
            writeln!(out, "primary={}", d.primary).unwrap();
        }

        // Workspaces
        for (i, ws) in state.workspaces.iter().enumerate() {
            writeln!(out).unwrap();
            writeln!(out, "[workspace.{}]", i).unwrap();
            writeln!(out, "id={}", ws.id).unwrap();
            writeln!(out, "name={}", ws.name).unwrap();
            writeln!(out, "monitor_id={}", ws.monitor_id).unwrap();
        }

        // Windows
        for (i, w) in state.windows.iter().enumerate() {
            writeln!(out).unwrap();
            writeln!(out, "[window.{}]", i).unwrap();
            writeln!(out, "window_id={}", w.window_id).unwrap();
            writeln!(out, "app_id={}", w.app_id).unwrap();
            writeln!(out, "title={}", w.title).unwrap();
            writeln!(
                out,
                "bounds={},{},{},{}",
                w.bounds.0, w.bounds.1, w.bounds.2, w.bounds.3
            )
            .unwrap();
            writeln!(out, "workspace_id={}", w.workspace_id).unwrap();
            writeln!(out, "state={}", w.state.as_str()).unwrap();
            writeln!(out, "z_order={}", w.z_order).unwrap();
            writeln!(out, "is_sticky={}", w.is_sticky).unwrap();
        }

        Ok(out)
    }

    // ── Deserialization ─────────────────────────────────────────────

    /// Deserialize a [`SessionState`] from a string previously produced by [`Self::save`].
    pub fn load(data: &str) -> Result<SessionState, SessionError> {
        let mut state = SessionState::empty();

        // Gather sections: (header, key-value pairs)
        let sections = Self::parse_sections(data)?;

        for (header, pairs) in &sections {
            if *header == "session" {
                Self::apply_session_section(&mut state, pairs)?;
            } else if header.starts_with("display.") {
                state.display_config.push(Self::parse_display(pairs)?);
            } else if header.starts_with("workspace.") {
                state.workspaces.push(Self::parse_workspace(pairs)?);
            } else if header.starts_with("window.") {
                state.windows.push(Self::parse_window(pairs)?);
            }
            // Unknown sections are silently ignored for forward compat.
        }

        Ok(state)
    }

    /// Save to a file on disk.
    pub fn save_to_file(state: &SessionState, path: &str) -> Result<(), SessionError> {
        let data = Self::save(state)?;
        std::fs::write(path, data.as_bytes())
            .map_err(|e| SessionError::Io(format!("write {}: {}", path, e)))
    }

    /// Load from a file on disk.
    pub fn load_from_file(path: &str) -> Result<SessionState, SessionError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| SessionError::Io(format!("read {}: {}", path, e)))?;
        Self::load(&data)
    }

    /// Platform-specific default path for the auto-saved session file.
    pub fn auto_save_path() -> String {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return format!("{}/liquide/session.state", appdata);
            }
            // Fallback
            return "liquide-session.state".to_string();
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return format!(
                    "{}/Library/Application Support/liquide/session.state",
                    home
                );
            }
            return "liquide-session.state".to_string();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            // XDG_CONFIG_HOME or ~/.config
            if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                return format!("{}/liquide/session.state", xdg);
            }
            if let Ok(home) = std::env::var("HOME") {
                return format!("{}/.config/liquide/session.state", home);
            }
            "liquide-session.state".to_string()
        }
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Parse the text into a list of (section_header, key-value pairs).
    fn parse_sections(data: &str) -> Result<Vec<(&str, Vec<(&str, &str)>)>, SessionError> {
        let mut sections: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
        let mut current: Option<(&str, Vec<(&str, &str)>)> = None;

        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if let Some(sec) = current.take() {
                    sections.push(sec);
                }
                let header = &line[1..line.len() - 1];
                current = Some((header, Vec::new()));
            } else if let Some((_, ref mut pairs)) = current {
                if let Some(eq) = line.find('=') {
                    let key = line[..eq].trim();
                    let val = line[eq + 1..].trim();
                    pairs.push((key, val));
                }
                // Lines without '=' inside a section are ignored.
            }
            // Lines before any section header are ignored.
        }
        if let Some(sec) = current.take() {
            sections.push(sec);
        }

        Ok(sections)
    }

    fn apply_session_section(
        state: &mut SessionState,
        pairs: &[(&str, &str)],
    ) -> Result<(), SessionError> {
        for &(key, val) in pairs {
            match key {
                "timestamp" => {
                    state.timestamp = val
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad timestamp: {}", val)))?;
                }
                "active_workspace" => {
                    state.active_workspace = val.parse().map_err(|_| {
                        SessionError::Parse(format!("bad active_workspace: {}", val))
                    })?;
                }
                "focused_window" => {
                    if val == "none" {
                        state.focused_window = None;
                    } else {
                        state.focused_window = Some(val.parse().map_err(|_| {
                            SessionError::Parse(format!("bad focused_window: {}", val))
                        })?);
                    }
                }
                "theme_id" => {
                    state.theme_id = val.to_string();
                }
                _ => {} // forward compat
            }
        }
        Ok(())
    }

    fn parse_display(pairs: &[(&str, &str)]) -> Result<DisplayState, SessionError> {
        let mut connector = String::new();
        let mut resolution = (0u32, 0u32);
        let mut position = (0i32, 0i32);
        let mut scale = 1.0f32;
        let mut primary = false;

        for &(key, val) in pairs {
            match key {
                "connector" => connector = val.to_string(),
                "resolution" => {
                    let parts: Vec<&str> = val.split('x').collect();
                    if parts.len() != 2 {
                        return Err(SessionError::Parse(format!("bad resolution: {}", val)));
                    }
                    resolution.0 = parts[0]
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad resolution w: {}", val)))?;
                    resolution.1 = parts[1]
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad resolution h: {}", val)))?;
                }
                "position" => {
                    let parts: Vec<&str> = val.split(',').collect();
                    if parts.len() != 2 {
                        return Err(SessionError::Parse(format!("bad position: {}", val)));
                    }
                    position.0 = parts[0]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad position x: {}", val)))?;
                    position.1 = parts[1]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad position y: {}", val)))?;
                }
                "scale" => {
                    scale = val
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad scale: {}", val)))?;
                }
                "primary" => {
                    primary = val == "true";
                }
                _ => {}
            }
        }

        if connector.is_empty() {
            return Err(SessionError::Parse(
                "display missing connector".to_string(),
            ));
        }

        Ok(DisplayState {
            connector,
            resolution,
            position,
            scale,
            primary,
        })
    }

    fn parse_workspace(pairs: &[(&str, &str)]) -> Result<WorkspaceState, SessionError> {
        let mut id = 0u32;
        let mut name = String::new();
        let mut monitor_id = 0u32;
        let mut has_id = false;

        for &(key, val) in pairs {
            match key {
                "id" => {
                    id = val
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad workspace id: {}", val)))?;
                    has_id = true;
                }
                "name" => name = val.to_string(),
                "monitor_id" => {
                    monitor_id = val.parse().map_err(|_| {
                        SessionError::Parse(format!("bad workspace monitor_id: {}", val))
                    })?;
                }
                _ => {}
            }
        }

        if !has_id {
            return Err(SessionError::Parse("workspace missing id".to_string()));
        }

        Ok(WorkspaceState {
            id,
            name,
            monitor_id,
        })
    }

    fn parse_window(pairs: &[(&str, &str)]) -> Result<WindowState, SessionError> {
        let mut window_id = 0u64;
        let mut app_id = String::new();
        let mut title = String::new();
        let mut bounds = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut workspace_id = 0u32;
        let mut win_state = WindowVisualState::Normal;
        let mut z_order = 0u32;
        let mut is_sticky = false;
        let mut has_window_id = false;

        for &(key, val) in pairs {
            match key {
                "window_id" => {
                    window_id = val
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad window_id: {}", val)))?;
                    has_window_id = true;
                }
                "app_id" => app_id = val.to_string(),
                "title" => title = val.to_string(),
                "bounds" => {
                    let parts: Vec<&str> = val.split(',').collect();
                    if parts.len() != 4 {
                        return Err(SessionError::Parse(format!("bad bounds: {}", val)));
                    }
                    bounds.0 = parts[0]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad bounds x: {}", val)))?;
                    bounds.1 = parts[1]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad bounds y: {}", val)))?;
                    bounds.2 = parts[2]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad bounds w: {}", val)))?;
                    bounds.3 = parts[3]
                        .trim()
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad bounds h: {}", val)))?;
                }
                "workspace_id" => {
                    workspace_id = val.parse().map_err(|_| {
                        SessionError::Parse(format!("bad window workspace_id: {}", val))
                    })?;
                }
                "state" => {
                    win_state = WindowVisualState::from_str(val).ok_or_else(|| {
                        SessionError::Parse(format!("bad window state: {}", val))
                    })?;
                }
                "z_order" => {
                    z_order = val
                        .parse()
                        .map_err(|_| SessionError::Parse(format!("bad z_order: {}", val)))?;
                }
                "is_sticky" => {
                    is_sticky = val == "true";
                }
                _ => {}
            }
        }

        if !has_window_id {
            return Err(SessionError::Parse("window missing window_id".to_string()));
        }

        Ok(WindowState {
            window_id,
            app_id,
            title,
            bounds,
            workspace_id,
            state: win_state,
            z_order,
            is_sticky,
        })
    }
}

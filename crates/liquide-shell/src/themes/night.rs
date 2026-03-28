//! Create the Night OLED-optimized theme CSS (spec-theme-night.md)
//!
//! True black backgrounds, restrained glass (10px blur), no noise/specular.
//! Optimized for OLED displays and bandwidth-constrained connections.

pub const CSS: &str = r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Night — OLED Dark
   Preset: night
   Spec: spec-theme-night.md
   ═══════════════════════════════════════════════════════ */

desktop-background {
    background: rgb(0, 0, 0);
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
}

/* ── Status bar ── */

statusbar {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 34;
    padding-left: 12;
    padding-right: 12;
    align-items: center;
    justify-content: space-between;
    z-index: 10;
    background: linear-gradient(180deg, rgba(8, 8, 12, 0.88), rgba(4, 4, 8, 0.82));
    border-bottom-color: rgba(255, 255, 255, 0.06);
    border-bottom-width: 1;
    color: rgba(255, 255, 255, 1.0);
    font-size: 13;
    font-weight: 500;
    blur-radius: 24;
    glass-tint: rgba(6, 6, 10, 0.80);
}

statusbar-slot {
    display: flex;
    align-items: center;
    flex-grow: 1;
    flex-shrink: 1;
    flex-basis: 0;
    gap: 8;
}

statusbar-slot.left { justify-content: flex-start; }
statusbar-slot.center { justify-content: center; flex-grow: 0; flex-shrink: 0; flex-basis: auto; }
statusbar-slot.right { justify-content: flex-end; }

statusbar-item {
    display: flex;
    align-items: center;
    padding-left: 4;
    padding-right: 4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* ── Logo / brand area ── */

statusbar-logo {
    display: flex;
    align-items: center;
    gap: 6;
    padding-left: 6;
    padding-right: 10;
    height: 24;
    border-radius: 7;
    font-weight: 700;
    font-size: 13;
    color: rgba(10, 132, 255, 1.0);
}

statusbar-logo:hover {
    background: rgba(10, 132, 255, 0.15);
    color: rgba(64, 156, 255, 1.0);
}

/* ── Status indicators ── */

status-indicator {
    display: flex;
    align-items: center;
    gap: 4;
    padding-left: 8;
    padding-right: 8;
    height: 22;
    border-radius: 6;
    font-size: 12;
    color: rgba(255, 255, 255, 0.60);
}

status-indicator:hover {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.85);
}

status-indicator.connected { color: rgb(48, 209, 88); }
status-indicator.degraded { color: rgb(255, 214, 10); }
status-indicator.disconnected { color: rgb(142, 142, 147); }

/* ── Notification indicator ── */

notification-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 22;
    height: 22;
    padding-left: 6;
    padding-right: 6;
    border-radius: 11;
    font-size: 11;
    font-weight: 600;
    color: rgba(100, 210, 255, 0.55);
}

notification-indicator:hover {
    background: rgba(255, 255, 255, 0.08);
}

notification-indicator.active { color: rgb(255, 69, 58); }

notification-indicator.dnd {
    background: rgba(255, 149, 0, 0.15);
    color: rgb(255, 179, 64);
}

/* ── Session / user button ── */

session-button {
    display: flex;
    align-items: center;
    gap: 6;
    padding-left: 8;
    padding-right: 10;
    height: 24;
    border-radius: 12;
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.08);
    border-width: 1;
    font-size: 12;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.90);
}

session-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

status-tray {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 4;
    padding: 2;
}

/* ── Windows ── */

window {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: rgba(10, 10, 10, 0.92);
    border-color: rgba(255, 255, 255, 0.10);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(0, 0, 0, 0.70);
    glass-tint: rgba(10, 10, 10, 0.88);
    overflow: hidden;
}

window.focused {
    border-color: rgba(255, 255, 255, 0.18);
    titlebar-background: rgba(12, 12, 12, 0.98);
}

window-titlebar {
    display: flex;
    align-items: center;
    height: 36;
    padding-left: 12;
    padding-right: 8;
    background: rgba(12, 12, 12, 0.98);
    color: rgba(255, 255, 255, 1.0);
    font-size: 13;
    font-weight: 500;
}

window-title {
    flex-grow: 1;
    text-align: center;
    color: rgba(255, 255, 255, 1.0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

titlebar-buttons {
    display: flex;
    align-items: center;
    gap: 6;
}

close-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 69, 58, 0.70);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(255, 69, 58, 0.85);
}

maximize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.80);
}

maximize-button:hover {
    background: rgba(255, 255, 255, 0.10);
}

minimize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.80);
}

minimize-button:hover {
    background: rgba(255, 255, 255, 0.10);
}

window-content {
    flex-grow: 1;
    background: rgba(10, 10, 10, 0.95);
}

/* ── Dock ── */

dock {
    display: flex;
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 56;
    justify-content: center;
    align-items: center;
    gap: 4;
    padding-left: 12;
    padding-right: 12;
    background: linear-gradient(0deg, rgba(4, 4, 8, 0.85), rgba(8, 8, 12, 0.78));
    border-top-color: rgba(255, 255, 255, 0.06);
    border-top-width: 1;
    blur-radius: 24;
    glass-tint: rgba(6, 6, 10, 0.78);
}

dock-item {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44;
    height: 44;
    border-radius: 12;
    color: rgba(255, 255, 255, 0.80);
}

dock-item.active { color: rgba(255, 255, 255, 1.0); }
dock-item:hover { background: rgba(255, 255, 255, 0.10); }

dock-item-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-grow: 1;
}

dock-item-label {
    display: none;
}

dock-indicator {
    display: flex;
    width: 4;
    height: 4;
    border-radius: 2;
    background: rgba(10, 132, 255, 0.80);
}

/* ── Workspace container ── */

workspace-container {
    position: fixed;
    top: 34;
    left: 0;
    right: 0;
    bottom: 56;
    overflow: hidden;
}

/* ── Notifications ── */

notification-area {
    display: flex;
    flex-direction: column;
    position: fixed;
    top: 36;
    right: 12;
    z-index: 20;
    gap: 8;
}

notification {
    display: flex;
    flex-direction: column;
    width: 320;
    padding: 12;
    border-radius: 12;
    background: rgba(14, 14, 14, 0.96);
    blur-radius: 10;
}

notification-title {
    font-weight: 600;
    font-size: 13;
    color: rgba(255, 255, 255, 1.0);
    margin-bottom: 4;
}

notification-body {
    font-size: 12;
    color: rgba(255, 255, 255, 0.70);
}

notification-icon {
    display: flex;
    width: 32;
    height: 32;
    margin-right: 10;
    color: rgba(255, 255, 255, 0.5);
}

notification-content {
    flex-grow: 1;
}

notification-actions {
    display: flex;
    gap: 6;
    margin-top: 8;
}

notification-action {
    display: flex;
    padding-left: 8;
    padding-right: 8;
    height: 24;
    border-radius: 6;
    font-size: 12;
    align-items: center;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.8);
}

/* ── Launcher ── */

launcher-overlay {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    align-items: center;
    justify-content: center;
    z-index: 30;
    background: rgba(0, 0, 0, 0.60);
}

launcher {
    display: flex;
    flex-direction: column;
    width: 480;
    max-height: 600;
    padding: 16;
    border-radius: 16;
    background: rgba(4, 4, 4, 0.98);
    blur-radius: 20;
}

launcher-search {
    height: 36;
    padding-left: 12;
    padding-right: 12;
    border-radius: 8;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 1.0);
    font-size: 14;
    margin-bottom: 8;
}

launcher-results {
    display: flex;
    flex-direction: column;
    gap: 2;
    overflow: hidden;
}

launcher-item {
    display: flex;
    align-items: center;
    height: 40;
    padding-left: 12;
    padding-right: 12;
    border-radius: 8;
    background: transparent;
    color: rgba(255, 255, 255, 1.0);
    font-size: 14;
}

launcher-item:hover { background: rgba(255, 255, 255, 0.06); }
launcher-item.selected { background: rgba(10, 132, 255, 0.25); }

launcher-item-icon {
    display: flex;
    width: 24;
    height: 24;
    margin-right: 10;
}

launcher-item-label {
    flex-grow: 1;
}

/* ── Menus ── */

context-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 10;
    background: rgba(10, 10, 10, 0.95);
    border-color: rgba(255, 255, 255, 0.08);
    border-width: 1;
    blur-radius: 10;
    min-width: 180;
    max-height: 480;
    overflow: hidden;
}

session-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 10;
    background: rgba(10, 10, 10, 0.95);
    border-color: rgba(255, 255, 255, 0.08);
    border-width: 1;
    blur-radius: 10;
    min-width: 200;
    max-height: 480;
    overflow: hidden;
}

app-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 10;
    background: rgba(10, 10, 10, 0.95);
    border-color: rgba(255, 255, 255, 0.08);
    border-width: 1;
    blur-radius: 10;
    min-width: 180;
    max-height: 480;
    overflow: hidden;
}

menu-item {
    display: flex;
    align-items: center;
    height: 28;
    padding-left: 12;
    padding-right: 12;
    border-radius: 6;
    color: rgba(255, 255, 255, 1.0);
    font-size: 13;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

menu-item:hover { background: rgba(10, 132, 255, 0.25); }
menu-item.disabled { color: rgba(255, 255, 255, 0.30); }

menu-item-icon {
    display: flex;
    width: 16;
    height: 16;
    margin-right: 8;
}

menu-item-label {
    flex-grow: 1;
}

menu-item-shortcut {
    color: rgba(255, 255, 255, 0.40);
    font-size: 11;
    margin-left: 12;
}

/* ── Tooltip ── */

tooltip {
    position: fixed;
    z-index: 6000;
    pointer-events: none;
    max-width: 300;
}

tooltip-content {
    background: rgba(30, 30, 30, 0.95);
    color: rgba(255, 255, 255, 0.9);
    padding: 4 8;
    border-radius: 6;
    font-size: 12;
    white-space: nowrap;
}

tooltip-arrow {
    display: none;
}

menu-separator {
    height: 1;
    margin-top: 4;
    margin-bottom: 4;
    margin-left: 12;
    margin-right: 12;
    background: rgba(255, 255, 255, 0.10);
}

/* ── Loading ── */

loading-overlay {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    align-items: center;
    justify-content: center;
    z-index: 50;
    background: rgba(0, 0, 0, 0.90);
}

loading-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32;
    border-radius: 16;
    background: rgba(14, 14, 14, 0.96);
    color: rgba(255, 255, 255, 1.0);
}

cursor { color: rgba(255, 255, 255, 1.0); }

app-settings.sidebar-item { background: rgba(255, 255, 255, 0.06); }
app-terminal { background: rgb(0, 0, 0); color: rgb(80, 220, 80); }
app-browser.urlbar { background: rgba(255, 255, 255, 0.08); }
"#;

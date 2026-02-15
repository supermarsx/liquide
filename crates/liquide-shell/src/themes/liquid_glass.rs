//! Liquid Glass dark theme — Deep blue glass with dark blue glows
//!
//! Based on spec-design.md §2.1 color palette with enhanced glass effects.
//! Inspired by Chromium's two-shadow key+ambient system, layered blur,
//! and modern glass UI patterns. All surfaces use translucent deep-blue
//! tints with stylish dark blue glow accents and heavy blur.

pub const CSS: &str = r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Liquid Glass — Deep Dark Blue
   Preset: liquid-glass (default)
   Spec: spec-design.md §2.1
   Design: Dark translucent glass with blue glow accents
   ═══════════════════════════════════════════════════════ */

/* ── Base structure ────────────────────────────────── */

desktop-background {
    background: rgb(12, 14, 28);
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
}

/* ── Status bar ────────────────────────────────────── */

statusbar {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 34;
    padding-left: 14;
    padding-right: 14;
    align-items: center;
    justify-content: space-between;
    z-index: 10;
    background: linear-gradient(180deg, rgba(18, 22, 48, 0.88), rgba(12, 16, 38, 0.82));
    border-bottom-color: rgba(60, 120, 220, 0.12);
    border-bottom-width: 1;
    color: rgba(220, 230, 255, 0.95);
    font-size: 13;
    font-weight: 500;
    blur-radius: 32;
    box-shadow-color: rgba(15, 40, 120, 0.35);
    glass-tint: rgba(14, 18, 42, 0.80);
}

statusbar-slot {
    display: flex;
    align-items: center;
    gap: 10;
    flex-grow: 1;
    flex-shrink: 1;
    flex-basis: 0;
}

statusbar-slot.left {
    justify-content: flex-start;
}

statusbar-slot.center {
    flex-grow: 0;
    flex-shrink: 0;
    flex-basis: auto;
    justify-content: center;
}

statusbar-slot.right {
    justify-content: flex-end;
}

/* ── Status bar items ─────────────────────────────── */

statusbar-item {
    display: flex;
    align-items: center;
    padding-left: 8;
    padding-right: 8;
    height: 22;
    border-radius: 6;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 400;
    color: rgba(200, 215, 255, 0.85);
}

statusbar-item:hover {
    background: rgba(60, 120, 220, 0.15);
    color: rgba(220, 235, 255, 1.0);
}

/* Clock styling — center piece, glass pill with blue glow */
statusbar-item#clock {
    font-size: 14;
    font-weight: 600;
    letter-spacing: 1;
    color: rgba(230, 240, 255, 1.0);
    padding-left: 18;
    padding-right: 18;
    background: rgba(30, 60, 130, 0.18);
    border-color: rgba(60, 120, 220, 0.15);
    border-width: 1;
    border-radius: 10;
    box-shadow-color: rgba(30, 80, 200, 0.20);
}

/* ── Logo / brand area ────────────────────────────── */

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
    color: rgba(80, 180, 255, 1.0);
}

statusbar-logo:hover {
    background: rgba(60, 120, 220, 0.15);
    color: rgba(100, 200, 255, 1.0);
}

/* ── Status indicators ────────────────────────────── */

status-indicator {
    display: flex;
    align-items: center;
    gap: 4;
    padding-left: 8;
    padding-right: 8;
    height: 22;
    border-radius: 6;
    font-size: 12;
    color: rgba(180, 200, 255, 0.70);
}

status-indicator:hover {
    background: rgba(60, 120, 220, 0.12);
    color: rgba(200, 220, 255, 0.90);
}

status-indicator.connected {
    color: rgb(52, 199, 89);
}

status-indicator.degraded {
    color: rgb(255, 204, 0);
}

status-indicator.disconnected {
    color: rgb(255, 69, 58);
}

/* ── Notification indicator ───────────────────────── */

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
    color: rgba(180, 200, 255, 0.60);
}

notification-indicator:hover {
    background: rgba(60, 120, 220, 0.12);
}

notification-indicator.active {
    background: rgba(255, 50, 50, 0.18);
    color: rgb(255, 100, 90);
    box-shadow-color: rgba(255, 50, 50, 0.15);
}

notification-indicator.dnd {
    background: rgba(255, 149, 0, 0.18);
    color: rgb(255, 179, 64);
}

/* ── System tray ──────────────────────────────────── */

status-tray {
    display: flex;
    align-items: center;
    gap: 4;
    padding-left: 6;
    padding-right: 6;
    height: 22;
    border-radius: 6;
    background: rgba(30, 60, 130, 0.12);
    border-color: rgba(60, 120, 220, 0.08);
    border-width: 1;
}

status-tray:hover {
    background: rgba(40, 80, 160, 0.18);
}

/* ── Session / user button ────────────────────────── */

session-button {
    display: flex;
    align-items: center;
    gap: 6;
    padding-left: 8;
    padding-right: 10;
    height: 24;
    border-radius: 12;
    background: rgba(30, 60, 130, 0.15);
    border-color: rgba(60, 120, 220, 0.10);
    border-width: 1;
    font-size: 12;
    font-weight: 500;
    color: rgba(200, 220, 255, 0.90);
}

session-button:hover {
    background: rgba(40, 80, 170, 0.25);
    box-shadow-color: rgba(30, 80, 200, 0.15);
}

/* ── Windows ───────────────────────────────────────── */

window {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: rgba(16, 20, 40, 0.72);
    border-color: rgba(60, 120, 220, 0.14);
    border-width: 1;
    border-radius: 14;
    box-shadow-color: rgba(10, 30, 100, 0.50);
    glass-tint: rgba(16, 20, 42, 0.68);
    blur-radius: 20;
    overflow: hidden;
}

window.focused {
    border-color: rgba(70, 140, 255, 0.25);
    box-shadow-color: rgba(30, 80, 220, 0.35);
}

window-titlebar {
    display: flex;
    align-items: center;
    height: 36;
    padding-left: 12;
    padding-right: 8;
    background: rgba(30, 50, 100, 0.12);
    border-bottom-color: rgba(60, 120, 220, 0.06);
    border-bottom-width: 1;
    color: rgba(220, 235, 255, 1.0);
    font-size: 13;
    font-weight: 500;
}

window-title {
    flex-grow: 1;
    text-align: center;
    color: rgba(220, 235, 255, 1.0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

titlebar-buttons {
    display: flex;
    align-items: center;
    gap: 8;
}

close-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 60, 48, 0.75);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(255, 60, 48, 1.0);
    box-shadow-color: rgba(255, 60, 48, 0.30);
}

maximize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(60, 120, 220, 0.15);
    color: rgba(180, 210, 255, 0.70);
}

maximize-button:hover {
    background: rgba(60, 120, 220, 0.30);
    color: rgba(220, 235, 255, 1.0);
}

minimize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(60, 120, 220, 0.15);
    color: rgba(180, 210, 255, 0.70);
}

minimize-button:hover {
    background: rgba(60, 120, 220, 0.30);
    color: rgba(220, 235, 255, 1.0);
}

window-content {
    flex-grow: 1;
    background: rgba(14, 18, 36, 0.92);
}

/* ── Dock ──────────────────────────────────────────── */

dock {
    display: flex;
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 58;
    justify-content: center;
    align-items: center;
    gap: 4;
    padding-left: 14;
    padding-right: 14;
    background: linear-gradient(0deg, rgba(12, 16, 38, 0.85), rgba(18, 24, 50, 0.78));
    border-top-color: rgba(60, 120, 220, 0.10);
    border-top-width: 1;
    blur-radius: 32;
    glass-tint: rgba(14, 18, 42, 0.75);
    box-shadow-color: rgba(15, 40, 120, 0.30);
}

dock-item {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46;
    height: 46;
    border-radius: 13;
    color: rgba(160, 190, 255, 0.75);
}

dock-item.active {
    color: rgba(120, 190, 255, 1.0);
    box-shadow-color: rgba(40, 100, 220, 0.25);
}

dock-item:hover {
    background: rgba(60, 120, 220, 0.18);
    color: rgba(200, 225, 255, 1.0);
    box-shadow-color: rgba(40, 100, 220, 0.20);
}

/* ── Workspace container ───────────────────────────── */

workspace-container {
    position: fixed;
    top: 34;
    left: 0;
    width: 100%;
    bottom: 58;
    overflow: hidden;
}

/* ── Notifications ─────────────────────────────────── */

notification-area {
    display: flex;
    flex-direction: column;
    position: fixed;
    top: 42;
    right: 12;
    z-index: 20;
    gap: 8;
}

notification {
    display: flex;
    flex-direction: column;
    width: 320;
    padding: 14;
    border-radius: 14;
    background: rgba(18, 24, 50, 0.88);
    border-color: rgba(60, 120, 220, 0.12);
    border-width: 1;
    blur-radius: 28;
    glass-tint: rgba(16, 22, 46, 0.82);
    box-shadow-color: rgba(15, 40, 120, 0.30);
}

notification-title {
    font-weight: 600;
    font-size: 13;
    color: rgba(220, 235, 255, 1.0);
    margin-bottom: 4;
}

notification-body {
    font-size: 12;
    color: rgba(170, 195, 255, 0.75);
}

/* ── Launcher ──────────────────────────────────────── */

launcher-overlay {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    align-items: center;
    justify-content: center;
    z-index: 30;
    background: rgba(6, 8, 20, 0.50);
}

launcher {
    display: flex;
    flex-direction: column;
    width: 480;
    max-height: 600;
    padding: 18;
    border-radius: 18;
    background: rgba(14, 18, 42, 0.88);
    border-color: rgba(60, 120, 220, 0.14);
    border-width: 1;
    blur-radius: 48;
    glass-tint: rgba(14, 18, 42, 0.80);
    box-shadow-color: rgba(20, 50, 140, 0.40);
}

launcher-search {
    height: 38;
    padding-left: 14;
    padding-right: 14;
    border-radius: 10;
    background: rgba(30, 60, 130, 0.15);
    border-color: rgba(60, 120, 220, 0.12);
    border-width: 1;
    color: rgba(220, 235, 255, 1.0);
    font-size: 14;
    margin-bottom: 10;
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
    color: rgba(200, 220, 255, 1.0);
    font-size: 14;
}

launcher-item:hover {
    background: rgba(60, 120, 220, 0.15);
}

launcher-item.selected {
    background: rgba(40, 100, 220, 0.30);
    box-shadow-color: rgba(40, 100, 220, 0.15);
}

/* ── Menus (context, session, app) ─────────────────── */

context-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 5;
    border-radius: 10;
    background: rgba(16, 22, 46, 0.92);
    border-color: rgba(60, 120, 220, 0.16);
    border-width: 1;
    border-style: solid;
    blur-radius: 28;
    glass-tint: rgba(14, 20, 44, 0.86);
    box-shadow-color: rgba(10, 30, 100, 0.40);
    min-width: 160;
    max-width: 240;
    max-height: 480;
    overflow: hidden;
}

session-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 5;
    border-radius: 10;
    background: rgba(16, 22, 46, 0.92);
    border-color: rgba(60, 120, 220, 0.16);
    border-width: 1;
    border-style: solid;
    blur-radius: 28;
    glass-tint: rgba(14, 20, 44, 0.86);
    box-shadow-color: rgba(10, 30, 100, 0.40);
    min-width: 180;
    max-width: 240;
    max-height: 480;
    overflow: hidden;
}

app-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 5;
    border-radius: 10;
    background: rgba(16, 22, 46, 0.92);
    border-color: rgba(60, 120, 220, 0.16);
    border-width: 1;
    border-style: solid;
    blur-radius: 28;
    glass-tint: rgba(14, 20, 44, 0.86);
    box-shadow-color: rgba(10, 30, 100, 0.40);
    min-width: 160;
    max-width: 240;
    max-height: 480;
    overflow: hidden;
}

menu-item {
    display: flex;
    align-items: center;
    height: 30;
    padding-left: 12;
    padding-right: 12;
    border-radius: 7;
    color: rgba(200, 220, 255, 1.0);
    font-size: 13;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

menu-item:hover {
    background: rgba(40, 100, 220, 0.30);
    color: rgba(230, 240, 255, 1.0);
}

menu-item.disabled {
    color: rgba(140, 160, 200, 0.35);
}

menu-separator {
    height: 1;
    margin-top: 4;
    margin-bottom: 4;
    margin-left: 12;
    margin-right: 12;
    background: rgba(60, 120, 220, 0.12);
}

/* ── Loading ───────────────────────────────────────── */

loading-overlay {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    align-items: center;
    justify-content: center;
    z-index: 50;
    background: rgba(8, 10, 24, 0.90);
}

loading-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 36;
    border-radius: 18;
    background: rgba(18, 24, 50, 0.92);
    border-color: rgba(60, 120, 220, 0.14);
    border-width: 1;
    box-shadow-color: rgba(20, 60, 180, 0.30);
    color: rgba(220, 235, 255, 1.0);
}

/* ── Cursor ────────────────────────────────────────── */

cursor {
    color: rgba(255, 255, 255, 1.0);
}

/* ── App-specific ──────────────────────────────────── */

app-settings.sidebar-item {
    background: rgba(30, 60, 130, 0.12);
}

app-terminal {
    background: rgb(10, 12, 24);
    color: rgb(80, 200, 120);
}

app-browser.urlbar {
    background: rgba(30, 60, 130, 0.15);
    border-color: rgba(60, 120, 220, 0.10);
    border-width: 1;
}
"#;

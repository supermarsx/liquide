//! Liquid Glass dark theme — Standard cool blue tones
//!
//! Based on spec-design.md §2.1 color palette.  
//! Full glass effects, translucent surfaces, default LiquiDE look.

pub const CSS: &str = r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Liquid Glass — Standard Dark
   Preset: liquid-glass (default)
   Spec: spec-design.md §2.1
   ═══════════════════════════════════════════════════════ */

/* ── Base structure ────────────────────────────────── */

desktop-background {
    background: rgb(28, 28, 46);
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
    height: 32;
    padding-left: 12;
    padding-right: 12;
    align-items: center;
    z-index: 10;
    background: linear-gradient(180deg, rgba(40, 40, 70, 0.92), rgba(25, 25, 50, 0.88));
    border-bottom-color: rgba(255, 255, 255, 0.08);
    border-bottom-width: 1;
    color: rgba(255, 255, 255, 0.95);
    font-size: 13;
    font-weight: 500;
    blur-radius: 24;
    box-shadow-color: rgba(0, 0, 0, 0.25);
}

statusbar-slot {
    display: flex;
    align-items: center;
    gap: 12;
}

statusbar-slot.left {
    flex-basis: 0;
    flex-grow: 1;
    justify-content: flex-start;
}

statusbar-slot.center {
    flex-shrink: 0;
    justify-content: center;
}

statusbar-slot.right {
    flex-basis: 0;
    flex-grow: 1;
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
}

statusbar-item:hover {
    background: rgba(255, 255, 255, 0.08);
}

/* Clock styling - center piece */
statusbar-item#clock {
    font-size: 13;
    font-weight: 500;
    letter-spacing: 0.5;
    color: rgba(255, 255, 255, 1.0);
}

/* ── Logo / brand area ────────────────────────────── */

statusbar-logo {
    display: flex;
    align-items: center;
    gap: 6;
    padding-left: 4;
    padding-right: 8;
    height: 22;
    border-radius: 6;
    font-weight: 600;
    font-size: 13;
    color: rgba(100, 210, 255, 1.0);
}

statusbar-logo:hover {
    background: rgba(255, 255, 255, 0.08);
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
    color: rgba(255, 255, 255, 0.70);
}

status-indicator:hover {
    background: rgba(255, 255, 255, 0.08);
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
    color: rgba(255, 255, 255, 0.60);
}

notification-indicator:hover {
    background: rgba(255, 255, 255, 0.08);
}

notification-indicator.active {
    background: rgba(255, 69, 58, 0.20);
    color: rgb(255, 100, 90);
}

notification-indicator.dnd {
    background: rgba(255, 149, 0, 0.20);
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
    background: rgba(255, 255, 255, 0.06);
}

status-tray:hover {
    background: rgba(255, 255, 255, 0.10);
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
    background: rgba(255, 255, 255, 0.08);
    font-size: 12;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.90);
}

session-button:hover {
    background: rgba(255, 255, 255, 0.14);
}

/* ── Windows ───────────────────────────────────────── */

window {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: rgba(30, 30, 50, 0.75);
    border-color: rgba(255, 255, 255, 0.12);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(0, 0, 0, 0.30);
    glass-tint: rgba(30, 30, 50, 0.70);
    overflow: hidden;
}

window.focused {
    border-color: rgba(255, 255, 255, 0.20);
    titlebar-background: rgba(255, 255, 255, 0.10);
}

window-titlebar {
    display: flex;
    align-items: center;
    height: 36;
    padding-left: 12;
    padding-right: 8;
    background: rgba(255, 255, 255, 0.08);
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
    background: rgba(255, 69, 58, 0.80);
    color: rgba(255, 255, 255, 0.94);
}

close-button:hover {
    background: rgba(255, 69, 58, 1.0);
}

maximize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.70);
}

maximize-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

minimize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.70);
}

minimize-button:hover {
    background: rgba(255, 255, 255, 0.12);
}

window-content {
    flex-grow: 1;
    background: rgba(30, 30, 50, 0.95);
}

/* ── Dock ──────────────────────────────────────────── */

dock {
    display: flex;
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 56;
    justify-content: center;
    align-items: center;
    gap: 4;
    padding-left: 12;
    padding-right: 12;
    background: rgba(30, 30, 50, 0.70);
    border-top-color: rgba(255, 255, 255, 0.06);
    border-top-width: 1;
    blur-radius: 20;
}

dock-item {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44;
    height: 44;
    border-radius: 12;
    color: rgba(255, 255, 255, 0.70);
}

dock-item.active {
    color: rgba(255, 255, 255, 1.0);
}

dock-item:hover {
    background: rgba(255, 255, 255, 0.12);
}

/* ── Workspace container ───────────────────────────── */

workspace-container {
    position: fixed;
    top: 32;
    left: 0;
    width: 100%;
    bottom: 56;
    overflow: hidden;
}

/* ── Notifications ─────────────────────────────────── */

notification-area {
    display: flex;
    flex-direction: column;
    position: fixed;
    top: 40;
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
    background: rgba(40, 40, 60, 0.90);
    blur-radius: 20;
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
    background: rgba(0, 0, 0, 0.40);
}

launcher {
    display: flex;
    flex-direction: column;
    width: 480;
    max-height: 600;
    padding: 16;
    border-radius: 16;
    background: rgba(20, 20, 40, 0.85);
    blur-radius: 40;
}

launcher-search {
    height: 36;
    padding-left: 12;
    padding-right: 12;
    border-radius: 8;
    background: rgba(255, 255, 255, 0.08);
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

launcher-item:hover {
    background: rgba(255, 255, 255, 0.08);
}

launcher-item.selected {
    background: rgba(0, 122, 255, 0.30);
}

/* ── Menus (context, session, app) ─────────────────── */

context-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 8;
    background: rgba(30, 30, 50, 0.92);
    border-color: rgba(255, 255, 255, 0.15);
    border-width: 1;
    border-style: solid;
    blur-radius: 20;
    min-width: 140;
    max-width: 220;
    max-height: 480;
    overflow: hidden;
}

session-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 8;
    background: rgba(30, 30, 50, 0.92);
    border-color: rgba(255, 255, 255, 0.15);
    border-width: 1;
    border-style: solid;
    blur-radius: 20;
    min-width: 160;
    max-width: 220;
    max-height: 480;
    overflow: hidden;
}

app-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 8;
    background: rgba(30, 30, 50, 0.92);
    border-color: rgba(255, 255, 255, 0.15);
    border-width: 1;
    border-style: solid;
    blur-radius: 20;
    min-width: 140;
    max-width: 220;
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

menu-item:hover {
    background: rgba(0, 122, 255, 0.30);
}

menu-item.disabled {
    color: rgba(255, 255, 255, 0.35);
}

menu-separator {
    height: 1;
    margin-top: 4;
    margin-bottom: 4;
    margin-left: 12;
    margin-right: 12;
    background: rgba(255, 255, 255, 0.12);
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
    background: rgba(20, 20, 40, 0.85);
}

loading-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32;
    border-radius: 16;
    background: rgba(40, 40, 60, 0.90);
    color: rgba(255, 255, 255, 1.0);
}

/* ── Cursor ────────────────────────────────────────── */

cursor {
    color: rgba(255, 255, 255, 1.0);
}

/* ── App-specific ──────────────────────────────────── */

app-settings.sidebar-item {
    background: rgba(255, 255, 255, 0.08);
}

app-terminal {
    background: rgb(18, 18, 30);
    color: rgb(100, 220, 100);
}

app-browser.urlbar {
    background: rgba(255, 255, 255, 0.10);
}
"#;

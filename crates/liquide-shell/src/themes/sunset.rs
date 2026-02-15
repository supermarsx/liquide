//! Create the Sunset warm dark theme CSS (spec-theme-sunset.md)
//!
//! Amber/orange tones, warm glass tint, full effects. The golden-hour theme.

pub const CSS: &str = r#"
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Sunset — Warm Dark
   Preset: sunset
   Spec: spec-theme-sunset.md
   ═══════════════════════════════════════════════════════ */

desktop-background {
    background: rgb(26, 16, 8);
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
}

/* ── Status bar ── */

statusbar {
    display: flex;
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 28;
    padding-left: 8;
    padding-right: 8;
    align-items: center;
    z-index: 10;
    background: rgba(20, 14, 4, 0.90);
    border-bottom-color: rgba(255, 180, 80, 0.06);
    border-bottom-width: 1;
    color: rgba(255, 245, 230, 1.0);
    font-size: 13;
    blur-radius: 10;
}

statusbar-slot {
    display: flex;
    align-items: center;
    flex-grow: 1;
    gap: 8;
}

statusbar-slot.left { justify-content: flex-start; }
statusbar-slot.center { justify-content: center; }
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

status-indicator.connected { color: rgb(52, 199, 89); }
status-indicator.degraded { color: rgb(255, 214, 10); }
notification-indicator.active { color: rgb(255, 107, 107); }
notification-indicator { color: rgba(255, 179, 64, 0.60); }

status-tray {
    background: rgba(255, 200, 120, 0.08);
    border-radius: 4;
    padding: 2;
}

/* ── Windows ── */

window {
    position: absolute;
    display: flex;
    flex-direction: column;
    background: rgba(32, 22, 10, 0.78);
    border-color: rgba(255, 180, 80, 0.12);
    border-width: 1;
    border-radius: 16;
    box-shadow-color: rgba(20, 10, 0, 0.40);
    glass-tint: rgba(32, 22, 10, 0.72);
    overflow: hidden;
}

window.focused {
    border-color: rgba(255, 180, 80, 0.22);
    titlebar-background: rgba(40, 28, 14, 0.65);
}

window-titlebar {
    display: flex;
    align-items: center;
    height: 36;
    padding-left: 12;
    padding-right: 8;
    background: rgba(40, 28, 14, 0.60);
    color: rgba(255, 245, 230, 1.0);
    font-size: 13;
    font-weight: 500;
}

window-title {
    flex-grow: 1;
    text-align: center;
    color: rgba(255, 245, 230, 1.0);
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
    background: rgba(255, 107, 107, 0.75);
    color: rgba(255, 245, 230, 0.94);
}

close-button:hover { background: rgba(255, 107, 107, 1.0); }

maximize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 0.72);
}

maximize-button:hover { background: rgba(255, 200, 120, 0.10); }

minimize-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14;
    height: 14;
    border-radius: 7;
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 0.72);
}

minimize-button:hover { background: rgba(255, 200, 120, 0.10); }

window-content {
    flex-grow: 1;
    background: rgba(26, 16, 8, 0.95);
}

/* ── Dock ── */

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
    background: rgba(32, 22, 10, 0.72);
    border-top-color: rgba(255, 180, 80, 0.06);
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
    color: rgba(255, 245, 230, 0.72);
}

dock-item.active { color: rgba(255, 159, 10, 1.0); }
dock-item:hover { background: rgba(255, 200, 120, 0.10); }

/* ── Workspace container ── */

workspace-container {
    position: fixed;
    top: 28;
    left: 0;
    width: 100%;
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
    background: rgba(36, 26, 12, 0.94);
    blur-radius: 20;
}

notification-title {
    font-weight: 600;
    font-size: 13;
    color: rgba(255, 245, 230, 1.0);
    margin-bottom: 4;
}

notification-body {
    font-size: 12;
    color: rgba(255, 245, 230, 0.70);
}

/* ── Launcher ── */

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
    background: rgba(0, 0, 0, 0.45);
}

launcher {
    display: flex;
    flex-direction: column;
    width: 480;
    max-height: 600;
    padding: 16;
    border-radius: 16;
    background: rgba(16, 10, 2, 0.96);
    blur-radius: 40;
}

launcher-search {
    height: 36;
    padding-left: 12;
    padding-right: 12;
    border-radius: 8;
    background: rgba(255, 200, 120, 0.06);
    color: rgba(255, 245, 230, 1.0);
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
    color: rgba(255, 245, 230, 1.0);
    font-size: 14;
}

launcher-item:hover { background: rgba(255, 200, 120, 0.08); }
launcher-item.selected { background: rgba(255, 159, 10, 0.25); }

/* ── Menus ── */

context-menu {
    display: flex;
    flex-direction: column;
    position: fixed;
    z-index: 25;
    padding: 4;
    border-radius: 10;
    background: rgba(32, 22, 10, 0.90);
    border-color: rgba(255, 180, 80, 0.10);
    border-width: 1;
    blur-radius: 20;
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
    background: rgba(32, 22, 10, 0.90);
    border-color: rgba(255, 180, 80, 0.10);
    border-width: 1;
    blur-radius: 20;
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
    background: rgba(32, 22, 10, 0.90);
    border-color: rgba(255, 180, 80, 0.10);
    border-width: 1;
    blur-radius: 20;
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
    color: rgba(255, 245, 230, 1.0);
    font-size: 13;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

menu-item:hover { background: rgba(255, 159, 10, 0.25); }
menu-item.disabled { color: rgba(255, 245, 230, 0.35); }

menu-separator {
    height: 1;
    margin-top: 4;
    margin-bottom: 4;
    margin-left: 12;
    margin-right: 12;
    background: rgba(255, 180, 80, 0.12);
}

/* ── Loading ── */

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
    background: rgba(20, 10, 0, 0.85);
}

loading-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 32;
    border-radius: 16;
    background: rgba(36, 26, 12, 0.94);
    color: rgba(255, 245, 230, 1.0);
}

cursor { color: rgba(255, 245, 230, 1.0); }

app-settings.sidebar-item { background: rgba(255, 200, 120, 0.06); }
app-terminal { background: rgb(18, 10, 2); color: rgb(255, 179, 64); }
app-browser.urlbar { background: rgba(255, 200, 120, 0.08); }
"#;

# LiquiDE Theme: Midday — Tarnished White Light Theme Specification

> **Preset ID**: `midday`
> **Type**: Light (warm off-white)
> **License**: MIT
> **Related specs**: [Design Language](spec-design.md) · [Night Theme](spec-theme-night.md) · [Sunset Theme](spec-theme-sunset.md)

---

## 0) Overview

**Midday** is LiquiDE's light theme. Rather than a clinical pure-white design, Midday uses a **tarnished white** — a warm, slightly yellowed off-white reminiscent of aged paper, natural linen, or the diffused light of midday sun through frosted glass. Surfaces have a creamy warmth that makes the interface feel organic and easy on the eyes, avoiding the harshness of pure white backgrounds.

Midday is the ideal choice for:
- Well-lit environments where dark themes cause excessive contrast with surroundings.
- Users who prefer light-mode interfaces but find pure white too harsh.
- Prolonged reading and document-heavy workflows.
- Environments with overhead lighting where dark screens create glare reflections.

---

## 1) Design Philosophy

### 1.1 Tarnished White
- No surface is pure white (`#FFFFFF`). The lightest background is a warm off-white (`#F5F0E8`).
- All whites carry a subtle warm shift — cream, linen, parchment.
- This prevents the "clinical" or "glaring" feel of pure-white light themes.

### 1.2 Glass on Light
- Glass effects work **differently in light mode**: windows and panels use white-tinted translucency instead of dark-tinted.
- Background content bleeds through as warm light blurs instead of dark shadows.
- Glass tint shifts to `rgba(180, 160, 120, 0.04)` — warm neutral.
- Specular highlights are subtle and warm.

### 1.3 Deep Accents
- Primary accent is a deep teal-blue (`#0071B3`) — strong enough to stand against light surfaces.
- Accent hover/active states are darker, not lighter (inverse of dark theme behavior).
- The accent provides strong visual anchoring on the light canvas.

### 1.4 Dark Text
- Primary text is a warm near-black (`#1C1B18`) — not pure black, matching the tarnished philosophy.
- Secondary text uses warm gray tones.
- High readability maintained through contrast without the harshness of pure #000/#FFF pairs.

---

## 2) Color Palette

```css
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Midday — Tarnished White Light
   Preset: midday
   ═══════════════════════════════════════════════════════ */

.liquid-theme-midday {

  /* ── Primary colors ────────────────────────── */
  --liquid-accent:             #0071B3;
  --liquid-accent-hover:       #005C94;
  --liquid-accent-active:      #004A78;
  --liquid-accent-rgb:         0, 113, 179;

  /* ── Text colors ───────────────────────────── */
  --liquid-text:               #1C1B18;
  --liquid-text-secondary:     rgba(28, 27, 24, 0.62);
  --liquid-text-tertiary:      rgba(28, 27, 24, 0.42);
  --liquid-text-disabled:      rgba(28, 27, 24, 0.28);
  --liquid-text-on-accent:     #FFFFFF;

  /* ── Surface colors ────────────────────────── */
  --liquid-surface:            rgba(28, 27, 24, 0.04);
  --liquid-surface-hover:      rgba(28, 27, 24, 0.07);
  --liquid-surface-active:     rgba(28, 27, 24, 0.10);
  --liquid-surface-elevated:   rgba(28, 27, 24, 0.03);

  /* ── Background ────────────────────────────── */
  --liquid-bg-desktop:         #F5F0E8;
  --liquid-bg-window:          rgba(248, 244, 238, 0.82);
  --liquid-bg-panel:           rgba(245, 240, 232, 0.90);
  --liquid-bg-dock:            rgba(248, 244, 238, 0.78);
  --liquid-bg-popover:         rgba(250, 246, 240, 0.94);
  --liquid-bg-modal:           rgba(248, 244, 238, 0.97);
  --liquid-bg-tooltip:         rgba(50, 46, 38, 0.92);

  /* ── Borders ───────────────────────────────── */
  --liquid-border:             rgba(28, 27, 24, 0.10);
  --liquid-border-strong:      rgba(28, 27, 24, 0.18);
  --liquid-border-subtle:      rgba(28, 27, 24, 0.05);
  --liquid-border-focus:       var(--liquid-accent);

  /* ── Shadows (warm, soft) ──────────────────── */
  --liquid-shadow-sm:          0 2px 8px rgba(28, 20, 8, 0.08);
  --liquid-shadow-md:          0 8px 32px rgba(28, 20, 8, 0.12);
  --liquid-shadow-lg:          0 16px 64px rgba(28, 20, 8, 0.16);
  --liquid-shadow-dock:        0 0 40px rgba(28, 20, 8, 0.12);

  /* ── Semantic colors ───────────────────────── */
  --liquid-success:            #248A3D;
  --liquid-warning:            #B25000;
  --liquid-error:              #D70015;
  --liquid-info:               #0071B3;

  /* ── Connection status ─────────────────────── */
  --liquid-status-connected:   #248A3D;
  --liquid-status-reconnecting:#B25000;
  --liquid-status-disconnected:#D70015;
}
```

---

## 3) Glass Configuration

```css
.liquid-theme-midday {

  /* ── Glass properties (light-mode adapted) ─── */
  --liquid-glass-blur:         20px;
  --liquid-glass-blur-heavy:   40px;
  --liquid-glass-blur-light:   10px;
  --liquid-glass-noise:        0.025;          /* lighter noise — visible on light bg */
  --liquid-glass-tint:         rgba(180, 160, 120, 0.04);
  --liquid-glass-specular:     rgba(255, 255, 255, 0.12);
  --liquid-glass-inner-glow:   inset 0 1px 0 rgba(255, 255, 255, 0.40);

  /* ── Specular enabled (warm, subtle) ────────── */
  --liquid-specular-enabled:   true;
  --liquid-specular-size:      300px;
  --liquid-specular-opacity:   0.05;
}
```

---

## 4) Typography Adjustments

```css
.liquid-theme-midday {
  /* Font stack unchanged — inherits from base */
}

/* Light-mode text shadow: slightly darkened for glass readability */
.liquid-theme-midday .liquid-glass .text-content {
  text-shadow: 0 1px 1px rgba(255, 255, 255, 0.6);
}
```

---

## 5) Component Overrides

### 5.1 Windows

```css
.liquid-theme-midday .liquid-window {
  background: var(--liquid-bg-window);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-lg);
  box-shadow: var(--liquid-shadow-md);
}

.liquid-theme-midday .liquid-window.focused {
  border-color: var(--liquid-border-strong);
  box-shadow: var(--liquid-shadow-lg);
}

/* Titlebar: warm light glass */
.liquid-theme-midday .liquid-window .titlebar {
  background: rgba(240, 236, 228, 0.65);
  border-bottom: 1px solid var(--liquid-border-subtle);
}

.liquid-theme-midday .liquid-window .titlebar .title {
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-window .window-btn {
  color: var(--liquid-text-secondary);
}

.liquid-theme-midday .liquid-window .window-btn:hover {
  background: var(--liquid-surface-hover);
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-window .close-btn:hover {
  background: var(--liquid-error);
  color: white;
}
```

### 5.2 Dock

```css
.liquid-theme-midday .liquid-dock {
  background: var(--liquid-bg-dock);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  box-shadow: var(--liquid-shadow-dock);
}

/* Active dot: dark on light */
.liquid-theme-midday .liquid-dock .dock-item.active::after {
  background: var(--liquid-text);
}

/* Badge: keeps red for visibility on light background */
.liquid-theme-midday .liquid-dock .dock-item .badge {
  background: var(--liquid-error);
  color: white;
}
```

### 5.3 Status Bar

```css
.liquid-theme-midday .liquid-status-bar {
  background: rgba(242, 238, 230, 0.92);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border-bottom: 1px solid var(--liquid-border);
  color: var(--liquid-text-secondary);
}

.liquid-theme-midday .liquid-status-bar .status-center {
  color: var(--liquid-text);
}
```

### 5.4 Login Screen

```css
.liquid-theme-midday .liquid-login .login-frost {
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  background: rgba(245, 240, 232, 0.35);
}

.liquid-theme-midday .liquid-login .login-glow {
  background: radial-gradient(
    circle,
    rgba(var(--liquid-accent-rgb), 0.06) 0%,
    transparent 70%
  );
}

/* Light particles on light background */
.liquid-theme-midday .liquid-login .login-particle {
  background: rgba(28, 27, 24, 0.025);
}

/* Clock: dark text on light */
.liquid-theme-midday .liquid-login .login-clock {
  color: var(--liquid-text);
  text-shadow: 0 2px 12px rgba(28, 20, 8, 0.08);
}

.liquid-theme-midday .liquid-login .login-date {
  color: var(--liquid-text-secondary);
}

/* Avatar: warm border on light */
.liquid-theme-midday .liquid-login .login-avatar {
  border: 3px solid rgba(28, 27, 24, 0.10);
  box-shadow:
    inset 0 0 12px rgba(28, 27, 24, 0.05),
    0 4px 24px rgba(28, 20, 8, 0.10);
}

/* Input fields: light surface */
.liquid-theme-midday .liquid-login .login-input,
.liquid-theme-midday .liquid-login .login-username-input {
  background: rgba(255, 255, 255, 0.60);
  border: 1px solid var(--liquid-border);
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-login .login-input:focus,
.liquid-theme-midday .liquid-login .login-username-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.18);
}

/* Submit button */
.liquid-theme-midday .liquid-login .login-submit {
  background: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}

.liquid-theme-midday .liquid-login .login-submit:hover {
  background: var(--liquid-accent-hover);
  box-shadow: 0 4px 16px rgba(var(--liquid-accent-rgb), 0.25);
}

/* Error: dark danger color */
.liquid-theme-midday .liquid-login .login-input.error {
  border-color: var(--liquid-error);
}

.liquid-theme-midday .liquid-login .login-error-message {
  color: var(--liquid-error);
}
```

### 5.5 Notifications

```css
.liquid-theme-midday .liquid-notification {
  background: rgba(248, 244, 238, 0.94);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
}

.liquid-theme-midday .liquid-notification.urgent {
  border-left: 3px solid var(--liquid-error);
}

.liquid-theme-midday .liquid-notification .notif-title {
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-notification .notif-body {
  color: var(--liquid-text-secondary);
}
```

### 5.6 App Launcher

```css
.liquid-theme-midday .liquid-launcher {
  background: rgba(248, 244, 238, 0.97);
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  border: 1px solid var(--liquid-border);
}

.liquid-theme-midday .liquid-launcher .search-input {
  background: rgba(255, 255, 255, 0.65);
  border: 1px solid var(--liquid-border);
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-launcher .search-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.18);
}

.liquid-theme-midday .liquid-launcher .app-item .app-name {
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-launcher .app-item .app-desc {
  color: var(--liquid-text-tertiary);
}
```

### 5.7 Buttons

```css
.liquid-theme-midday .liquid-btn {
  background: var(--liquid-surface);
  border: 1px solid var(--liquid-border);
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-btn:hover {
  background: var(--liquid-surface-hover);
  border-color: var(--liquid-border-strong);
}

.liquid-theme-midday .liquid-btn.primary {
  background: var(--liquid-accent);
  border-color: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}

.liquid-theme-midday .liquid-btn.primary:hover {
  background: var(--liquid-accent-hover);
}

.liquid-theme-midday .liquid-btn.danger {
  color: var(--liquid-error);
}

.liquid-theme-midday .liquid-btn.danger:hover {
  background: rgba(215, 0, 21, 0.08);
}
```

### 5.8 Input Fields

```css
.liquid-theme-midday .liquid-input {
  background: rgba(255, 255, 255, 0.60);
  border: 1px solid var(--liquid-border);
  color: var(--liquid-text);
}

.liquid-theme-midday .liquid-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.18);
}

.liquid-theme-midday .liquid-input::placeholder {
  color: var(--liquid-text-tertiary);
}
```

### 5.9 Scrollbars

```css
.liquid-theme-midday .liquid-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(28, 27, 24, 0.12);
  border-radius: var(--liquid-radius-full);
  border: 2px solid transparent;
  background-clip: content-box;
}

.liquid-theme-midday .liquid-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(28, 27, 24, 0.20);
  background-clip: content-box;
}
```

### 5.10 Tooltips

```css
/* Tooltips remain dark even in light theme for contrast */
.liquid-theme-midday .liquid-tooltip {
  background: var(--liquid-bg-tooltip);
  color: #F5F0E8;
  border: 1px solid rgba(255, 255, 255, 0.08);
}
```

### 5.11 Tile Preview

```css
.liquid-theme-midday {
  --liquid-tile-preview-bg:     rgba(0, 113, 179, 0.10);
  --liquid-tile-preview-border: 2px solid var(--liquid-accent);
}
```

---

## 6) Default Wallpaper

- **Name**: `midday-linen.jpg`
- **Description**: Soft, warm off-white gradient with subtle linen texture and gentle warm light caustics. Feels like sunlight diffused through thin curtains onto a linen surface. Very low saturation — almost monochrome warm.
- **Dimensions**: 3840×2160 (4K source).
- **Average luminance**: ~85% — bright enough for a light theme, not pure white to avoid glare.
- **Color temperature**: warm (4500K–5000K equivalent white point).
- **Wallpaper disabled mode**: `--liquid-bg-desktop: #F5F0E8` — warm off-white fallback.

---

## 7) Performance Characteristics

| Property | Midday Value | Standard Value | Impact |
|----------|-------------|----------------|--------|
| Glass blur | 20px | 20px | Same compositing cost |
| Noise texture | 0.025 | 0.03 | Negligible difference |
| Specular highlight | Enabled | Enabled | Same cursor tracking cost |
| Shadow complexity | Softer | Standard | Same render cost |
| Background brightness | Light (~85%) | Dark (~15%) | Higher entropy, potentially higher bandwidth |

**Bandwidth note**: Light themes produce higher-entropy frames than dark themes, which can result in 10-20% higher bandwidth usage under similar content. The adaptive quality system accounts for this automatically. For bandwidth-constrained connections, dark themes (Night or standard) are recommended.

---

## 8) Accessibility Considerations

- Warm near-black text (`#1C1B18`) on tarnished white (`#F5F0E8`) provides WCAG AAA contrast ratio (>12:1).
- Secondary text at 0.62 opacity maintains WCAG AA compliance (>4.5:1).
- Deep teal accent (`#0071B3`) is distinguishable across all common color vision deficiencies.
- Light theme provides better readability in bright ambient lighting where dark themes cause pupil constriction strain.
- Tooltips remain dark (inverted) for strong contrast against the light interface.
- Semantic colors (success, warning, error) are darker/more saturated than their dark-theme equivalents to maintain contrast on light backgrounds.

---

## 9) Configuration

To activate Midday theme:

### Server-side (per-user)
```toml
# ~/.config/liquide/session.toml
[appearance]
theme = "midday"
```

### Server-side (system default)
```toml
# /etc/liquide/server.toml
[appearance]
default_theme = "midday"
```

### Client-side
```toml
# Client config
[general]
theme = "midday"
```

### CSS override file
```css
/* ~/.config/liquide/theme.css */
@import "/etc/liquide/themes/midday.css";

/* Optional: cooler off-white variant */
:root {
  --liquid-bg-desktop: #EFF2F5;   /* cooler linen */
}
```

---

## 10) High Contrast Light Mode

For users who need maximum contrast in light mode:

```css
.liquid-high-contrast.liquid-theme-midday {
  --liquid-bg-desktop:         #FFFFFF;
  --liquid-bg-window:          rgba(255, 255, 255, 0.98);
  --liquid-bg-dock:            rgba(255, 255, 255, 0.98);
  --liquid-text:               #000000;
  --liquid-text-secondary:     #000000;
  --liquid-border:             rgba(0, 0, 0, 0.40);
  --liquid-border-strong:      rgba(0, 0, 0, 0.70);
  --liquid-glass-blur:         0px;
  --liquid-glass-noise:        0;
  --liquid-specular-enabled:   false;
}
```

---

## 11) Theme Metadata

```toml
# /etc/liquide/themes/midday.toml
[theme]
id = "midday"
name = "Midday"
description = "Warm off-white light theme with tarnished linen tones"
type = "light"
author = "LiquiDE"
version = "1.0.0"

[theme.tags]
light = true
warm = true
low_eye_strain = true
high_ambient_light = true

[theme.wallpaper]
default = "midday-linen.jpg"
fallback_color = "#F5F0E8"
```

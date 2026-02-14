# LiquiDE Theme: Sunset — Warm Dark Theme Specification

> **Preset ID**: `sunset`
> **Type**: Dark (warm-toned)
> **License**: MIT
> **Related specs**: [Design Language](spec-design.md) · [Night Theme](spec-theme-night.md) · [Midday Theme](spec-theme-midday.md)

---

## 0) Overview

**Sunset** is LiquiDE's warm-toned dark theme. It replaces the cool blue default palette with rich amber, burnt orange, and warm brown tones — evoking the golden hour. Glass surfaces are tinted with a warm undertone, and the accent color is a vibrant orange that cuts through the dark chocolate surfaces.

Sunset is the ideal choice for:
- Users who prefer warm-toned interfaces over cool blues.
- Evening / late-night work sessions where warm colors reduce eye strain.
- Creative and design workflows where a warm ambient tone is preferred.
- Users who want a distinctive, personality-rich desktop.

---

## 1) Design Philosophy

### 1.1 Golden Warmth
- All surfaces carry a warm amber/brown undertone instead of blue-gray.
- Desktop background is a deep chocolate brown (`#1A1008`), not blue-black.
- Glass tint shifts from neutral white to warm amber.

### 1.2 Full Glass
- Glass effects are **fully enabled** — blur, noise, specular, shadows.
- Glass tint color is amber-shifted: `rgba(255, 160, 10, 0.04)`.
- The frosted glass effect takes on a warm, inviting tone.

### 1.3 Orange Accent
- Primary accent is a vivid orange (`#FF9F0A`) — the "sunset" signature color.
- Hover and active states shift toward darker amber tones.
- Accent appears in buttons, focus rings, active indicators, and selection highlights.

### 1.4 Warm Neutrals
- Text uses a slightly warm white (`#FFF5E6`) instead of pure white.
- Borders use amber-tinted white instead of neutral white.
- Shadows carry a warm brown tone instead of pure black.

---

## 2) Color Palette

```css
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Sunset — Warm Dark
   Preset: sunset
   ═══════════════════════════════════════════════════════ */

.liquid-theme-sunset {

  /* ── Primary colors ────────────────────────── */
  --liquid-accent:             #FF9F0A;
  --liquid-accent-hover:       #E08C00;
  --liquid-accent-active:      #CC7A00;
  --liquid-accent-rgb:         255, 159, 10;

  /* ── Text colors ───────────────────────────── */
  --liquid-text:               #FFF5E6;
  --liquid-text-secondary:     rgba(255, 245, 230, 0.72);
  --liquid-text-tertiary:      rgba(255, 245, 230, 0.50);
  --liquid-text-disabled:      rgba(255, 245, 230, 0.30);
  --liquid-text-on-accent:     #1A0E00;

  /* ── Surface colors ────────────────────────── */
  --liquid-surface:            rgba(255, 200, 120, 0.06);
  --liquid-surface-hover:      rgba(255, 200, 120, 0.10);
  --liquid-surface-active:     rgba(255, 200, 120, 0.15);
  --liquid-surface-elevated:   rgba(255, 200, 120, 0.08);

  /* ── Background ────────────────────────────── */
  --liquid-bg-desktop:         #1A1008;
  --liquid-bg-window:          rgba(32, 22, 10, 0.78);
  --liquid-bg-panel:           rgba(24, 16, 6, 0.88);
  --liquid-bg-dock:            rgba(32, 22, 10, 0.72);
  --liquid-bg-popover:         rgba(40, 28, 14, 0.92);
  --liquid-bg-modal:           rgba(20, 14, 4, 0.96);
  --liquid-bg-tooltip:         rgba(50, 36, 18, 0.95);

  /* ── Borders ───────────────────────────────── */
  --liquid-border:             rgba(255, 180, 80, 0.12);
  --liquid-border-strong:      rgba(255, 180, 80, 0.22);
  --liquid-border-subtle:      rgba(255, 180, 80, 0.06);
  --liquid-border-focus:       var(--liquid-accent);

  /* ── Shadows (warm-toned) ──────────────────── */
  --liquid-shadow-sm:          0 2px 8px rgba(20, 10, 0, 0.30);
  --liquid-shadow-md:          0 8px 32px rgba(20, 10, 0, 0.40);
  --liquid-shadow-lg:          0 16px 64px rgba(20, 10, 0, 0.50);
  --liquid-shadow-dock:        0 0 40px rgba(20, 10, 0, 0.60);

  /* ── Semantic colors ───────────────────────── */
  --liquid-success:            #34C759;
  --liquid-warning:            #FFD60A;
  --liquid-error:              #FF6B6B;
  --liquid-info:               #FFB340;

  /* ── Connection status ─────────────────────── */
  --liquid-status-connected:   #34C759;
  --liquid-status-reconnecting:#FFD60A;
  --liquid-status-disconnected:#FF6B6B;
}
```

---

## 3) Glass Configuration

```css
.liquid-theme-sunset {

  /* ── Glass properties (full, warm-tinted) ──── */
  --liquid-glass-blur:         20px;
  --liquid-glass-blur-heavy:   40px;
  --liquid-glass-blur-light:   10px;
  --liquid-glass-noise:        0.035;          /* slightly higher — warm frosted texture */
  --liquid-glass-tint:         rgba(255, 160, 10, 0.04);
  --liquid-glass-specular:     rgba(255, 200, 120, 0.08);
  --liquid-glass-inner-glow:   inset 0 1px 0 rgba(255, 200, 120, 0.08);

  /* ── Specular enabled (warm tone) ──────────── */
  --liquid-specular-enabled:   true;
  --liquid-specular-size:      320px;
  --liquid-specular-opacity:   0.06;
}
```

---

## 4) Typography Adjustments

```css
.liquid-theme-sunset {
  /* Font stack unchanged — inherits from base */
}

/* Warm text shadow for glass readability */
.liquid-theme-sunset .liquid-glass .text-content {
  text-shadow: 0 1px 2px rgba(20, 10, 0, 0.4);
}
```

---

## 5) Component Overrides

### 5.1 Windows

```css
.liquid-theme-sunset .liquid-window {
  background: var(--liquid-bg-window);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-lg);
  box-shadow: var(--liquid-shadow-md);
}

.liquid-theme-sunset .liquid-window.focused {
  border-color: var(--liquid-border-strong);
  box-shadow: var(--liquid-shadow-lg);
}

/* Titlebar: warm glass surface */
.liquid-theme-sunset .liquid-window .titlebar {
  background: rgba(40, 28, 14, 0.60);
  border-bottom: 1px solid var(--liquid-border-subtle);
}

/* Close button: warm red, not cool red */
.liquid-theme-sunset .liquid-window .close-btn:hover {
  background: #FF6B6B;
  color: #1A0E00;
}
```

### 5.2 Dock

```css
.liquid-theme-sunset .liquid-dock {
  background: var(--liquid-bg-dock);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  box-shadow: var(--liquid-shadow-dock);
}

/* Active dot: warm amber instead of white */
.liquid-theme-sunset .liquid-dock .dock-item.active::after {
  background: var(--liquid-accent);
}

/* Badge: orange instead of red */
.liquid-theme-sunset .liquid-dock .dock-item .badge {
  background: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}
```

### 5.3 Status Bar

```css
.liquid-theme-sunset .liquid-status-bar {
  background: rgba(20, 14, 4, 0.90);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border-bottom: 1px solid var(--liquid-border-subtle);
}
```

### 5.4 Login Screen

```css
.liquid-theme-sunset .liquid-login .login-frost {
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  background: rgba(20, 10, 0, 0.30);
}

.liquid-theme-sunset .liquid-login .login-glow {
  background: radial-gradient(
    circle,
    rgba(255, 159, 10, 0.10) 0%,
    transparent 70%
  );
}

/* Warm-tinted particles */
.liquid-theme-sunset .liquid-login .login-particle {
  background: rgba(255, 180, 80, 0.04);
}

/* Submit button: orange accent */
.liquid-theme-sunset .liquid-login .login-submit {
  background: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}

.liquid-theme-sunset .liquid-login .login-submit:hover {
  background: var(--liquid-accent-hover);
  box-shadow: 0 4px 16px rgba(255, 159, 10, 0.30);
}
```

### 5.5 Notifications

```css
.liquid-theme-sunset .liquid-notification {
  background: rgba(36, 26, 12, 0.94);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
}

/* Urgent border: warm red instead of cool red */
.liquid-theme-sunset .liquid-notification.urgent {
  border-left: 3px solid #FF6B6B;
}
```

### 5.6 App Launcher

```css
.liquid-theme-sunset .liquid-launcher {
  background: rgba(16, 10, 2, 0.96);
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  border: 1px solid var(--liquid-border);
}

.liquid-theme-sunset .liquid-launcher .search-input {
  background: rgba(255, 200, 120, 0.06);
  border: 1px solid var(--liquid-border);
}

.liquid-theme-sunset .liquid-launcher .search-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.20);
}
```

### 5.7 Buttons

```css
.liquid-theme-sunset .liquid-btn.primary {
  background: var(--liquid-accent);
  border-color: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}

.liquid-theme-sunset .liquid-btn.primary:hover {
  background: var(--liquid-accent-hover);
}

.liquid-theme-sunset .liquid-btn:focus-visible {
  outline: 2px solid var(--liquid-accent);
  outline-offset: 2px;
}

.liquid-theme-sunset .liquid-btn.danger {
  color: #FF6B6B;
}

.liquid-theme-sunset .liquid-btn.danger:hover {
  background: rgba(255, 107, 107, 0.15);
}
```

### 5.8 Input Fields

```css
.liquid-theme-sunset .liquid-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(255, 159, 10, 0.20);
}
```

### 5.9 Tile Preview

```css
.liquid-theme-sunset {
  --liquid-tile-preview-bg:     rgba(255, 159, 10, 0.12);
  --liquid-tile-preview-border: 2px solid var(--liquid-accent);
}
```

---

## 6) Default Wallpaper

- **Name**: `sunset-amber.jpg`
- **Description**: Deep warm gradient from dark chocolate (bottom) through burnt sienna to a subtle golden glow (upper third). Organic flowing shapes reminiscent of sand dunes or heat haze. Low saturation to complement, not compete with, the glass UI.
- **Dimensions**: 3840×2160 (4K source).
- **Average luminance**: ~12% — dark enough for glass contrast, warm enough for atmosphere.
- **Color temperature**: warm (2800K–3200K equivalent white point).
- **Wallpaper disabled mode**: `--liquid-bg-desktop: #1A1008` — deep chocolate brown fallback.

---

## 7) Performance Characteristics

| Property | Sunset Value | Standard Value | Impact |
|----------|-------------|----------------|--------|
| Glass blur | 20px | 20px | Same compositing cost |
| Noise texture | 0.035 | 0.03 | Negligible difference |
| Specular highlight | Enabled | Enabled | Same cursor tracking cost |
| Shadow complexity | Standard | Standard | Same render cost |
| Background darkness | Dark (~12%) | Dark (~15%) | Comparable compression efficiency |

**Performance**: Sunset has essentially identical performance characteristics to the standard Liquid Glass theme. The warm color shift does not affect compositing or encoding costs.

---

## 8) Accessibility Considerations

- Warm white text (`#FFF5E6`) on dark brown surfaces maintains WCAG AAA ratio (>10:1 for primary text).
- Orange accent (`#FF9F0A`) is distinguishable for most color vision deficiencies.
- **Note**: orange/red distinction may be reduced for deuteranopia users — error states use a warm red (`#FF6B6B`) with sufficient luminance difference from the orange accent to remain distinguishable even with reduced color perception.
- Info color uses a warm amber (`#FFB340`) instead of blue, avoiding the standard theme's reliance on blue/orange distinction.

---

## 9) Configuration

To activate Sunset theme:

### Server-side (per-user)
```toml
# ~/.config/liquide/session.toml
[appearance]
theme = "sunset"
```

### Server-side (system default)
```toml
# /etc/liquide/server.toml
[appearance]
default_theme = "sunset"
```

### Client-side
```toml
# Client config
[general]
theme = "sunset"
```

### CSS override file
```css
/* ~/.config/liquide/theme.css */
@import "/etc/liquide/themes/sunset.css";

/* Optional: shift accent toward deeper amber */
:root {
  --liquid-accent: #E08C00;
}
```

---

## 10) Theme Metadata

```toml
# /etc/liquide/themes/sunset.toml
[theme]
id = "sunset"
name = "Sunset"
description = "Warm dark theme with amber and orange tones"
type = "dark"
author = "LiquiDE"
version = "1.0.0"

[theme.tags]
warm = true
amber = true
low_eye_strain = true

[theme.wallpaper]
default = "sunset-amber.jpg"
fallback_color = "#1A1008"
```

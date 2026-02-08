# LiquiDE Theme: Night — OLED Dark Theme Specification

> **Preset ID**: `night`
> **Type**: Dark (OLED-optimized)
> **License**: MIT
> **Related specs**: [Design Language](spec-design.md) · [Sunset Theme](spec-theme-sunset.md) · [Midday Theme](spec-theme-midday.md)

---

## 0) Overview

**Night** is LiquiDE's OLED-optimized dark theme. It uses pure black (`#000000`) backgrounds and deep, true-dark surfaces to maximize contrast and minimize power draw on OLED/AMOLED displays. Glass effects are toned down in favor of sharp edges and high-contrast borders, giving the theme a clean, minimal, "lights-off cockpit" aesthetic.

Night is the ideal choice for:
- OLED/AMOLED client displays (power savings, no burn-in glow).
- Nighttime / low-light environments.
- Users who prefer maximum contrast with minimal visual chrome.
- Bandwidth-constrained connections (darker frames compress better).

---

## 1) Design Philosophy

### 1.1 True Black
- Desktop background is pure `#000000` by default.
- Window backgrounds use near-black tones with minimal alpha.
- Dock and panels sit on true black with subtle translucency.
- OLED pixels are fully off in background regions.

### 1.2 Restrained Glass
- Glass blur is **reduced** (10px default vs. 20px standard) — enough for depth, not enough to glow.
- Noise texture is **disabled** — clean surfaces only.
- Specular highlights are **disabled** — no glow pollution on OLED.
- Shadows are **tighter and darker** — subtle depth cues without bleeding.

### 1.3 Accent Precision
- Cool blue accent (`#0A84FF`) tuned for OLED clarity.
- Accent color appears sparingly — focused on interactive elements and indicators.
- No ambient glow or tinted surfaces — accent only where it serves function.

### 1.4 High Legibility
- Text is pure white (`#FFFFFF`) against near-black surfaces.
- Secondary text uses a higher opacity than the standard theme (0.80 vs. 0.70) for improved readability.
- Text shadows are disabled — not needed against opaque dark surfaces.

---

## 2) Color Palette

```css
/* ═══════════════════════════════════════════════════════
   LiquiDE Theme: Night — OLED Dark
   Preset: night
   ═══════════════════════════════════════════════════════ */

.liquid-theme-night {

  /* ── Primary colors ────────────────────────── */
  --liquid-accent:             #0A84FF;
  --liquid-accent-hover:       #409CFF;
  --liquid-accent-active:      #0066CC;
  --liquid-accent-rgb:         10, 132, 255;

  /* ── Text colors ───────────────────────────── */
  --liquid-text:               #FFFFFF;
  --liquid-text-secondary:     rgba(255, 255, 255, 0.80);
  --liquid-text-tertiary:      rgba(255, 255, 255, 0.55);
  --liquid-text-disabled:      rgba(255, 255, 255, 0.32);
  --liquid-text-on-accent:     #FFFFFF;

  /* ── Surface colors ────────────────────────── */
  --liquid-surface:            rgba(255, 255, 255, 0.06);
  --liquid-surface-hover:      rgba(255, 255, 255, 0.10);
  --liquid-surface-active:     rgba(255, 255, 255, 0.14);
  --liquid-surface-elevated:   rgba(255, 255, 255, 0.08);

  /* ── Background ────────────────────────────── */
  --liquid-bg-desktop:         #000000;
  --liquid-bg-window:          rgba(10, 10, 10, 0.92);
  --liquid-bg-panel:           rgba(8, 8, 8, 0.95);
  --liquid-bg-dock:            rgba(10, 10, 10, 0.88);
  --liquid-bg-popover:         rgba(18, 18, 18, 0.95);
  --liquid-bg-modal:           rgba(6, 6, 6, 0.98);
  --liquid-bg-tooltip:         rgba(28, 28, 28, 0.95);

  /* ── Borders ───────────────────────────────── */
  --liquid-border:             rgba(255, 255, 255, 0.10);
  --liquid-border-strong:      rgba(255, 255, 255, 0.18);
  --liquid-border-subtle:      rgba(255, 255, 255, 0.05);
  --liquid-border-focus:       var(--liquid-accent);

  /* ── Shadows ───────────────────────────────── */
  --liquid-shadow-sm:          0 1px 4px rgba(0, 0, 0, 0.60);
  --liquid-shadow-md:          0 4px 16px rgba(0, 0, 0, 0.70);
  --liquid-shadow-lg:          0 8px 32px rgba(0, 0, 0, 0.80);
  --liquid-shadow-dock:        0 0 24px rgba(0, 0, 0, 0.90);

  /* ── Semantic colors ───────────────────────── */
  --liquid-success:            #30D158;
  --liquid-warning:            #FFD60A;
  --liquid-error:              #FF453A;
  --liquid-info:               #64D2FF;

  /* ── Connection status ─────────────────────── */
  --liquid-status-connected:   #30D158;
  --liquid-status-reconnecting:#FFD60A;
  --liquid-status-disconnected:#FF453A;
}
```

---

## 3) Glass Configuration

```css
.liquid-theme-night {

  /* ── Glass properties (restrained for OLED) ── */
  --liquid-glass-blur:         10px;
  --liquid-glass-blur-heavy:   20px;
  --liquid-glass-blur-light:   5px;
  --liquid-glass-noise:        0;              /* disabled — clean surfaces */
  --liquid-glass-tint:         rgba(255, 255, 255, 0.02);
  --liquid-glass-specular:     rgba(255, 255, 255, 0.00);
  --liquid-glass-inner-glow:   inset 0 1px 0 rgba(255, 255, 255, 0.06);

  /* ── Specular disabled ─────────────────────── */
  --liquid-specular-enabled:   false;
}
```

---

## 4) Typography Adjustments

```css
.liquid-theme-night {

  /* Text rendering optimized for OLED contrast */
  /* Font stack unchanged — inherits from base */

  /* No text shadow needed against opaque dark surfaces */
}

.liquid-theme-night .liquid-glass .text-content {
  text-shadow: none;
}
```

---

## 5) Component Overrides

### 5.1 Windows

```css
.liquid-theme-night .liquid-window {
  background: var(--liquid-bg-window);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-lg);
  box-shadow: var(--liquid-shadow-md);
}

.liquid-theme-night .liquid-window.focused {
  border-color: var(--liquid-border-strong);
  box-shadow: var(--liquid-shadow-lg);
}

/* Titlebar: solid dark, no transparency */
.liquid-theme-night .liquid-window .titlebar {
  background: rgba(12, 12, 12, 0.98);
  border-bottom: 1px solid var(--liquid-border);
}

/* Close button: dimmer red on OLED to avoid bloom */
.liquid-theme-night .liquid-window .close-btn:hover {
  background: rgba(255, 69, 58, 0.85);
  color: white;
}
```

### 5.2 Dock

```css
.liquid-theme-night .liquid-dock {
  background: var(--liquid-bg-dock);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  box-shadow: var(--liquid-shadow-dock);
}

/* Active dot: brighter to stand against true black */
.liquid-theme-night .liquid-dock .dock-item.active::after {
  background: var(--liquid-text);
  box-shadow: 0 0 4px rgba(255, 255, 255, 0.3);
}
```

### 5.3 Status Bar

```css
.liquid-theme-night .liquid-status-bar {
  background: rgba(0, 0, 0, 0.95);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border-bottom: 1px solid var(--liquid-border-subtle);
}
```

### 5.4 Login Screen

```css
.liquid-theme-night .liquid-login .login-frost {
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  background: rgba(0, 0, 0, 0.60);
}

.liquid-theme-night .liquid-login .login-glow {
  /* Subtler glow for OLED — avoid burn-in risk */
  background: radial-gradient(
    circle,
    rgba(var(--liquid-accent-rgb), 0.04) 0%,
    transparent 60%
  );
}

/* Disable particles — unnecessary glow on OLED */
.liquid-theme-night .liquid-login .login-particles {
  display: none;
}
```

### 5.5 Notifications

```css
.liquid-theme-night .liquid-notification {
  background: rgba(14, 14, 14, 0.96);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
}
```

### 5.6 App Launcher

```css
.liquid-theme-night .liquid-launcher {
  background: rgba(4, 4, 4, 0.98);
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  border: 1px solid var(--liquid-border);
}

.liquid-theme-night .liquid-launcher .search-input {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--liquid-border);
}
```

### 5.7 Tile Preview

```css
.liquid-theme-night {
  --liquid-tile-preview-bg:     rgba(10, 132, 255, 0.10);
  --liquid-tile-preview-border: 2px solid var(--liquid-accent);
}
```

---

## 6) Default Wallpaper

- **Name**: `night-void.jpg`
- **Description**: Pure black or near-black abstract with extremely subtle dark blue gradient veins. Nearly invisible — designed to let window glass be the visual interest.
- **Dimensions**: 3840×2160 (4K source).
- **Average luminance**: ≤5% — ensures OLED pixels remain mostly off.
- **Wallpaper disabled mode**: `--liquid-bg-desktop: #000000` — true black fallback.

---

## 7) Performance Characteristics

| Property | Night Value | Standard Value | Impact |
|----------|------------|----------------|--------|
| Glass blur | 10px | 20px | Lower compositing cost |
| Noise texture | Disabled | 0.03 | No texture overhead |
| Specular highlight | Disabled | Enabled | No per-frame cursor tracking |
| Shadow complexity | Tighter | Standard | Slightly less render area |
| Background darkness | True black | Dark blue | Better video compression (dark frames compress 20-40% smaller) |
| Login particles | Disabled | Optional | No particle animation overhead |

**Bandwidth benefit**: Dark themes compress significantly better with H.264/H.265/AV1. Night's true black surfaces produce minimal entropy, resulting in measurably lower bandwidth usage versus lighter themes — estimated 15-30% bandwidth reduction for typical desktop workloads.

---

## 8) Accessibility Considerations

- Night's high contrast (pure white on near-black) exceeds WCAG AAA ratio (>12:1 for primary text).
- Secondary text at 0.80 opacity maintains WCAG AA compliance (>7:1).
- Focus rings are clearly visible against dark surfaces.
- **Caution**: for users with certain visual conditions (e.g., astigmatism), reading white text on pure black can cause halation. These users may prefer the standard dark theme or Sunset instead.

---

## 9) Configuration

To activate Night theme:

### Server-side (per-user)
```toml
# ~/.config/liquide/session.toml
[appearance]
theme = "night"
```

### Server-side (system default)
```toml
# /etc/liquide/server.toml
[appearance]
default_theme = "night"
```

### Client-side
```toml
# Client config
[general]
theme = "night"
```

### CSS override file
```css
/* ~/.config/liquide/theme.css */
@import "/etc/liquide/themes/night.css";

/* Optional: further customization on top of Night */
:root {
  --liquid-accent: #5E5CE6;  /* purple accent variant */
}
```

---

## 10) Theme Metadata

```toml
# /etc/liquide/themes/night.toml
[theme]
id = "night"
name = "Night"
description = "OLED-optimized dark theme with true black backgrounds"
type = "dark"
author = "LiquiDE"
version = "1.0.0"

[theme.tags]
oled = true
high_contrast = true
low_bandwidth = true
power_efficient = true

[theme.wallpaper]
default = "night-void.jpg"
fallback_color = "#000000"
```

# LiquidDE Design Language — Specification

> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Client](spec-client.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md)

---

## 0) Overview

The **Liquid Glass** design language defines the visual identity of LiquidDE — both the remote desktop environment and the LiquidClient application. Every visual element is implemented through a CSS-driven theming engine, making the entire appearance customizable by users and administrators.

This document serves as both a design specification and the authoritative CSS documentation for theming LiquidDE.

---

## 1) Design Principles

### 1.1 Depth
- Visual hierarchy communicated through layers, blur, and shadows.
- Foreground elements float above blurred backgrounds.
- Multiple depth levels create a sense of physical space.

### 1.2 Translucency
- Panels, windows, and UI elements are semi-transparent.
- Background content is visible through UI chrome (blurred, not clear).
- Translucency level adapts to context: more opaque for readability, more transparent for aesthetics.

### 1.3 Clarity
- **Text is always sharp** — never blurred or compromised by effects.
- Backgrounds soften to push text forward.
- High contrast between text and its immediate background.
- Subpixel rendering when safe for the transport codec.

### 1.4 Motion Discipline
- Animations are purposeful — they communicate state changes, not decoration.
- Animations never spam frames (event-driven, not constant).
- All animations respect the CPU effect budget.
- Every animation can be disabled via reduce-motion preference.

### 1.5 Remote-First Ergonomics
- Large hit targets (minimum 44×44px logical).
- Clear focus outlines for keyboard navigation.
- Connection quality indicator always visible.
- UI adapts to bandwidth constraints (simpler effects under low bandwidth).

---

## 2) Color System

### 2.1 Base Palette

```css
:root {
  /* ── Primary colors ────────────────────────── */
  --liquid-accent:             #007AFF;
  --liquid-accent-hover:       #0066DD;
  --liquid-accent-active:      #0055BB;

  /* ── Text colors ───────────────────────────── */
  --liquid-text:               #FFFFFF;
  --liquid-text-secondary:     rgba(255, 255, 255, 0.70);
  --liquid-text-tertiary:      rgba(255, 255, 255, 0.50);
  --liquid-text-disabled:      rgba(255, 255, 255, 0.30);
  --liquid-text-on-accent:     #FFFFFF;

  /* ── Surface colors ────────────────────────── */
  --liquid-surface:            rgba(255, 255, 255, 0.08);
  --liquid-surface-hover:      rgba(255, 255, 255, 0.12);
  --liquid-surface-active:     rgba(255, 255, 255, 0.16);
  --liquid-surface-elevated:   rgba(255, 255, 255, 0.10);

  /* ── Background ────────────────────────────── */
  --liquid-bg-desktop:         #1C1C2E;
  --liquid-bg-window:          rgba(30, 30, 50, 0.75);
  --liquid-bg-panel:           rgba(20, 20, 40, 0.85);
  --liquid-bg-dock:            rgba(30, 30, 50, 0.70);
  --liquid-bg-popover:         rgba(40, 40, 60, 0.90);
  --liquid-bg-modal:           rgba(20, 20, 40, 0.95);
  --liquid-bg-tooltip:         rgba(60, 60, 80, 0.95);

  /* ── Borders ───────────────────────────────── */
  --liquid-border:             rgba(255, 255, 255, 0.12);
  --liquid-border-strong:      rgba(255, 255, 255, 0.20);
  --liquid-border-subtle:      rgba(255, 255, 255, 0.06);
  --liquid-border-focus:       var(--liquid-accent);

  /* ── Shadows ───────────────────────────────── */
  --liquid-shadow-sm:          0 2px 8px rgba(0, 0, 0, 0.20);
  --liquid-shadow-md:          0 8px 32px rgba(0, 0, 0, 0.30);
  --liquid-shadow-lg:          0 16px 64px rgba(0, 0, 0, 0.40);
  --liquid-shadow-dock:        0 0 40px rgba(0, 0, 0, 0.50);

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

### 2.2 Light Theme Override

```css
.liquid-theme-light {
  --liquid-text:               #1C1C1E;
  --liquid-text-secondary:     rgba(28, 28, 30, 0.60);
  --liquid-text-tertiary:      rgba(28, 28, 30, 0.40);
  --liquid-bg-desktop:         #F2F2F7;
  --liquid-bg-window:          rgba(255, 255, 255, 0.75);
  --liquid-bg-panel:           rgba(255, 255, 255, 0.85);
  --liquid-bg-dock:            rgba(255, 255, 255, 0.70);
  --liquid-surface:            rgba(0, 0, 0, 0.04);
  --liquid-surface-hover:      rgba(0, 0, 0, 0.08);
  --liquid-border:             rgba(0, 0, 0, 0.10);
  --liquid-shadow-md:          0 8px 32px rgba(0, 0, 0, 0.15);
}
```

### 2.3 Accent Color Customization
Users can set any accent color:
```css
:root {
  --liquid-accent: #FF6B35;   /* custom orange accent */
}
```
The system auto-generates hover/active variants by darkening/lightening.

---

## 3) Glass Effect System

### 3.1 Glass Layers

Glass effects are created with a combination of background blur, tinted overlay, and subtle borders:

```css
.liquid-glass {
  background: var(--liquid-bg-window);
  backdrop-filter: blur(var(--liquid-glass-blur));
  -webkit-backdrop-filter: blur(var(--liquid-glass-blur));
  border: var(--liquid-border);
  box-shadow: var(--liquid-shadow-md);
}
```

### 3.2 Glass Variables

```css
:root {
  /* ── Glass properties ──────────────────────── */
  --liquid-glass-blur:         20px;
  --liquid-glass-blur-heavy:   40px;
  --liquid-glass-blur-light:   10px;
  --liquid-glass-noise:        0.03;         /* frosted noise texture opacity */
  --liquid-glass-tint:         rgba(255, 255, 255, 0.05);
  --liquid-glass-specular:     rgba(255, 255, 255, 0.10);
  --liquid-glass-inner-glow:   inset 0 1px 0 rgba(255, 255, 255, 0.10);
}
```

### 3.3 Glass Levels

Different UI elements use different glass intensities:

| Level | Blur | Opacity | Use |
|-------|------|---------|-----|
| **Heavy** | 40px | 0.85 | Modals, critical dialogs |
| **Standard** | 20px | 0.75 | Windows, panels |
| **Light** | 10px | 0.65 | Dock, popovers |
| **Subtle** | 5px | 0.50 | Tooltips, badges |

### 3.4 Frosted Noise Texture
- A subtle noise overlay gives glass surfaces a "frosted" look.
- Applied as a pseudo-element with low opacity.
- Noise is a static tiling texture (not animated — no CPU cost after first render).
- Configurable: `--liquid-glass-noise: 0` disables it.

### 3.5 Specular Highlights
- Glass surfaces can show a soft highlight that responds to pointer position.
- **Throttled**: highlight position updates at most 15fps, not on every mouse move.
- **Optional**: can be disabled for performance.
- Implemented as a radial gradient overlay positioned relative to the cursor.

```css
:root {
  --liquid-specular-enabled:   true;
  --liquid-specular-size:      300px;
  --liquid-specular-opacity:   0.08;
  --liquid-specular-fps:       15;
}
```

### 3.6 Performance-Adaptive Glass
When the CPU effect budget is tight, glass degrades gracefully:

1. **Full glass**: blur + noise + specular + shadow.
2. **Reduced**: blur (downsampled) + shadow, no noise/specular.
3. **Minimal**: tinted solid background + shadow, no blur.
4. **Flat**: solid background, no effects.

This degradation is automatic (controlled by effect budget) but can be forced via CSS:

```css
.liquid-performance-minimal .liquid-glass {
  backdrop-filter: none;
  background: var(--liquid-bg-panel);
}
```

---

## 4) Typography

### 4.1 Font Stack

```css
:root {
  --liquid-font-sans:      "Inter", "SF Pro", "Segoe UI", system-ui, -apple-system, sans-serif;
  --liquid-font-mono:      "JetBrains Mono", "SF Mono", "Cascadia Code", "Fira Code", monospace;
  --liquid-font-size-xs:   11px;
  --liquid-font-size-sm:   13px;
  --liquid-font-size-base: 14px;
  --liquid-font-size-md:   16px;
  --liquid-font-size-lg:   20px;
  --liquid-font-size-xl:   24px;
  --liquid-font-size-2xl:  32px;
  --liquid-font-weight-regular:  400;
  --liquid-font-weight-medium:   500;
  --liquid-font-weight-semibold: 600;
  --liquid-font-weight-bold:     700;
  --liquid-line-height:          1.5;
  --liquid-letter-spacing:       0.01em;
}
```

### 4.2 Text Rendering
- Text is **always rendered sharply** — blur effects never apply to text layers.
- Text has a subtle text-shadow for readability on glass backgrounds.
- Subpixel rendering mode:
  - `auto` — enabled for local display, disabled for remote (codec artifacts).
  - `always` — forced on.
  - `never` — forced off (best for remote at lower bitrates).

### 4.3 Text on Glass
To ensure readability on translucent backgrounds:
```css
.liquid-glass .text-content {
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}
```

---

## 5) Spacing & Layout

### 5.1 Spacing Scale

```css
:root {
  --liquid-space-0:    0px;
  --liquid-space-1:    4px;
  --liquid-space-2:    8px;
  --liquid-space-3:    12px;
  --liquid-space-4:    16px;
  --liquid-space-5:    20px;
  --liquid-space-6:    24px;
  --liquid-space-8:    32px;
  --liquid-space-10:   40px;
  --liquid-space-12:   48px;
  --liquid-space-16:   64px;
}
```

### 5.2 Border Radius

```css
:root {
  --liquid-radius-none: 0;
  --liquid-radius-sm:   6px;
  --liquid-radius:      10px;
  --liquid-radius-md:   12px;
  --liquid-radius-lg:   16px;
  --liquid-radius-xl:   20px;
  --liquid-radius-2xl:  24px;
  --liquid-radius-full: 9999px;
}
```

### 5.3 Hit Targets
- Minimum interactive element size: 44×44px (logical pixels).
- Touch targets: 48×48px minimum.
- Padding inside interactive elements: minimum 8px.

---

## 6) Animation & Transitions

### 6.1 Duration Scale

```css
:root {
  --liquid-duration-instant:  0ms;
  --liquid-duration-fast:     100ms;
  --liquid-duration-normal:   200ms;
  --liquid-duration-slow:     300ms;
  --liquid-duration-slower:   500ms;
}
```

### 6.2 Easing Curves

```css
:root {
  --liquid-ease-default:    cubic-bezier(0.4, 0.0, 0.2, 1.0);  /* Material standard */
  --liquid-ease-in:         cubic-bezier(0.4, 0.0, 1.0, 1.0);
  --liquid-ease-out:        cubic-bezier(0.0, 0.0, 0.2, 1.0);
  --liquid-ease-in-out:     cubic-bezier(0.4, 0.0, 0.2, 1.0);
  --liquid-ease-bounce:     cubic-bezier(0.34, 1.56, 0.64, 1.0);
  --liquid-ease-spring:     cubic-bezier(0.5, 1.8, 0.5, 0.8);
}
```

### 6.3 Animation Rules
- **Appear**: fade in + slight upward translate (200ms, ease-out).
- **Disappear**: fade out + slight downward translate (150ms, ease-in).
- **Hover**: 100ms color/shadow transition.
- **Focus**: 100ms outline transition.
- **Window open**: scale from 0.95→1.0 + fade in (250ms, ease-out).
- **Window close**: scale to 0.95 + fade out (200ms, ease-in).
- **Window minimize**: scale to dock icon position (300ms, ease-in-out).
- **Dock magnification**: 150ms scale transition on hover.
- **Notification enter**: slide in from right (300ms, bounce ease).
- **Notification exit**: fade out + slide right (200ms, ease-in).

### 6.4 Reduce Motion
When enabled (`prefers-reduced-motion` or DE setting):
- All animations set to `duration: 0ms`.
- Transitions become instant state changes.
- Specular highlights disabled.
- Dock magnification disabled.

```css
@media (prefers-reduced-motion: reduce) {
  * {
    transition-duration: 0ms !important;
    animation-duration: 0ms !important;
  }
}
```

---

## 7) Component Specifications

### 7.1 Windows

```css
.liquid-window {
  background: var(--liquid-bg-window);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-lg);
  box-shadow: var(--liquid-shadow-md);
  overflow: hidden;
}

.liquid-window.focused {
  border-color: var(--liquid-border-strong);
  box-shadow: var(--liquid-shadow-lg);
}

.liquid-window .titlebar {
  height: 36px;
  padding: 0 var(--liquid-space-3);
  display: flex;
  align-items: center;
  gap: var(--liquid-space-2);
  background: var(--liquid-surface);
  border-bottom: 1px solid var(--liquid-border-subtle);
  user-select: none;
  -webkit-app-region: drag;
}

.liquid-window .titlebar .title {
  font-size: var(--liquid-font-size-sm);
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-text);
  flex: 1;
  text-align: center;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.liquid-window .window-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--liquid-radius-full);
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: var(--liquid-text-secondary);
  transition: background var(--liquid-duration-fast) var(--liquid-ease-default);
  -webkit-app-region: no-drag;
}

.liquid-window .window-btn:hover {
  background: var(--liquid-surface-hover);
  color: var(--liquid-text);
}

.liquid-window .close-btn:hover {
  background: var(--liquid-error);
  color: white;
}
```

### 7.2 Dock

```css
.liquid-dock {
  position: fixed;
  /* Position depends on config: bottom, left, right, top */
  bottom: var(--liquid-space-2);
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: var(--liquid-space-1);
  padding: var(--liquid-space-2) var(--liquid-space-3);
  background: var(--liquid-bg-dock);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-xl);
  box-shadow: var(--liquid-shadow-dock);
  z-index: 1000;
}

.liquid-dock .dock-item {
  width: var(--liquid-dock-icon-size, 48px);
  height: var(--liquid-dock-icon-size, 48px);
  border-radius: var(--liquid-radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform var(--liquid-duration-fast) var(--liquid-ease-spring);
  position: relative;
  cursor: pointer;
}

.liquid-dock .dock-item:hover {
  transform: scale(1.3) translateY(-8px);
}

/* Neighbor magnification */
.liquid-dock .dock-item:hover + .dock-item,
.liquid-dock .dock-item:has(+ .dock-item:hover) {
  transform: scale(1.15) translateY(-4px);
}

.liquid-dock .dock-item .icon {
  width: 100%;
  height: 100%;
  border-radius: var(--liquid-radius-md);
  object-fit: contain;
}

.liquid-dock .dock-item.active::after {
  content: "";
  position: absolute;
  bottom: -6px;
  left: 50%;
  transform: translateX(-50%);
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--liquid-text);
}

.liquid-dock .dock-separator {
  width: 1px;
  height: 32px;
  background: var(--liquid-border);
  margin: 0 var(--liquid-space-1);
}

.liquid-dock .dock-item .badge {
  position: absolute;
  top: -2px;
  right: -2px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: var(--liquid-radius-full);
  background: var(--liquid-error);
  color: white;
  font-size: 10px;
  font-weight: var(--liquid-font-weight-bold);
  display: flex;
  align-items: center;
  justify-content: center;
}
```

### 7.3 Dock Position Variants

```css
/* Left dock */
.liquid-dock.position-left {
  bottom: auto;
  left: var(--liquid-space-2);
  top: 50%;
  transform: translateY(-50%);
  flex-direction: column;
}

.liquid-dock.position-left .dock-item:hover {
  transform: scale(1.3) translateX(8px);
}

/* Right dock */
.liquid-dock.position-right {
  bottom: auto;
  left: auto;
  right: var(--liquid-space-2);
  top: 50%;
  transform: translateY(-50%);
  flex-direction: column;
}

.liquid-dock.position-right .dock-item:hover {
  transform: scale(1.3) translateX(-8px);
}

/* Top dock */
.liquid-dock.position-top {
  bottom: auto;
  top: var(--liquid-space-2);
}

.liquid-dock.position-top .dock-item:hover {
  transform: scale(1.3) translateY(8px);
}
```

### 7.4 Status Bar

```css
.liquid-status-bar {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: var(--liquid-statusbar-height, 28px);
  display: flex;
  align-items: center;
  padding: 0 var(--liquid-space-3);
  background: var(--liquid-bg-panel);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border-bottom: 1px solid var(--liquid-border-subtle);
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-secondary);
  z-index: 999;
}

.liquid-status-bar .status-left {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-3);
  flex: 1;
}

.liquid-status-bar .status-center {
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-text);
}

.liquid-status-bar .status-right {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-3);
  flex: 1;
  justify-content: flex-end;
}

.liquid-status-bar .connection-indicator {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-1);
}

.liquid-status-bar .connection-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--liquid-status-connected);
}
```

### 7.5 App Launcher

```css
.liquid-launcher {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 600px;
  max-height: 500px;
  background: var(--liquid-bg-modal);
  backdrop-filter: blur(var(--liquid-glass-blur-heavy));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-2xl);
  box-shadow: var(--liquid-shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.liquid-launcher .search-bar {
  padding: var(--liquid-space-4);
  border-bottom: 1px solid var(--liquid-border-subtle);
}

.liquid-launcher .search-input {
  width: 100%;
  height: 40px;
  padding: 0 var(--liquid-space-4);
  background: var(--liquid-surface);
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius);
  color: var(--liquid-text);
  font-size: var(--liquid-font-size-md);
  outline: none;
}

.liquid-launcher .search-input:focus {
  border-color: var(--liquid-border-focus);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.25);
}

.liquid-launcher .results {
  flex: 1;
  overflow-y: auto;
  padding: var(--liquid-space-2);
}

.liquid-launcher .app-item {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-3);
  padding: var(--liquid-space-3) var(--liquid-space-4);
  border-radius: var(--liquid-radius);
  cursor: pointer;
  transition: background var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-launcher .app-item:hover,
.liquid-launcher .app-item.selected {
  background: var(--liquid-surface-hover);
}

.liquid-launcher .app-item .app-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--liquid-radius-sm);
}

.liquid-launcher .app-item .app-name {
  font-size: var(--liquid-font-size-base);
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-text);
}

.liquid-launcher .app-item .app-desc {
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-tertiary);
}
```

### 7.6 Notifications

```css
.liquid-notification-stack {
  position: fixed;
  top: calc(var(--liquid-statusbar-height, 28px) + var(--liquid-space-3));
  right: var(--liquid-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--liquid-space-2);
  z-index: 2000;
  pointer-events: none;
}

.liquid-notification {
  width: 360px;
  padding: var(--liquid-space-3) var(--liquid-space-4);
  background: var(--liquid-bg-popover);
  backdrop-filter: blur(var(--liquid-glass-blur));
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-lg);
  box-shadow: var(--liquid-shadow-md);
  pointer-events: auto;
  animation: notification-enter var(--liquid-duration-slow) var(--liquid-ease-bounce);
}

.liquid-notification.urgent {
  border-left: 3px solid var(--liquid-error);
}

.liquid-notification .notif-header {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-2);
  margin-bottom: var(--liquid-space-1);
}

.liquid-notification .notif-app {
  font-size: var(--liquid-font-size-xs);
  font-weight: var(--liquid-font-weight-semibold);
  color: var(--liquid-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.liquid-notification .notif-time {
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-tertiary);
  margin-left: auto;
}

.liquid-notification .notif-title {
  font-size: var(--liquid-font-size-base);
  font-weight: var(--liquid-font-weight-semibold);
  color: var(--liquid-text);
}

.liquid-notification .notif-body {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-text-secondary);
  margin-top: var(--liquid-space-1);
}

@keyframes notification-enter {
  from {
    opacity: 0;
    transform: translateX(100px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
```

### 7.7 Tooltips

```css
.liquid-tooltip {
  padding: var(--liquid-space-1) var(--liquid-space-2);
  background: var(--liquid-bg-tooltip);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border: 1px solid var(--liquid-border-subtle);
  border-radius: var(--liquid-radius-sm);
  box-shadow: var(--liquid-shadow-sm);
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text);
  max-width: 250px;
  pointer-events: none;
}
```

### 7.8 Buttons

```css
.liquid-btn {
  height: 32px;
  padding: 0 var(--liquid-space-4);
  border-radius: var(--liquid-radius);
  border: 1px solid var(--liquid-border);
  background: var(--liquid-surface);
  color: var(--liquid-text);
  font-size: var(--liquid-font-size-sm);
  font-weight: var(--liquid-font-weight-medium);
  cursor: pointer;
  transition: all var(--liquid-duration-fast) var(--liquid-ease-default);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--liquid-space-2);
}

.liquid-btn:hover {
  background: var(--liquid-surface-hover);
  border-color: var(--liquid-border-strong);
}

.liquid-btn:active {
  background: var(--liquid-surface-active);
  transform: scale(0.98);
}

.liquid-btn:focus-visible {
  outline: 2px solid var(--liquid-border-focus);
  outline-offset: 2px;
}

.liquid-btn.primary {
  background: var(--liquid-accent);
  border-color: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
}

.liquid-btn.primary:hover {
  background: var(--liquid-accent-hover);
}

.liquid-btn.danger {
  color: var(--liquid-error);
}

.liquid-btn.danger:hover {
  background: rgba(255, 69, 58, 0.15);
}
```

### 7.9 Input Fields

```css
.liquid-input {
  height: 36px;
  padding: 0 var(--liquid-space-3);
  background: var(--liquid-surface);
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius);
  color: var(--liquid-text);
  font-size: var(--liquid-font-size-base);
  font-family: var(--liquid-font-sans);
  transition: border-color var(--liquid-duration-fast) var(--liquid-ease-default),
              box-shadow var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-input:focus {
  border-color: var(--liquid-border-focus);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.25);
  outline: none;
}

.liquid-input::placeholder {
  color: var(--liquid-text-tertiary);
}
```

### 7.10 Scrollbars

```css
.liquid-scrollbar::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.liquid-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.liquid-scrollbar::-webkit-scrollbar-thumb {
  background: var(--liquid-surface-hover);
  border-radius: var(--liquid-radius-full);
  border: 2px solid transparent;
  background-clip: content-box;
}

.liquid-scrollbar::-webkit-scrollbar-thumb:hover {
  background: var(--liquid-surface-active);
  background-clip: content-box;
}
```

### 7.11 Tiling

```css
/* Snap zone preview (shown when dragging a window near an edge/corner) */
.liquid-tile-preview {
  position: fixed;
  background: var(--liquid-tile-preview-bg, rgba(0, 122, 255, 0.15));
  border: var(--liquid-tile-preview-border, 2px solid var(--liquid-accent));
  border-radius: var(--liquid-radius);
  z-index: 900;
  pointer-events: none;
  animation: tile-preview-appear var(--liquid-duration-fast) var(--liquid-ease-out);
}

@keyframes tile-preview-appear {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* Tiled window adjustments */
.liquid-window.tiled {
  border-radius: var(--liquid-radius-sm);   /* reduced radius when tiled */
  box-shadow: var(--liquid-shadow-sm);      /* lighter shadow when tiled */
}

.liquid-window.tiled .titlebar {
  height: 32px;                              /* slightly shorter titlebar */
}

/* Tiling mode indicator */
.liquid-tile-indicator {
  position: fixed;
  top: calc(var(--liquid-statusbar-height, 28px) + var(--liquid-space-2));
  left: 50%;
  transform: translateX(-50%);
  padding: var(--liquid-space-1) var(--liquid-space-3);
  background: var(--liquid-bg-tooltip);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border: 1px solid var(--liquid-border-subtle);
  border-radius: var(--liquid-radius-full);
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-secondary);
  z-index: 998;
  pointer-events: none;
  opacity: 0.8;
}

/* Resize handle between tiles */
.liquid-tile-resize-handle {
  position: absolute;
  background: transparent;
  z-index: 901;
  cursor: col-resize;   /* or row-resize for horizontal splits */
}

.liquid-tile-resize-handle:hover {
  background: var(--liquid-accent);
  opacity: 0.5;
}
```

### 7.12 Tablet Mode

```css
/* Tablet mode root adjustments */
.liquid-tablet-mode {
  --liquid-statusbar-height: var(--liquid-tablet-statusbar-height, 40px);
  --liquid-dock-icon-size: var(--liquid-tablet-dock-icon-size, 56px);
}

/* Larger touch targets */
.liquid-tablet-mode .liquid-btn {
  min-width: var(--liquid-tablet-min-target, 56px);
  min-height: var(--liquid-tablet-min-target, 56px);
  font-size: var(--liquid-font-size-md);
}

.liquid-tablet-mode .liquid-input {
  height: 48px;
  font-size: var(--liquid-font-size-md);
}

/* Dock in tablet mode */
.liquid-tablet-mode .liquid-dock {
  border-radius: 0;
  left: 0;
  right: 0;
  bottom: 0;
  transform: none;
  justify-content: space-evenly;
  padding: var(--liquid-space-2) var(--liquid-space-4);
}

/* Status bar in tablet mode */
.liquid-tablet-mode .liquid-status-bar {
  height: var(--liquid-tablet-statusbar-height, 40px);
  font-size: var(--liquid-font-size-sm);
  padding: 0 var(--liquid-space-4);
}

/* Windows default to maximized in tablet mode */
.liquid-tablet-mode .liquid-window {
  border-radius: 0;
  box-shadow: none;
}

/* Launcher uses grid layout in tablet mode */
.liquid-tablet-mode .liquid-launcher {
  width: 100%;
  max-width: 100%;
  height: 100%;
  max-height: 100%;
  border-radius: 0;
}

.liquid-tablet-mode .liquid-launcher .results {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
  gap: var(--liquid-space-4);
  padding: var(--liquid-space-4);
}

.liquid-tablet-mode .liquid-launcher .app-item {
  flex-direction: column;
  text-align: center;
  padding: var(--liquid-space-3);
}

.liquid-tablet-mode .liquid-launcher .app-item .app-icon {
  width: 56px;
  height: 56px;
}
```

### 7.13 Login Screen

```css
/* ─── Login screen root ─────────────────────────────── */
.liquid-login {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 10000;
  overflow: hidden;
}

/* Background wallpaper layer */
.liquid-login .login-wallpaper {
  position: absolute;
  inset: 0;
  background-size: cover;
  background-position: center;
  z-index: 0;
}

/* Frosted glass overlay */
.liquid-login .login-frost {
  position: absolute;
  inset: 0;
  backdrop-filter: blur(var(--liquid-login-blur, 40px));
  background: rgba(0, 0, 0, 0.25);
  z-index: 1;
}

/* Ambient glow behind avatar */
.liquid-login .login-glow {
  position: absolute;
  width: 400px;
  height: 400px;
  border-radius: var(--liquid-radius-full);
  background: radial-gradient(
    circle,
    rgba(var(--liquid-accent-rgb), 0.08) 0%,
    transparent 70%
  );
  top: 50%;
  left: 50%;
  transform: translate(-50%, -60%);
  z-index: 2;
  pointer-events: none;
}

/* Optional floating particle layer */
.liquid-login .login-particles {
  position: absolute;
  inset: 0;
  z-index: 2;
  pointer-events: none;
  overflow: hidden;
}

.liquid-login .login-particle {
  position: absolute;
  border-radius: var(--liquid-radius-full);
  background: rgba(255, 255, 255, 0.04);
  animation: login-particle-drift 20s linear infinite;
}

@keyframes login-particle-drift {
  from {
    transform: translateY(110vh) translateX(0);
    opacity: 0;
  }
  10% { opacity: 1; }
  90% { opacity: 1; }
  to {
    transform: translateY(-10vh) translateX(40px);
    opacity: 0;
  }
}

/* ─── Content card (vertically centered) ───────────── */
.liquid-login .login-content {
  position: relative;
  z-index: 10;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--liquid-space-4);
  max-width: 400px;
  width: 100%;
  padding: var(--liquid-space-6);
}

/* ─── Clock & date ──────────────────────────────────── */
.liquid-login .login-clock {
  font-size: 72px;
  font-weight: 200;
  color: var(--liquid-text);
  line-height: 1;
  letter-spacing: -2px;
  text-shadow: 0 2px 20px rgba(0, 0, 0, 0.3);
  text-align: center;
}

.liquid-login .login-date {
  font-size: var(--liquid-font-size-md);
  font-weight: var(--liquid-font-weight-normal);
  color: var(--liquid-text-secondary);
  text-align: center;
  margin-top: calc(-1 * var(--liquid-space-2));
}

/* ─── User avatar ───────────────────────────────────── */
.liquid-login .login-avatar {
  width: var(--liquid-login-avatar-size, 120px);
  height: var(--liquid-login-avatar-size, 120px);
  border-radius: var(--liquid-radius-full);
  border: 3px solid rgba(255, 255, 255, 0.15);
  box-shadow:
    inset 0 0 12px rgba(255, 255, 255, 0.15),
    0 4px 24px rgba(0, 0, 0, 0.3);
  overflow: hidden;
  background: var(--liquid-surface);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  transition:
    transform var(--liquid-duration-normal) var(--liquid-ease-spring),
    box-shadow var(--liquid-duration-normal) var(--liquid-ease-default);
}

.liquid-login .login-avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* Initials fallback when no avatar image */
.liquid-login .login-avatar .avatar-initials {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 40px;
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-accent);
}

/* Avatar entrance animation */
@keyframes login-avatar-enter {
  from {
    opacity: 0;
    transform: scale(0.9);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.liquid-login .login-avatar {
  animation: login-avatar-enter var(--liquid-duration-normal) var(--liquid-ease-out) both;
}

/* ─── Username input field ─────────────────────── */
.liquid-login .login-username-input {
  width: 320px;
  height: 48px;
  padding: 0 var(--liquid-space-4);
  background: var(--liquid-surface);
  backdrop-filter: blur(20px);
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-full);
  color: var(--liquid-text);
  font-size: var(--liquid-font-size-md);
  outline: none;
  text-align: center;
  transition:
    border-color var(--liquid-duration-fast) var(--liquid-ease-default),
    box-shadow var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-username-input::placeholder {
  color: var(--liquid-text-tertiary);
}

.liquid-login .login-username-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.25);
}

/* Read-only username display (when pre-filled from profile with skip-to-credentials) */
.liquid-login .login-username-display {
  font-size: 20px;
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-text);
  text-align: center;
}

/* ─── Username & greeting ──────────────────────────── */
.liquid-login .login-username {
  font-size: 20px;
  font-weight: var(--liquid-font-weight-medium);
  color: var(--liquid-text);
  text-align: center;
}

.liquid-login .login-greeting {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-text-secondary);
  text-align: center;
  margin-top: calc(-1 * var(--liquid-space-2));
}

/* ─── Credential input ─────────────────────────────── */
.liquid-login .login-input-group {
  width: 320px;
  position: relative;
}

.liquid-login .login-input {
  width: 100%;
  height: 48px;
  padding: 0 var(--liquid-space-4);
  padding-right: 44px;                      /* space for toggle icon */
  background: var(--liquid-surface);
  backdrop-filter: blur(20px);
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-full);
  color: var(--liquid-text);
  font-size: var(--liquid-font-size-md);
  outline: none;
  transition:
    border-color var(--liquid-duration-fast) var(--liquid-ease-default),
    box-shadow var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-input::placeholder {
  color: var(--liquid-text-tertiary);
}

.liquid-login .login-input:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.25);
}

/* Password visibility toggle */
.liquid-login .login-input-toggle {
  position: absolute;
  right: var(--liquid-space-2);
  top: 50%;
  transform: translateY(-50%);
  width: 32px;
  height: 32px;
  border-radius: var(--liquid-radius-full);
  border: none;
  background: transparent;
  color: var(--liquid-text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: color var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-input-toggle:hover {
  color: var(--liquid-text-secondary);
}

/* Error state */
.liquid-login .login-input.error {
  border-color: var(--liquid-danger);
  animation: login-shake 300ms var(--liquid-ease-default);
}

@keyframes login-shake {
  0%   { transform: translateX(0); }
  20%  { transform: translateX(-8px); }
  40%  { transform: translateX(8px); }
  60%  { transform: translateX(-4px); }
  80%  { transform: translateX(4px); }
  100% { transform: translateX(0); }
}

.liquid-login .login-error-message {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-danger);
  text-align: center;
  margin-top: var(--liquid-space-1);
}

/* ─── PIN input mode ───────────────────────────────── */
.liquid-login .login-pin-group {
  display: flex;
  gap: var(--liquid-space-2);
  justify-content: center;
}

.liquid-login .login-pin-digit {
  width: 44px;
  height: 52px;
  text-align: center;
  font-size: 24px;
  font-weight: var(--liquid-font-weight-medium);
  background: var(--liquid-surface);
  backdrop-filter: blur(20px);
  border: 1px solid var(--liquid-border);
  border-radius: var(--liquid-radius-md);
  color: var(--liquid-text);
  outline: none;
  transition:
    border-color var(--liquid-duration-fast) var(--liquid-ease-default),
    box-shadow var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-pin-digit:focus {
  border-color: var(--liquid-accent);
  box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.25);
}

.liquid-login .login-pin-digit.filled {
  background: var(--liquid-surface-active);
}

/* ─── Smart card / security key prompt ─────────────── */
.liquid-login .login-device-prompt {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--liquid-space-3);
}

.liquid-login .login-device-prompt .device-icon {
  width: 64px;
  height: 64px;
  color: var(--liquid-accent);
  animation: login-device-pulse 2s ease-in-out infinite;
}

@keyframes login-device-pulse {
  0%, 100% {
    opacity: 0.7;
    filter: drop-shadow(0 0 8px rgba(var(--liquid-accent-rgb), 0.2));
  }
  50% {
    opacity: 1;
    filter: drop-shadow(0 0 16px rgba(var(--liquid-accent-rgb), 0.4));
  }
}

.liquid-login .login-device-prompt .device-text {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-text-secondary);
}

/* ─── Auth method indicators ───────────────────────── */
.liquid-login .login-auth-methods {
  display: flex;
  gap: var(--liquid-space-3);
  justify-content: center;
}

.liquid-login .login-auth-methods .auth-method-icon {
  width: 32px;
  height: 32px;
  border-radius: var(--liquid-radius-full);
  border: none;
  background: transparent;
  color: var(--liquid-text-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    color var(--liquid-duration-fast) var(--liquid-ease-default),
    background var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-auth-methods .auth-method-icon:hover {
  color: var(--liquid-text-secondary);
  background: var(--liquid-surface-hover);
}

.liquid-login .login-auth-methods .auth-method-icon.active {
  color: var(--liquid-accent);
}

/* ─── Sign-in button ───────────────────────────────── */
.liquid-login .login-submit {
  width: 320px;
  height: 48px;
  border: none;
  border-radius: var(--liquid-radius-full);
  background: var(--liquid-accent);
  color: var(--liquid-text-on-accent);
  font-size: var(--liquid-font-size-md);
  font-weight: var(--liquid-font-weight-medium);
  cursor: pointer;
  position: relative;
  overflow: hidden;
  transition:
    background var(--liquid-duration-fast) var(--liquid-ease-default),
    box-shadow var(--liquid-duration-fast) var(--liquid-ease-default),
    transform var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-submit:hover {
  background: var(--liquid-accent-hover);
  box-shadow: 0 4px 16px rgba(var(--liquid-accent-rgb), 0.3);
  transform: translateY(-1px);
}

.liquid-login .login-submit:active {
  background: var(--liquid-accent-active);
  box-shadow: 0 2px 8px rgba(var(--liquid-accent-rgb), 0.2);
  transform: translateY(0);
}

/* Loading spinner (glass ring) */
.liquid-login .login-submit.loading .btn-text {
  opacity: 0;
}

.liquid-login .login-submit.loading::after {
  content: "";
  position: absolute;
  width: 24px;
  height: 24px;
  top: 50%;
  left: 50%;
  margin: -12px 0 0 -12px;
  border: 2px solid rgba(255, 255, 255, 0.2);
  border-top-color: var(--liquid-text-on-accent);
  border-radius: var(--liquid-radius-full);
  animation: login-spinner 600ms linear infinite;
}

@keyframes login-spinner {
  to { transform: rotate(360deg); }
}

/* ─── Session resume indicator ─────────────────────── */
.liquid-login .login-resume {
  display: flex;
  align-items: center;
  gap: var(--liquid-space-2);
  padding: var(--liquid-space-2) var(--liquid-space-3);
  background: var(--liquid-surface);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  border: 1px solid var(--liquid-border-subtle);
  border-radius: var(--liquid-radius-md);
}

.liquid-login .login-resume .resume-text {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-text-secondary);
}

.liquid-login .login-resume .resume-thumbnail {
  width: 48px;
  height: 32px;
  border-radius: var(--liquid-radius-sm);
  overflow: hidden;
  filter: blur(2px);
  opacity: 0.6;
}

/* ─── Rate limit countdown ─────────────────────────── */
.liquid-login .login-cooldown {
  font-size: var(--liquid-font-size-sm);
  color: var(--liquid-text-tertiary);
  text-align: center;
}

/* ─── Server info strip (bottom-left) ──────────────── */
.liquid-login .login-server-info {
  position: absolute;
  bottom: var(--liquid-space-4);
  left: var(--liquid-space-4);
  z-index: 10;
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-tertiary);
  display: flex;
  align-items: center;
  gap: var(--liquid-space-2);
}

/* ─── Utility controls (bottom-right) ──────────────── */
.liquid-login .login-utilities {
  position: absolute;
  bottom: var(--liquid-space-4);
  right: var(--liquid-space-4);
  z-index: 10;
  display: flex;
  align-items: center;
  gap: var(--liquid-space-2);
}

.liquid-login .login-utilities .util-btn {
  width: 36px;
  height: 36px;
  border-radius: var(--liquid-radius-full);
  border: none;
  background: var(--liquid-surface);
  backdrop-filter: blur(var(--liquid-glass-blur-light));
  color: var(--liquid-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    background var(--liquid-duration-fast) var(--liquid-ease-default),
    color var(--liquid-duration-fast) var(--liquid-ease-default);
}

.liquid-login .login-utilities .util-btn:hover {
  background: var(--liquid-surface-hover);
  color: var(--liquid-text);
}

/* ─── Branding / custom logo ───────────────────────── */
.liquid-login .login-logo {
  max-height: 48px;
  max-width: 200px;
  object-fit: contain;
  opacity: 0.8;
}

.liquid-login .login-banner {
  position: absolute;
  bottom: var(--liquid-space-4);
  left: 50%;
  transform: translateX(-50%);
  z-index: 10;
  font-size: var(--liquid-font-size-xs);
  color: var(--liquid-text-tertiary);
  text-align: center;
  max-width: 60%;
}

/* ─── Cascade entrance animation ───────────────────── */
@keyframes login-cascade-in {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.liquid-login .login-content > * {
  animation: login-cascade-in var(--liquid-duration-normal) var(--liquid-ease-out) both;
}

.liquid-login .login-content > *:nth-child(1) { animation-delay: 0ms; }
.liquid-login .login-content > *:nth-child(2) { animation-delay: 50ms; }
.liquid-login .login-content > *:nth-child(3) { animation-delay: 100ms; }
.liquid-login .login-content > *:nth-child(4) { animation-delay: 150ms; }
.liquid-login .login-content > *:nth-child(5) { animation-delay: 200ms; }
.liquid-login .login-content > *:nth-child(6) { animation-delay: 250ms; }
.liquid-login .login-content > *:nth-child(7) { animation-delay: 300ms; }

/* ─── Auth success transition ──────────────────────── */
.liquid-login.auth-success {
  animation: login-dissolve 400ms var(--liquid-ease-in) forwards;
}

@keyframes login-dissolve {
  to {
    opacity: 0;
    filter: blur(8px);
    transform: scale(1.02);
  }
}

/* ─── High contrast overrides ──────────────────────── */
.liquid-high-contrast .liquid-login .login-frost {
  background: rgba(0, 0, 0, 0.85);
  backdrop-filter: none;
}

.liquid-high-contrast .liquid-login .login-avatar {
  border: 3px solid var(--liquid-text);
}

.liquid-high-contrast .liquid-login .login-input {
  border-width: 2px;
  background: rgba(0, 0, 0, 0.9);
}

.liquid-high-contrast .liquid-login .login-submit {
  border: 2px solid var(--liquid-text-on-accent);
}

/* ─── Reduced motion ───────────────────────────────── */
@media (prefers-reduced-motion: reduce) {
  .liquid-login .login-avatar,
  .liquid-login .login-content > *,
  .liquid-login.auth-success,
  .liquid-login .login-particle,
  .liquid-login .login-device-prompt .device-icon {
    animation: none !important;
  }
}
```

---

## 8) Iconography

### 8.1 Icon Style
- **Line icons** with consistent 1.5px stroke width.
- 24×24px grid with 2px padding.
- Rounded line caps and joins.
- Single color (inherits `currentColor`).

### 8.2 Icon Set
LiquidDE ships with a minimal icon set for shell UI:
- Window controls: close, minimize, maximize, restore.
- System: settings, search, power, lock, network, volume, brightness.
- Files: folder, file, image, video, archive.
- Actions: copy, paste, cut, delete, refresh, download, upload.
- Navigation: back, forward, up, home, menu.
- Status: connected, disconnected, warning, error, info.

### 8.3 Dock Icons
- Application icons are 128×128px source, rendered at dock icon size.
- Rounded rectangle mask applied (radius = 22% of icon size).
- Icons should follow a consistent style but are per-application.

---

## 9) Desktop & Wallpaper

### 9.1 Default Wallpaper
- Ships with a set of dark, abstract wallpapers that complement the glass aesthetic.
- Default: deep blue/purple gradient with subtle organic shapes.
- Wallpaper is pre-blurred for glass panel backdrops (separate cached layer).

### 9.2 Wallpaper Settings
```css
.liquid-desktop {
  background: var(--liquid-bg-desktop);
  background-image: url("/wallpapers/default-dark.jpg");
  background-size: cover;
  background-position: center;
}
```

Users can set wallpapers via:
- Settings app (browse/select).
- CSS override in `theme.css`.
- `session.toml`: `wallpaper = "/path/to/image.jpg"`.

### 9.3 Disabled Wallpaper Mode
For maximum performance:
```css
.liquid-desktop.no-wallpaper {
  background-image: none;
  background: var(--liquid-bg-desktop);
}
```

When wallpaper is disabled, glass surfaces use the solid `--liquid-bg-desktop` color as the backdrop instead of a blurred wallpaper — no blur computation needed.

---

## 10) Accessibility

### 10.1 High Contrast Mode

```css
.liquid-high-contrast {
  --liquid-bg-window:        rgba(0, 0, 0, 0.95);
  --liquid-bg-dock:          rgba(0, 0, 0, 0.95);
  --liquid-border:           rgba(255, 255, 255, 0.50);
  --liquid-border-strong:    rgba(255, 255, 255, 0.80);
  --liquid-text:             #FFFFFF;
  --liquid-text-secondary:   #FFFFFF;
  --liquid-glass-blur:       0px;          /* disable blur */
  --liquid-glass-noise:      0;            /* disable noise */
  --liquid-specular-enabled: false;
}

.liquid-high-contrast .liquid-window .titlebar {
  border-bottom: 2px solid var(--liquid-border-strong);
}

.liquid-high-contrast .liquid-btn:focus-visible {
  outline-width: 3px;
}
```

### 10.2 Large Text Mode

```css
.liquid-large-text {
  --liquid-font-size-xs:   13px;
  --liquid-font-size-sm:   15px;
  --liquid-font-size-base: 17px;
  --liquid-font-size-md:   19px;
  --liquid-font-size-lg:   24px;
}
```

### 10.3 Focus Indicators
- All interactive elements have visible focus indicators.
- Focus ring: 2px solid accent color, 2px offset.
- Never rely on color alone for state communication.

---

## 11) CSS Theming Guide

### 11.1 Creating a Custom Theme

1. Create `~/.config/liquidde/theme.css`.
2. Override any CSS variable or class.
3. Changes apply immediately (live reload).

Example — "Warm Amber" theme:
```css
:root {
  --liquid-accent:           #FF9F0A;
  --liquid-accent-hover:     #E08C00;
  --liquid-bg-desktop:       #1A1510;
  --liquid-bg-window:        rgba(40, 30, 20, 0.75);
  --liquid-bg-dock:          rgba(40, 30, 20, 0.70);
  --liquid-glass-tint:       rgba(255, 159, 10, 0.05);
  --liquid-border:           rgba(255, 159, 10, 0.15);
}
```

Example — "Flat Minimal" theme (no glass):
```css
:root {
  --liquid-glass-blur:       0px;
  --liquid-glass-noise:      0;
  --liquid-specular-enabled: false;
  --liquid-bg-window:        #2D2D2D;
  --liquid-bg-dock:          #1E1E1E;
  --liquid-bg-panel:         #252525;
  --liquid-shadow-md:        none;
  --liquid-shadow-lg:        none;
  --liquid-border:           1px solid #444444;
  --liquid-radius:           4px;
  --liquid-radius-lg:        6px;
  --liquid-radius-xl:        8px;
}
```

### 11.2 CSS File Load Order
1. `/etc/liquidde/theme.css` — system base theme (Liquid Glass defaults).
2. `~/.config/liquidde/theme.css` — user overrides.
3. User overrides take precedence (CSS cascade).

### 11.3 Available CSS Classes (Complete Reference)

#### Desktop
| Selector | Element |
|----------|---------|
| `.liquid-desktop` | Desktop background/root |
| `.liquid-desktop.no-wallpaper` | Desktop with wallpaper disabled |

#### Dock
| Selector | Element |
|----------|---------|
| `.liquid-dock` | Dock container |
| `.liquid-dock.position-bottom` | Bottom-positioned dock |
| `.liquid-dock.position-left` | Left-positioned dock |
| `.liquid-dock.position-right` | Right-positioned dock |
| `.liquid-dock.position-top` | Top-positioned dock |
| `.liquid-dock.auto-hide` | Dock in auto-hide mode |
| `.liquid-dock .dock-item` | Individual dock icon |
| `.liquid-dock .dock-item.active` | Running application icon |
| `.liquid-dock .dock-item.focused` | Focused application icon |
| `.liquid-dock .dock-item:hover` | Hovered dock icon |
| `.liquid-dock .dock-item .icon` | Icon image inside dock item |
| `.liquid-dock .dock-item .badge` | Notification badge |
| `.liquid-dock .dock-separator` | Separator between dock sections |

#### Windows
| Selector | Element |
|----------|---------|
| `.liquid-window` | Window container |
| `.liquid-window.focused` | Focused window |
| `.liquid-window.maximized` | Maximized window |
| `.liquid-window.tiled` | Tiled/snapped window |
| `.liquid-window.tiled.master` | Master tile window |
| `.liquid-window .titlebar` | Window title bar |
| `.liquid-window .title` | Window title text |
| `.liquid-window .window-btn` | Window control button (generic) |
| `.liquid-window .close-btn` | Close button |
| `.liquid-window .minimize-btn` | Minimize button |
| `.liquid-window .maximize-btn` | Maximize button |
| `.liquid-window .content` | Window content area |

#### Tiling
| Selector | Element |
|----------|---------|
| `.liquid-tile-preview` | Snap zone preview overlay (shown when dragging) |
| `.liquid-tile-indicator` | Tiling mode indicator (shown when tiling is active) |
| `.liquid-tile-gap` | Gap between tiled windows (styled via `--liquid-tile-gap`) |
| `.liquid-tile-resize-handle` | Drag handle between adjacent tiles |

#### Status Bar
| Selector | Element |
|----------|---------|
| `.liquid-status-bar` | Status bar container |
| `.liquid-status-bar .status-left` | Left section |
| `.liquid-status-bar .status-center` | Center section |
| `.liquid-status-bar .status-right` | Right section |
| `.liquid-status-bar .clock` | Clock display |
| `.liquid-status-bar .connection-indicator` | Connection status |
| `.liquid-status-bar .connection-dot` | Status dot (colored) |
| `.liquid-status-bar .tray-area` | System tray area |

#### Launcher
| Selector | Element |
|----------|---------|
| `.liquid-launcher` | Launcher overlay |
| `.liquid-launcher .search-bar` | Search bar area |
| `.liquid-launcher .search-input` | Search text input |
| `.liquid-launcher .results` | Results list |
| `.liquid-launcher .app-item` | Application entry |
| `.liquid-launcher .app-item.selected` | Selected/highlighted entry |
| `.liquid-launcher .app-icon` | Application icon |
| `.liquid-launcher .app-name` | Application name |
| `.liquid-launcher .app-desc` | Application description |
| `.liquid-launcher .category` | Category header |

#### Notifications
| Selector | Element |
|----------|---------|
| `.liquid-notification-stack` | Notification container |
| `.liquid-notification` | Individual notification |
| `.liquid-notification.urgent` | Urgent notification |
| `.liquid-notification .notif-header` | Notification header |
| `.liquid-notification .notif-app` | Source application name |
| `.liquid-notification .notif-time` | Timestamp |
| `.liquid-notification .notif-title` | Notification title |
| `.liquid-notification .notif-body` | Notification body text |

#### Panels & Popovers
| Selector | Element |
|----------|---------|
| `.liquid-panel` | Generic panel |
| `.liquid-panel.glass` | Glass-styled panel |
| `.liquid-popover` | Popover/dropdown |
| `.liquid-modal` | Modal dialog |
| `.liquid-modal .modal-backdrop` | Modal backdrop overlay |
| `.liquid-tooltip` | Tooltip |

#### Controls
| Selector | Element |
|----------|---------|
| `.liquid-btn` | Button |
| `.liquid-btn.primary` | Primary action button |
| `.liquid-btn.danger` | Destructive action button |
| `.liquid-btn.icon-only` | Icon-only button |
| `.liquid-input` | Text input |
| `.liquid-select` | Select/dropdown |
| `.liquid-checkbox` | Checkbox |
| `.liquid-toggle` | Toggle switch |
| `.liquid-slider` | Slider/range |
| `.liquid-scrollbar` | Scrollbar styling container |

#### System
| Selector | Element |
|----------|---------|
| `.liquid-theme-dark` | Dark theme root |
| `.liquid-theme-light` | Light theme root |
| `.liquid-high-contrast` | High contrast mode |
| `.liquid-large-text` | Large text mode |
| `.liquid-performance-minimal` | Reduced effects mode |
| `.liquid-glass` | Generic glass effect mixin |
| `.liquid-tablet-mode` | Tablet mode root (applied when tablet mode enabled) |

#### Tablet Mode
| Selector | Element |
|----------|---------|
| `.liquid-tablet-mode` | Root class when tablet mode is active |
| `.liquid-tablet-mode .liquid-dock` | Dock adapted for touch (larger icons, bottom bar) |
| `.liquid-tablet-mode .liquid-status-bar` | Taller status bar (40px) with larger touch areas |
| `.liquid-tablet-mode .liquid-launcher` | Grid layout launcher with larger icons |
| `.liquid-tablet-mode .liquid-window` | Windows default to maximized |
| `.liquid-tablet-mode .liquid-btn` | Buttons with larger minimum touch targets (56×56px) |
| `.liquid-tablet-mode .liquid-input` | Taller input fields for touch |
| `.liquid-tablet-mode .liquid-notification` | Notifications accessible via swipe gesture |

#### Login Screen
| Selector | Element |
|----------|---------|
| `.liquid-login` | Login screen root (full-screen overlay) |
| `.liquid-login .login-wallpaper` | Background wallpaper layer |
| `.liquid-login .login-frost` | Frosted glass blur overlay |
| `.liquid-login .login-glow` | Ambient radial glow behind avatar |
| `.liquid-login .login-particles` | Optional floating particle container |
| `.liquid-login .login-particle` | Individual floating particle |
| `.liquid-login .login-content` | Centered content card |
| `.liquid-login .login-clock` | Large time display |
| `.liquid-login .login-date` | Date display below clock |
| `.liquid-login .login-avatar` | Circular user avatar with glass ring |
| `.liquid-login .login-avatar .avatar-initials` | Initials fallback inside avatar |
| `.liquid-login .login-username-input` | Username text input field |
| `.liquid-login .login-username-display` | Read-only username display (pre-filled from profile) |
| `.liquid-login .login-username` | Username display |
| `.liquid-login .login-greeting` | Time-of-day greeting |
| `.liquid-login .login-input-group` | Credential input container |
| `.liquid-login .login-input` | Password/text input field |
| `.liquid-login .login-input.error` | Input in error state (shake animation) |
| `.liquid-login .login-input-toggle` | Password show/hide eye icon |
| `.liquid-login .login-error-message` | Error message text |
| `.liquid-login .login-pin-group` | PIN digit input container |
| `.liquid-login .login-pin-digit` | Individual PIN digit box |
| `.liquid-login .login-pin-digit.filled` | PIN digit with entered value |
| `.liquid-login .login-device-prompt` | Smart card / security key prompt |
| `.liquid-login .login-device-prompt .device-icon` | Pulsing device icon |
| `.liquid-login .login-device-prompt .device-text` | Device prompt instruction text |
| `.liquid-login .login-auth-methods` | Auth method icon row |
| `.liquid-login .login-auth-methods .auth-method-icon` | Individual auth method icon |
| `.liquid-login .login-auth-methods .auth-method-icon.active` | Active/selected auth method |
| `.liquid-login .login-submit` | Sign-in button |
| `.liquid-login .login-submit.loading` | Sign-in button in loading state |
| `.liquid-login .login-resume` | Session resume indicator chip |
| `.liquid-login .login-resume .resume-thumbnail` | Blurred session thumbnail |
| `.liquid-login .login-cooldown` | Rate limit countdown text |
| `.liquid-login .login-server-info` | Server info strip (bottom-left) |
| `.liquid-login .login-utilities` | Utility controls container (bottom-right) |
| `.liquid-login .login-utilities .util-btn` | Utility control button (power, accessibility, language) |
| `.liquid-login .login-logo` | Custom organization logo |
| `.liquid-login .login-banner` | Legal/compliance banner text |
| `.liquid-login.auth-success` | Login screen dissolving after auth success |

### 11.4 Custom Properties Reference (Complete)

| Property | Default | Description |
|----------|---------|-------------|
| `--liquid-accent` | `#007AFF` | Primary accent color |
| `--liquid-accent-hover` | `#0066DD` | Accent hover state |
| `--liquid-accent-active` | `#0055BB` | Accent active state |
| `--liquid-text` | `#FFFFFF` | Primary text color |
| `--liquid-text-secondary` | `rgba(255,255,255,0.70)` | Secondary text |
| `--liquid-text-tertiary` | `rgba(255,255,255,0.50)` | Tertiary text |
| `--liquid-text-disabled` | `rgba(255,255,255,0.30)` | Disabled text |
| `--liquid-bg-desktop` | `#1C1C2E` | Desktop background |
| `--liquid-bg-window` | `rgba(30,30,50,0.75)` | Window background |
| `--liquid-bg-dock` | `rgba(30,30,50,0.70)` | Dock background |
| `--liquid-bg-panel` | `rgba(20,20,40,0.85)` | Panel background |
| `--liquid-bg-popover` | `rgba(40,40,60,0.90)` | Popover background |
| `--liquid-bg-modal` | `rgba(20,20,40,0.95)` | Modal background |
| `--liquid-bg-tooltip` | `rgba(60,60,80,0.95)` | Tooltip background |
| `--liquid-surface` | `rgba(255,255,255,0.08)` | Interactive surface |
| `--liquid-surface-hover` | `rgba(255,255,255,0.12)` | Surface hover |
| `--liquid-surface-active` | `rgba(255,255,255,0.16)` | Surface active |
| `--liquid-border` | `rgba(255,255,255,0.12)` | Default border |
| `--liquid-border-strong` | `rgba(255,255,255,0.20)` | Strong border |
| `--liquid-border-subtle` | `rgba(255,255,255,0.06)` | Subtle border |
| `--liquid-border-focus` | `var(--liquid-accent)` | Focus border |
| `--liquid-shadow-sm` | `0 2px 8px rgba(0,0,0,0.20)` | Small shadow |
| `--liquid-shadow-md` | `0 8px 32px rgba(0,0,0,0.30)` | Medium shadow |
| `--liquid-shadow-lg` | `0 16px 64px rgba(0,0,0,0.40)` | Large shadow |
| `--liquid-shadow-dock` | `0 0 40px rgba(0,0,0,0.50)` | Dock shadow |
| `--liquid-glass-blur` | `20px` | Standard glass blur radius |
| `--liquid-glass-blur-heavy` | `40px` | Heavy glass blur radius |
| `--liquid-glass-blur-light` | `10px` | Light glass blur radius |
| `--liquid-glass-noise` | `0.03` | Frosted noise opacity |
| `--liquid-glass-tint` | `rgba(255,255,255,0.05)` | Glass tint color |
| `--liquid-glass-specular` | `rgba(255,255,255,0.10)` | Specular highlight color |
| `--liquid-radius` | `10px` | Default border radius |
| `--liquid-radius-sm` | `6px` | Small radius |
| `--liquid-radius-md` | `12px` | Medium radius |
| `--liquid-radius-lg` | `16px` | Large radius |
| `--liquid-radius-xl` | `20px` | Extra large radius |
| `--liquid-radius-2xl` | `24px` | 2× extra large radius |
| `--liquid-radius-full` | `9999px` | Full/pill radius |
| `--liquid-dock-icon-size` | `48px` | Dock icon size |
| `--liquid-dock-height` | `64px` | Dock total height |
| `--liquid-statusbar-height` | `28px` | Status bar height |
| `--liquid-font-sans` | `"Inter", system-ui, sans-serif` | Sans font stack |
| `--liquid-font-mono` | `"JetBrains Mono", monospace` | Mono font stack |
| `--liquid-font-size-base` | `14px` | Base font size |
| `--liquid-duration-fast` | `100ms` | Fast animation |
| `--liquid-duration-normal` | `200ms` | Normal animation |
| `--liquid-duration-slow` | `300ms` | Slow animation |
| `--liquid-ease-default` | `cubic-bezier(0.4, 0, 0.2, 1)` | Default easing |
| `--liquid-tile-gap` | `8px` | Gap between tiled windows |
| `--liquid-tile-outer-gap` | `8px` | Gap between tiles and screen edges |
| `--liquid-tile-preview-bg` | `rgba(0,122,255,0.15)` | Snap zone preview background |
| `--liquid-tile-preview-border` | `2px solid var(--liquid-accent)` | Snap zone preview border |
| `--liquid-tablet-min-target` | `56px` | Minimum touch target in tablet mode |
| `--liquid-tablet-statusbar-height` | `40px` | Status bar height in tablet mode |
| `--liquid-tablet-dock-icon-size` | `56px` | Dock icon size in tablet mode |
| `--liquid-login-blur` | `40px` | Login screen frosted glass blur intensity |
| `--liquid-login-avatar-size` | `120px` | Login screen avatar diameter |

---

## 12) Client-Side Design (LiquidClient)

The LiquidClient application uses the same Liquid Glass design language:

### 12.1 Connection Dialog
- Centered glass panel on a dark gradient background.
- Server address input, recent servers list, connect button.
- Uses standard `.liquid-glass`, `.liquid-input`, `.liquid-btn` styles.

### 12.2 Client Window Chrome
- Custom title bar using `.liquid-window .titlebar` styles.
- Window controls match the DE window buttons.
- Latency and status indicators integrated into the title bar.

### 12.3 Fullscreen Toolbar
- Uses `.liquid-glass` with `heavy` blur level.
- Compact layout with icon buttons.
- Auto-hide with smooth slide animation.

### 12.4 Stream Overlay
- Semi-transparent panel (`.liquid-panel.glass` with reduced opacity).
- Monospace font for statistics.
- Minimal visual footprint to not obstruct the session.

### 12.5 Settings Dialog
- Full-height side panel or modal.
- Sectioned with tabs.
- Glass-themed throughout.

---

## 13) Performance Considerations for Design

### 13.1 Render Cost by Element
| Element | Glass | Blur | Shadow | Specular | Total Cost |
|---------|-------|------|--------|----------|------------|
| Desktop wallpaper | — | Cached | — | — | Near zero (cached) |
| Status bar | Light | Cached | None | None | Very low |
| Dock | Standard | Cached | Yes | Optional | Low |
| Window chrome | Standard | Cached | Yes | Optional | Low |
| Window content | None | None | None | None | Zero (app-rendered) |
| Launcher | Heavy | Computed | Yes | None | Medium (on-demand) |
| Notification | Standard | Computed | Yes | None | Low (small area) |
| Modal backdrop | None | None | None | None | Zero (solid overlay) |

### 13.2 Cache Strategy
- **Always cached**: wallpaper blur, status bar, dock background.
- **Cached until invalidated**: window chrome (invalidated on focus change), notification list.
- **Computed on-demand**: launcher (only when open), tooltips (tiny area).
- **Never cached**: window content (application-rendered), specular highlights (throttled recomputation).

### 13.3 Degradation Strategy
- Effect budget determines which tier of glass rendering is active.
- Fallback chain: full glass → reduced glass → tinted solid → flat solid.
- At each tier, shadows may also be simplified or removed.
- CSS classes (`.liquid-performance-*`) allow theme authors to design for each tier.

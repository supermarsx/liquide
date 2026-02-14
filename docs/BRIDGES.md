# LiquiDE GTK & Qt Bridges

## Overview

LiquiDE provides bridge crates that enable seamless interop between the native
LiquiDE widget system and GTK/Qt toolkits. This allows:

1. **GTK/Qt apps running inside LiquiDE** — apps render as native LiquiDE
   windows, receiving input through the LiquiDE event system
2. **LiquiDE widgets embedded in GTK/Qt apps** — LiquiDE controls can appear
   inside traditional toolkit applications
3. **Unified clipboard, DnD, and IME** — shared platform services

## GTK Bridge (`liquide-bridge-gtk`)

### Architecture

```
GTK Application
    │
    ▼
liquide-bridge-gtk
    ├── GtkWindowBridge    ←→  LiquiDE Window
    ├── GtkWidgetAdapter   ←→  LiquiDE Widget
    ├── GtkEventTranslator ←→  LiquiDE Event
    ├── GtkClipboardBridge ←→  LiquiDE Clipboard
    ├── GtkDndBridge       ←→  LiquiDE DnD
    └── GtkA11yBridge      ←→  LiquiDE Accessibility
```

### Event Translation

| GTK Event | LiquiDE Event |
|-----------|---------------|
| `GdkEventButton` | `Event::PointerDown/Up` |
| `GdkEventMotion` | `Event::PointerMove` |
| `GdkEventKey` | `Event::KeyDown/Up` |
| `GdkEventScroll` | `Event::Scroll` |
| `GdkEventTouch` | `Event::Touch*` |
| `GdkEventFocus` | `Event::FocusIn/Out` |
| `GdkEventConfigure` | Window resize/move |

### Window Lifecycle

```
GtkWindowBridge::new(title, size)
    │
    ├── Creates LiquiDE Window
    ├── Creates GDK surface for rendering
    ├── Installs event filters
    └── Maps GTK widget tree → LiquiDE a11y tree
```

## Qt Bridge (`liquide-bridge-qt`)

### Architecture

```
Qt Application
    │
    ▼
liquide-bridge-qt
    ├── QtWindowBridge     ←→  LiquiDE Window
    ├── QtWidgetAdapter    ←→  LiquiDE Widget
    ├── QtEventTranslator  ←→  LiquiDE Event
    ├── QtClipboardBridge  ←→  LiquiDE Clipboard
    ├── QtDndBridge        ←→  LiquiDE DnD
    └── QtA11yBridge       ←→  LiquiDE Accessibility
```

### Event Translation

| Qt Event | LiquiDE Event |
|----------|---------------|
| `QMouseEvent` | `Event::PointerDown/Up/Move` |
| `QKeyEvent` | `Event::KeyDown/Up` |
| `QWheelEvent` | `Event::Scroll` |
| `QTouchEvent` | `Event::Touch*` |
| `QFocusEvent` | `Event::FocusIn/Out` |
| `QResizeEvent` | Window resize |
| `QInputMethodEvent` | IME composition |

### Platform Plugin

The Qt bridge can act as a Qt Platform Abstraction (QPA) plugin, making
LiquiDE a full Qt platform backend. This means any Qt app launches natively
in LiquiDE without modification.

## Threading Safety

Both bridges marshal events between the toolkit's main thread and LiquiDE's
event loop via a lock-free channel:

```
GTK/Qt Thread                LiquiDE Thread
    │                             │
    ├── gtk_event ──channel──►    │
    │                             ├── translate event
    │                             ├── dispatch to widget
    │                             ├── collect paint commands
    │    ◄──channel── render ◄────┤
    │                             │
```

## Clipboard Bridging

A unified clipboard manager sits between the toolkit and LiquiDE:

```
GTK Clipboard (GdkClipboard)  ←→  LiquiDE ClipboardManager  ←→  Remote Client
Qt Clipboard (QClipboard)     ←→  LiquiDE ClipboardManager  ←→  Remote Client
```

Supported formats: text/plain, text/html, image/png, application/json,
and custom MIME types via negotiation.

## Drag & Drop Bridging

The DnD bridge translates between toolkit-specific drag protocols and
LiquiDE's `DragSource`/`DropTarget` model:

```
Source Widget (GTK/Qt)
    │
    ▼ DragEnter → translated to LiquiDE DragEnter
DropTarget (LiquiDE widget or vice versa)
    │
    ▼ Drop → data transfer via LiquiDE DnD protocol
```

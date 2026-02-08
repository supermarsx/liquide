# LiquiDE — Built-in Applications & Addons Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-client.md](spec-client.md) (client), [spec-design.md](spec-design.md) (theming), [spec-interop.md](spec-interop.md) (desktop standards), [spec-accessibility.md](spec-accessibility.md) (accessibility)

---

## 1) Overview

LiquiDE ships a curated set of built-in applications that provide essential desktop functionality out of the box. These applications are designed as lightweight, Liquid Glass-themed utilities — not full-featured professional tools, but competent defaults that cover everyday needs.

### Design Principles

- **Native Liquid Glass**: all apps are written in Rust using LiquiDE's own UI toolkit (`liquid-ui`), which renders the Liquid Glass design language directly via the compositor. Glass blur, translucency, and depth effects are native — no external GUI toolkit required.
- **Remote-first**: all apps run on the **server** in a remote LiquiDE session. Their UI is streamed to the client like any other application. Where beneficial, specific apps support client-side offload (terminal, text editor) for reduced latency.
- **Lightweight**: each app targets < 50 MB RSS memory at idle, < 200ms startup time.
- **Accessible**: all apps expose full AT-SPI2 accessibility trees, support keyboard navigation, and respect high-contrast / reduced-motion / text-scaling preferences.
- **Policy-controlled**: enterprise administrators can hide, restrict, or replace any built-in app via the policy engine.

### Shared Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `liquid-ui` | (bundled) | LiquiDE native Rust UI toolkit (Liquid Glass rendering, layout, input) |
| tree-sitter | 0.20+ | Syntax highlighting (editor, terminal) |
| PipeWire | 1.0+ | Audio (system monitor alerts) |
| zbus | 4.0+ | D-Bus communication (Rust native) |

---

## 2) File Manager — `liquid-files`

### 2.1 Overview

A modern, fast file manager with dual-pane support, thumbnail previews, and full Liquid Glass integration. Positioned as a Nautilus/Dolphin-class file manager.

### 2.2 UI Layout

```
┌────────────────────────────────────────────────────────────┐
│ ← → ↑  /home/user/Documents           🔍 Search    ☰ Menu│
├──────────┬─────────────────────────────────────────────────┤
│ ★ Home   │  📁 Projects    📁 Reports    📄 notes.md     │
│ ★ Docs   │  📁 Photos      📄 todo.txt   📄 budget.xlsx  │
│ ★ Down   │  📷 photo.jpg   📄 readme.md  📁 Archive      │
│ 🔌 Drives│                                                │
│ 🗑 Trash │                                                │
│ 🌐 Net   │ ─────────────────────────────────────────────  │
│          │  12 items, 3 selected (14.2 MB)                │
└──────────┴─────────────────────────────────────────────────┘
```

### 2.3 Key Features

| Feature | Description |
|---------|-------------|
| **Navigation** | Breadcrumb path bar (click to type URI), back/forward/up buttons, address bar with autocomplete |
| **View modes** | Icon grid, detailed list, compact list, column view (Miller columns) |
| **Dual pane** | Optional split view (F3 toggle), drag-and-drop between panes |
| **Thumbnails** | Generated for images (PNG, JPEG, WebP, SVG), videos (first frame), PDFs (first page), fonts (preview), archives (icon) |
| **Search** | Recursive search with filename, content, MIME type, size, date filters. Integrates with Tracker/locate if available |
| **Bookmarks** | Sidebar bookmarks (user-managed), drag folders to sidebar to pin |
| **Trash** | XDG Trash spec compliance, restore to original location, empty trash |
| **Network** | SMB/CIFS, FTP/SFTP, WebDAV, NFS shares via GVfs. Sidebar "Network" entry shows discovered shares |
| **Archives** | Browse archive contents inline (delegates to `liquid-archive`). Create archives from selection (right-click → Compress) |
| **Bulk rename** | Select multiple files → rename with pattern (sequential numbering, find/replace, regex, date insertion) |
| **File preview** | Quick preview panel (Space key) — renders images, text file head, PDF first page, audio metadata, video thumbnail |
| **Permissions** | Properties dialog shows Unix permissions, ownership, extended attributes, SELinux context |
| **Drag-and-drop** | Full DnD within/between windows, to/from dock, to/from external apps |
| **Undo** | Undo last file operation (move, rename, trash, copy) with toast notification |
| **Tabs** | Multiple tabs per window (`Ctrl+T` new tab), drag tabs between windows |
| **Hidden files** | Toggle with `Ctrl+H` |
| **Progress** | Long operations (copy, move, delete large sets) show a progress dialog with cancel, pause, skip |

### 2.4 Remote Session Considerations

- The file manager browses the **server** filesystem by default.
- If `portals.file_chooser.allow_client_browsing = true` is set, a "Local Machine" sidebar entry allows browsing the client filesystem via the LiquiDE file transfer channel. Files selected from the client side are transferred to a server-side staging directory.
- Thumbnail generation occurs on the server. Network-mounted shares (SMB, NFS) are server-relative.

### 2.5 Configuration

```toml
# ~/.config/liquide/liquid-files.toml

[file_manager]
default_view = "icon-grid"         # icon-grid, list, compact, columns
sort_by = "name"                   # name, modified, size, type
sort_order = "ascending"
show_hidden = false
single_click_activate = false      # true = single click opens, false = double click
thumbnail_size = 64                # 32, 48, 64, 96, 128
confirm_trash = true
confirm_delete = true              # permanent delete (Shift+Delete)

[file_manager.sidebar]
show_bookmarks = true
show_devices = true
show_network = true
show_trash = true
show_recent = true

[file_manager.thumbnails]
enabled = true
max_file_size_mb = 50              # skip thumbnails for files larger than this
video_thumbnails = true
pdf_thumbnails = true
```

### 2.6 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+L` | Focus address bar |
| `Ctrl+F` | Open search |
| `Ctrl+H` | Toggle hidden files |
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `F2` | Rename selected |
| `F3` | Toggle dual pane |
| `F5` | Refresh |
| `Space` | Quick preview |
| `Delete` | Move to trash |
| `Shift+Delete` | Permanent delete |
| `Ctrl+Z` | Undo last operation |
| `Alt+Enter` | Properties dialog |
| `Backspace` | Go up one directory |

### 2.7 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Launch file manager | `apps.file_manager.enabled` | `true` |
| Access network shares | `apps.file_manager.network` | `true` |
| Delete files permanently | `apps.file_manager.permanent_delete` | `true` |
| Access removable drives | `apps.file_manager.removable_media` | `true` |
| Bulk rename | `apps.file_manager.bulk_rename` | `true` |
| Browse client filesystem | (uses `portals.file_chooser.allow_client_browsing`) | `false` |

### 2.8 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Files
GenericName=File Manager
Comment=Browse and manage files
Exec=liquid-files %U
Icon=system-file-manager
Categories=System;FileManager;
MimeType=inode/directory;
Keywords=file;folder;manager;browse;directory;
StartupNotify=true
```

---

## 3) Text Editor — `liquid-edit`

### 3.1 Overview

A lightweight text editor for quick file editing. Tree-sitter syntax highlighting, regex search, multi-tab — positioned as a gedit/Notepad++ equivalent, not an IDE.

### 3.2 Key Features

| Feature | Description |
|---------|-------------|
| **Syntax highlighting** | tree-sitter grammars for 50+ languages (auto-detected from extension/shebang/MIME) |
| **Line numbers** | Toggleable gutter with line numbers and current-line highlight |
| **Search & replace** | `Ctrl+F` find, `Ctrl+H` replace, regex mode, case-sensitive toggle, whole-word toggle, match count |
| **Word wrap** | Toggle soft wrap at window edge or specified column (80, 100, 120) |
| **Encoding** | Detects encoding (UTF-8 default), shows encoding in status bar, re-open with different encoding |
| **Line endings** | Detects and shows LF/CRLF/CR, convert between them |
| **Tabs** | Multiple files open in tabs, drag to reorder, `Ctrl+Tab` to switch |
| **Auto-save** | Configurable auto-save interval (default: 30 seconds), auto-save on focus loss |
| **Minimap** | Optional code minimap scrollbar |
| **Indent** | Auto-indent, tab/spaces toggle, configurable tab width (2/4/8) |
| **Bracket matching** | Highlight matching brackets/parentheses/braces |
| **Go to line** | `Ctrl+G` go to line number |
| **Printing** | Print with syntax highlighting preservation, configurable header/footer |
| **Large files** | Handles files up to 100 MB without lag (streaming read, viewport rendering) |

### 3.3 Configuration

```toml
# ~/.config/liquide/liquid-edit.toml

[editor]
font = "monospace 12"
tab_width = 4
insert_spaces = true          # spaces instead of tabs
word_wrap = false
wrap_column = 0               # 0 = window edge, else fixed column
show_line_numbers = true
show_minimap = false
highlight_current_line = true
bracket_matching = true
auto_save = true
auto_save_interval_sec = 30
encoding_default = "utf-8"
line_ending_default = "lf"    # lf, crlf
theme = "liquid-dark"         # follows system dark/light mode by default
show_whitespace = false
```

### 3.4 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | New file |
| `Ctrl+O` | Open file |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save As |
| `Ctrl+G` | Go to line |
| `Ctrl+D` | Duplicate line |
| `Ctrl+Shift+K` | Delete line |
| `Ctrl+/` | Toggle line comment |
| `Ctrl+]` / `Ctrl+[` | Indent / unindent |
| `Ctrl+Shift+P` | Command palette |

### 3.5 Remote Session Considerations

- The editor runs on the server and opens server-side files.
- When client-side offload is available (see spec.md §9, spec-client.md), the text editor can be offloaded to client-side rendering for sub-frame editing latency. In this mode, the server sends document state + incremental diffs, and the client renders locally using cached fonts.

### 3.6 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Launch editor | `apps.editor.enabled` | `true` |
| Open files > 10 MB | `apps.editor.large_files` | `true` |

### 3.7 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Text Editor
GenericName=Text Editor
Comment=Edit text files
Exec=liquid-edit %U
Icon=text-editor
Categories=Utility;TextEditor;
MimeType=text/plain;text/x-csrc;text/x-python;application/json;application/xml;text/markdown;text/html;text/css;application/javascript;text/x-rust;
Keywords=text;editor;code;edit;notepad;
StartupNotify=true
```

---

## 4) Terminal Emulator — `liquid-terminal`

### 4.1 Overview

A GPU-accelerated terminal emulator with true color support, tabs, split panes, and ligature rendering. Built on VTE or a custom Rust terminal renderer.

### 4.2 Key Features

| Feature | Description |
|---------|-------------|
| **Color** | True color (24-bit), 256 color, 16 color. Configurable color schemes |
| **Unicode** | Full Unicode support including emoji, CJK, combining characters, right-to-left |
| **Ligatures** | Programming font ligature support (configurable on/off) |
| **Tabs** | Multiple tabs per window, drag to reorder, tab title shows PWD or running command |
| **Split panes** | Horizontal/vertical split within a tab (like tmux but native) |
| **Profiles** | Named profiles with independent font, colors, cursor style, shell, starting directory |
| **Scrollback** | Configurable buffer (default: 10,000 lines, max: unlimited/on-disk) |
| **Search** | Search scrollback with regex support, highlight all matches |
| **URL detection** | Clickable URLs (Ctrl+click to open), file paths, IP addresses |
| **Copy on select** | Optional: selecting text automatically copies to clipboard |
| **Bell** | Visual bell (flash), auditory bell (system sound), or none |
| **Shell integration** | Detect PWD changes (OSC 7), command status (OSC 133), window title (OSC 0/2) |
| **Cursor styles** | Block, underline, bar. Blinking configurable |
| **Selection** | Click-drag select, double-click word, triple-click line. Ctrl+Shift+C/V for copy/paste |
| **Drop-down mode** | Optional drop-down/quake-style mode (slide from top of screen on hotkey) |

### 4.3 Configuration

```toml
# ~/.config/liquide/liquid-terminal.toml

[terminal]
shell = ""                         # empty = $SHELL or /bin/bash default
starting_directory = ""            # empty = home directory
scrollback_lines = 10000           # -1 for unlimited
audible_bell = false
visual_bell = true
allow_hyperlinks = true
copy_on_select = false
cursor_shape = "block"             # block, underline, bar
cursor_blink = true
word_chars = "-A-Za-z0-9,./?%&#:_=+@~"  # characters considered part of a word for double-click

[terminal.font]
family = "monospace"
size = 12
ligatures = true
bold_is_bright = false

[terminal.colors]
scheme = "liquid-dark"             # liquid-dark, liquid-light, solarized, monokai, nord, dracula, custom
foreground = "#d4d4d4"
background = "#1e1e1e"
cursor = "#aeafad"
selection_background = "#264f78"
# palette = ["#000000", ..., "#ffffff"]  # 16-color palette override

[terminal.padding]
horizontal = 8
vertical = 4

[terminal.dropdown]
enabled = false
hotkey = "F12"
height_percent = 40
animation_duration_ms = 150
```

### 4.4 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+T` | New tab |
| `Ctrl+Shift+W` | Close tab |
| `Ctrl+Shift+N` | New window |
| `Ctrl+Shift+C` | Copy (or `Ctrl+C` when no selection) |
| `Ctrl+Shift+V` | Paste |
| `Ctrl+Shift+F` | Search scrollback |
| `Ctrl+Shift+E` | Split horizontal |
| `Ctrl+Shift+O` | Split vertical |
| `Alt+[1-9]` | Switch to tab N |
| `Ctrl+=` / `Ctrl+-` | Zoom in / out |
| `Ctrl+0` | Reset zoom |
| `Shift+PageUp/Down` | Scroll up/down |

### 4.5 Remote Session Considerations

- The terminal emulator runs on the server by default. A shell process spawns on the server.
- Client-side terminal offload (see spec-client.md) renders the terminal locally: the server sends cell-grid diffs and the client renders glyphs using cached fonts. This eliminates round-trip latency for keystroke echo, making the terminal feel native even on high-latency connections.
- Drop-down mode works in both remote and local sessions (the compositor manages the sliding animation).

### 4.6 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Launch terminal | `apps.terminal.enabled` | `true` |
| Shell override | `apps.terminal.allowed_shells` | `["*"]` |
| Drop-down terminal | `apps.terminal.dropdown` | `true` |

### 4.7 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Terminal
GenericName=Terminal Emulator
Comment=Open a terminal window
Exec=liquid-terminal
Icon=utilities-terminal
Categories=System;TerminalEmulator;
Keywords=terminal;shell;command;prompt;console;
StartupNotify=true

[Desktop Action new-window]
Name=New Window
Exec=liquid-terminal --new-window

[Desktop Action new-tab]
Name=New Tab
Exec=liquid-terminal --new-tab
```

---

## 5) Image Viewer — `liquid-image`

### 5.1 Overview

A fast, lightweight image viewer with zoom, pan, rotate, and slideshow support.

### 5.2 Key Features

| Feature | Description |
|---------|-------------|
| **Formats** | PNG, JPEG, WebP, GIF (animated), SVG, BMP, TIFF, HEIC, AVIF, ICO, PNM |
| **Zoom** | Scroll wheel zoom, fit-to-window (`Ctrl+0`), 1:1 actual size (`Ctrl+1`), zoom percentage indicator |
| **Pan** | Click-drag to pan when zoomed, keyboard arrow keys |
| **Rotate** | `Ctrl+R` rotate 90° CW, `Ctrl+Shift+R` CCW, flip horizontal/vertical |
| **Navigation** | Previous/next image in directory (arrow keys or `<`/`>`) |
| **Slideshow** | Fullscreen slideshow with configurable interval (1s–60s), random order option |
| **EXIF** | Side panel showing EXIF/metadata: dimensions, color space, camera model, date, GPS coordinates |
| **Print** | Print current image with paper size and fit options |
| **Open with** | "Open in external editor" button (configurable, default: GIMP if available) |
| **Color profiles** | Respects embedded ICC profiles. Applies display profile for accurate color rendering |
| **Trash** | Delete key moves current image to trash (with undo toast) |
| **Copy** | `Ctrl+C` copies image to clipboard |
| **Set wallpaper** | Right-click → "Set as wallpaper" |

### 5.3 Configuration

```toml
[image_viewer]
zoom_mode = "fit"                   # fit, actual, last-used
interpolation = "bilinear"          # nearest, bilinear, lanczos
background_color = "checkered"      # checkered (transparency), black, white, #hex
slideshow_interval_sec = 5
slideshow_loop = true
slideshow_random = false
show_exif_panel = false             # auto-show EXIF panel
animate_gif = true
antialiasing = true
```

### 5.4 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Left` / `Right` | Previous / next image in directory |
| `Ctrl+0` | Fit to window |
| `Ctrl+1` | Actual size (100%) |
| `+` / `-` | Zoom in / out |
| `Ctrl+R` | Rotate clockwise |
| `F5` | Start slideshow |
| `F11` | Toggle fullscreen |
| `Delete` | Move to trash |
| `I` | Toggle info/EXIF panel |

### 5.5 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Launch image viewer | `apps.image_viewer.enabled` | `true` |
| Set wallpaper | `apps.image_viewer.set_wallpaper` | `true` |

### 5.6 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Image Viewer
GenericName=Image Viewer
Comment=View and browse images
Exec=liquid-image %U
Icon=image-viewer
Categories=Graphics;Viewer;
MimeType=image/png;image/jpeg;image/gif;image/webp;image/svg+xml;image/bmp;image/tiff;image/heic;image/avif;image/x-icon;
Keywords=image;photo;picture;viewer;gallery;
StartupNotify=true
```

---

## 6) Calculator — `liquid-calc`

### 6.1 Overview

A calculator with basic, scientific, and programmer modes. Expression input with history. Compact always-on-top mode available.

### 6.2 Modes

| Mode | Features |
|------|----------|
| **Basic** | Standard arithmetic: +, −, ×, ÷, %, parentheses, decimal, memory (M+/M-/MR/MC) |
| **Scientific** | Trigonometry (sin/cos/tan/asin/acos/atan), logarithms (ln/log/log2), powers, roots, factorials, constants (π, e, φ), degrees/radians toggle |
| **Programmer** | Hex/Dec/Oct/Bin display, bitwise operations (AND, OR, XOR, NOT, shift), byte/word/dword/qword size selector, two's complement |
| **Converter** | Unit conversion: length, weight, temperature, area, volume, speed, time, data size, energy, pressure |

### 6.3 Features

- **Expression input**: type mathematical expressions as text (e.g., `sin(45) * 2 + sqrt(16)`). Results are calculated as-you-type.
- **History**: scrollable list of previous calculations. Click to reuse a result.
- **Compact mode**: small always-on-top window showing just the display and numpad.
- **Keyboard accessible**: full numpad support, `Enter` to calculate, `Escape` to clear.
- **Copy/paste**: `Ctrl+C` copies result, `Ctrl+V` pastes number.
- **Thousand separators**: locale-aware display (e.g., `1,234,567.89`).
- **Arbitrary precision**: integer operations use arbitrary-precision arithmetic. Floating point uses 64-bit double.

### 6.4 Configuration

```toml
[calculator]
default_mode = "basic"             # basic, scientific, programmer, converter
angle_unit = "degrees"             # degrees, radians
thousands_separator = true
decimal_places = 10                # max decimal places shown
always_on_top = false
compact_mode = false
history_size = 100
```

### 6.5 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Calculator
GenericName=Calculator
Comment=Perform arithmetic and scientific calculations
Exec=liquid-calc
Icon=accessories-calculator
Categories=Utility;Calculator;
Keywords=calculator;math;arithmetic;convert;
StartupNotify=true
```

---

## 7) Screenshot Tool — `liquid-screenshot`

### 7.1 Overview

A screenshot and annotation tool invoked by keyboard shortcuts or the application menu. Captures the server-side compositor frame in remote sessions.

### 7.2 Capture Modes

| Mode | Trigger | Description |
|------|---------|-------------|
| **Full screen** | `Print Screen` | Capture entire desktop (all monitors) |
| **Active window** | `Alt+Print Screen` | Capture focused window (with or without decoration, configurable) |
| **Region select** | `Super+Shift+S` | Interactive crosshair selection. Click-drag to select rectangle. Press Escape to cancel |
| **Timed** | Via app menu | Delay (3s, 5s, 10s) before capture. Useful for capturing menus/tooltips |
| **Screen recording** | `Super+Shift+R` | Record screen to WebM/MP4 (PipeWire screencast). Click status bar indicator to stop |

### 7.3 Post-Capture Actions

After capture, a floating toolbar appears with the screenshot preview:

| Action | Description |
|--------|-------------|
| **Save** | Save to `~/Screenshots/Screenshot_YYYY-MM-DD_HH-MM-SS.png` |
| **Copy** | Copy to clipboard (default for `Super+Shift+S` region capture) |
| **Annotate** | Open annotation editor: draw freehand, rectangle, circle, arrow, text, highlight, blur/pixelate region |
| **Open** | Open in image viewer or external editor |
| **Discard** | Discard the screenshot |

### 7.4 Configuration

```toml
[screenshot]
save_directory = "~/Screenshots"
format = "png"                     # png, jpeg, webp
jpeg_quality = 90
include_cursor = false
include_decoration = true          # window screenshots include window shadow/border
play_shutter_sound = true
copy_to_clipboard = true           # always copy in addition to save
show_notification = true
delay_default_sec = 0
recording_format = "webm"         # webm, mp4
recording_framerate = 30
recording_audio = false            # capture desktop audio during recording
```

### 7.5 Remote Session Considerations

- Screenshots capture the **server-side** compositor frame buffer, not the client display.
- Saved files are stored on the server filesystem.
- "Copy to clipboard" places the image on the server clipboard, which is then synced to the client clipboard via the clipboard channel.
- Screen recording uses PipeWire screencast on the server side.

### 7.6 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Take screenshots | `apps.screenshot.enabled` | `true` |
| Screen recording | `apps.screenshot.recording` | `true` |
| Annotation tools | `apps.screenshot.annotate` | `true` |

### 7.7 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Screenshot
GenericName=Screenshot Tool
Comment=Take screenshots and record the screen
Exec=liquid-screenshot
Icon=applets-screenshooter
Categories=Utility;
Keywords=screenshot;capture;snip;record;screencast;
StartupNotify=true

[Desktop Action region]
Name=Capture Region
Exec=liquid-screenshot --region

[Desktop Action window]
Name=Capture Window
Exec=liquid-screenshot --window

[Desktop Action record]
Name=Record Screen
Exec=liquid-screenshot --record
```

---

## 8) Archive Manager — `liquid-archive`

### 8.1 Overview

Create, browse, and extract archives. Integrates with the file manager for seamless archive handling.

### 8.2 Supported Formats

| Format | Read | Write | Notes |
|--------|------|-------|-------|
| `.tar` | Yes | Yes | Uncompressed |
| `.tar.gz` / `.tgz` | Yes | Yes | gzip compressed |
| `.tar.bz2` | Yes | Yes | bzip2 compressed |
| `.tar.xz` | Yes | Yes | xz compressed |
| `.tar.zst` | Yes | Yes | Zstandard compressed |
| `.zip` | Yes | Yes | Deflate, ZIP64 supported |
| `.7z` | Yes | Yes | LZMA2 compressed |
| `.rar` | Yes | No | Read-only (unrar library) |
| `.gz` / `.bz2` / `.xz` / `.zst` | Yes | Yes | Single-file compressed |
| `.iso` | Yes | No | Browse ISO 9660 images |
| `.deb` / `.rpm` | Yes | No | Browse package contents |

### 8.3 Key Features

- **Browse**: open an archive and browse its contents like a directory tree without extracting.
- **Extract**: extract all or selected files to a chosen directory. Progress bar for large archives.
- **Create**: create archives from selected files/directories. Choose format and compression level.
- **Drag-and-drop**: drag files from the file manager to create an archive, or drag from archive browser to extract.
- **Password**: create and extract password-protected archives (zip AES, 7z AES-256).
- **Split archives**: create and extract multi-volume split archives.
- **Integrity check**: verify archive integrity without extracting.
- **File manager integration**: double-click an archive in `liquid-files` to browse it. Right-click → "Extract Here", "Extract To...", "Compress...".

### 8.4 Configuration

```toml
[archive]
default_format = "tar.zst"
compression_level = "normal"       # fast, normal, maximum
extract_to_subfolder = true        # create subfolder when extracting
open_after_extract = true          # open destination folder after extraction
overwrite_existing = "ask"         # ask, skip, overwrite, rename
```

### 8.5 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Archive Manager
GenericName=Archive Manager
Comment=Create and extract archives
Exec=liquid-archive %U
Icon=liquid-archive
Categories=Utility;Archiving;
MimeType=application/zip;application/x-tar;application/gzip;application/x-bzip2;application/x-xz;application/zstd;application/x-7z-compressed;application/x-rar;application/vnd.rar;application/x-iso9660-image;
Keywords=archive;compress;extract;zip;tar;unzip;7z;rar;
StartupNotify=true
```

---

## 9) System Monitor — `liquid-monitor`

### 9.1 Overview

A task manager and resource monitor. Displays processes, CPU/memory/disk/network usage with charts. Analogous to GNOME System Monitor or Windows Task Manager.

### 9.2 Tabs

#### System Overview

| Field | Source |
|-------|--------|
| Hostname | `gethostname()` |
| OS & Version | `/etc/os-release` |
| Kernel | `uname` |
| CPU | `/proc/cpuinfo` (model, cores, frequency) |
| Memory | `/proc/meminfo` (total, used, available, swap) |
| GPU | DRM/sysfs (model, driver, VRAM) |
| Disk | `statvfs` (filesystem usage per mount) |
| Uptime | `/proc/uptime` |
| Session | LiquiDE session ID, user, connected clients |

#### Processes

- **Columns**: PID, Name, User, CPU%, Memory (RSS), Disk R/W, Network, Status, Command.
- **Views**: flat list (default), tree view (parent → child hierarchy).
- **Sort**: click column header. Default: CPU% descending.
- **Search**: filter by process name.
- **Actions** (on selected process): End (SIGTERM), Kill (SIGKILL), Renice (set priority), Properties (full `/proc/<pid>/status` detail).
- **Color coding**: current user's processes vs. system processes.

#### CPU

- Per-core utilization graph (real-time, 60-second history).
- Total CPU percentage, clock frequency.
- Load averages (1m, 5m, 15m).

#### Memory

- Physical memory usage over time (total, used, buffers/cache, available).
- Swap usage.
- Top memory consumers (top 10 processes).

#### Disk

- Per-disk I/O throughput graph (read/write MB/s).
- Per-filesystem usage bar charts.

#### Network

- Per-interface throughput graph (TX/RX).
- Connection count (TCP/UDP).
- Active connections list (optional, like `netstat`).

### 9.3 Remote Session Considerations

- The system monitor displays **server** hardware and processes.
- A banner at the top reads: "Showing resources for [hostname] (remote server)."
- Process kill/renice operates on server processes. Permission requires appropriate Unix capabilities or policy approval.
- Network tab shows server network interfaces, not client interfaces.

### 9.4 Configuration

```toml
[system_monitor]
default_tab = "processes"          # overview, processes, cpu, memory, disk, network
update_interval_ms = 1000
show_all_processes = false          # false = only current user's processes
graph_history_sec = 60
```

### 9.5 Policy

| Action | Policy Key | Default |
|--------|-----------|---------|
| Launch system monitor | `apps.system_monitor.enabled` | `true` |
| View all processes | `apps.system_monitor.show_all` | `true` |
| End/kill processes | `apps.system_monitor.kill` | `true` (own), `false` (others) |
| Renice processes | `apps.system_monitor.renice` | `false` |

### 9.6 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=System Monitor
GenericName=System Monitor
Comment=View system resource usage and manage processes
Exec=liquid-monitor
Icon=utilities-system-monitor
Categories=System;Monitor;
Keywords=task;manager;process;cpu;memory;system;monitor;performance;
StartupNotify=true
```

---

## 10) Document Viewer — `liquid-docs`

### 10.1 Overview

A document viewer for PDF, EPUB, DjVu, and XPS files. Rendering via poppler (PDF) or mupdf.

### 10.2 Key Features

| Feature | Description |
|---------|-------------|
| **Formats** | PDF, PDF/A, EPUB, DjVu, XPS, CBR/CBZ (comic archives), TIFF (multi-page) |
| **Navigation** | Page thumbnails sidebar, table of contents sidebar, go-to-page, search within document |
| **Zoom** | Fit page, fit width, custom percentage, `Ctrl+=`/`Ctrl+-` zoom |
| **View modes** | Single page, continuous scroll, dual-page (book view), presentation mode (fullscreen, page-by-page) |
| **Annotations** | Highlight, underline, strikethrough, sticky note, freehand draw (saved within the PDF) |
| **Forms** | Fill PDF forms (text fields, checkboxes, radio buttons, dropdowns), save filled form |
| **Bookmarks** | Add/remove bookmarks per page, bookmarks sidebar |
| **Search** | Full-text search with match highlighting and result count |
| **Print** | Print document with page range, copies, duplex, paper size options |
| **Copy text** | Select and copy text from the document |
| **Dark mode** | Invert document colors for comfortable reading in dark environments |
| **Properties** | Document metadata: title, author, subject, keywords, creation date, page count, file size, PDF version |

### 10.3 Configuration

```toml
[document_viewer]
default_zoom = "fit-width"         # fit-page, fit-width, percentage
default_view = "continuous"        # single, continuous, dual, presentation
show_sidebar = true                # show thumbnails/TOC sidebar on open
remember_position = true           # remember last-read page per document
dark_mode_documents = false        # invert document colors
continuous_scroll_gap = 4          # pixels between pages in continuous mode
```

### 10.4 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Space` / `Shift+Space` | Page down / page up |
| `Ctrl+G` | Go to page |
| `Ctrl+F` | Search |
| `F5` | Presentation mode |
| `F9` | Toggle sidebar |
| `N` / `P` | Next / previous search result |
| `Ctrl+D` | Toggle bookmark on current page |

### 10.5 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Document Viewer
GenericName=Document Viewer
Comment=View PDF, EPUB, and other documents
Exec=liquid-docs %U
Icon=liquid-docs
Categories=Office;Viewer;
MimeType=application/pdf;application/epub+zip;image/vnd.djvu;application/oxps;application/vnd.ms-xpsdocument;application/x-cbr;application/x-cbz;
Keywords=pdf;document;viewer;reader;epub;book;
StartupNotify=true
```

---

## 11) Disk Usage Analyzer — `liquid-diskusage`

### 11.1 Overview

Visual disk usage analysis with treemap and sunburst visualizations.

### 11.2 Key Features

- **Scan**: scan selected directory or entire filesystem. Shows progress with file count and estimated time.
- **Treemap**: area-proportional rectangles showing relative file/directory sizes. Color-coded by file type. Click to drill into subdirectories.
- **Sunburst**: concentric ring chart showing directory hierarchy. Inner ring = root, outer rings = deeper directories.
- **List view**: sorted table showing directories/files by size (largest first).
- **Filters**: filter by file type, minimum size, date range. Show/hide hidden files.
- **Drill-down**: click any directory to zoom in. Breadcrumb trail shows current path.
- **Export**: export scan results to CSV or JSON.
- **Cancel**: cancel long scans gracefully.
- **Refresh**: re-scan without restarting the tool.

### 11.3 Configuration

```toml
[disk_usage]
default_view = "treemap"           # treemap, sunburst, list
show_hidden = false
follow_symlinks = false
exclude_patterns = ["/proc", "/sys", "/dev", "/run"]
```

### 11.4 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Disk Usage Analyzer
GenericName=Disk Usage Analyzer
Comment=Analyze disk space usage
Exec=liquid-diskusage
Icon=liquid-diskusage
Categories=System;Filesystem;
Keywords=disk;usage;space;analyzer;storage;size;
StartupNotify=true
```

---

## 12) Font Viewer — `liquid-fonts`

### 12.1 Overview

Browse, preview, and manage fonts installed on the system.

### 12.2 Key Features

- **Browse**: list all installed fonts, grouped by family (serif, sans-serif, monospace, display, handwriting).
- **Search**: filter fonts by name.
- **Preview**: configurable preview text (default: "The quick brown fox jumps over the lazy dog"). Adjustable preview size.
- **Character grid**: see all glyphs in a font. Click a glyph to see its codepoint, name, and copy it.
- **Font info**: family, style, weight, version, license, designer, description, character coverage percentage.
- **Compare**: select multiple fonts to see them side-by-side with the same preview text.
- **Install**: install `.ttf`/`.otf`/`.woff2` fonts to `~/.local/share/fonts/`. Requires confirmation.
- **Remove**: remove user-installed fonts (system fonts are read-only unless admin).
- **Waterfall**: preview a font at multiple sizes simultaneously (8pt, 10pt, 12pt, 14pt, 18pt, 24pt, 36pt, 48pt, 72pt).

### 12.3 Configuration

```toml
[font_viewer]
preview_text = "The quick brown fox jumps over the lazy dog"
preview_size = 24
default_group = "all"              # all, serif, sans-serif, monospace, display
show_system_fonts = true
show_user_fonts = true
```

### 12.4 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Fonts
GenericName=Font Viewer
Comment=Browse and manage fonts
Exec=liquid-fonts %U
Icon=preferences-desktop-font
Categories=Utility;
MimeType=font/ttf;font/otf;font/woff2;application/x-font-ttf;application/x-font-otf;
Keywords=font;typeface;typography;preview;install;
StartupNotify=true
```

---

## 13) Character Map — `liquid-charmap`

### 13.1 Overview

A Unicode character browser for finding and copying special characters, symbols, and emoji.

### 13.2 Key Features

- **Grid view**: displays characters in a scrollable grid organized by Unicode block.
- **Categories**: Latin, Greek, Cyrillic, CJK, Mathematical, Arrows, Box Drawing, Emoji, Dingbats, Currency, Technical, Braille, Musical, etc.
- **Search**: search by character name, codepoint (U+XXXX), or keyword.
- **Recent**: recently copied characters shown at the top (last 50).
- **Favorites**: pin frequently used characters for quick access.
- **Detail view**: clicking a character shows: glyph (large), Unicode name, codepoint, UTF-8/UTF-16 encoding, Unicode block, category, bidirectional class.
- **Copy**: click character to copy, or click "Copy" button in detail view. Copies the character (not the codepoint).
- **Multi-copy**: select multiple characters to build a string, then copy the combined string.

### 13.3 Configuration

```toml
[charmap]
font = "sans-serif 24"            # font for character display
show_codepoints = true             # show U+XXXX under each character
columns = 16                       # characters per row
```

### 13.4 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Character Map
GenericName=Character Map
Comment=Browse and insert special characters
Exec=liquid-charmap
Icon=accessories-character-map
Categories=Utility;
Keywords=unicode;character;symbol;emoji;special;glyph;
StartupNotify=true
```

---

## 14) Color Picker — `liquid-colorpicker`

### 14.1 Overview

A standalone color picker with eyedropper, manual input, palette management, and color harmony tools.

### 14.2 Key Features

- **Eyedropper**: pick a color from anywhere on screen. Click to sample. Shows magnified view around cursor during picking.
- **Color space input**: Hex (#RRGGBB), RGB (0–255), HSL, HSV, CMYK. Switching between spaces updates all fields live.
- **Palette**: create and manage custom color palettes. Import/export palettes (.gpl, .ase, .json).
- **History**: last 20 picked colors, persistent across sessions.
- **Copy**: click any color in any format to copy. Buttons for "Copy Hex", "Copy RGB", "Copy HSL".
- **Color harmonies**: show complementary, analogous, triadic, split-complementary, tetradic colors for the selected color.
- **Contrast checker**: input foreground and background colors, shows WCAG contrast ratio and pass/fail for AA/AAA levels.
- **Gradient**: generate gradient between two colors with configurable steps. Copy CSS gradient syntax.
- **Named colors**: search CSS/X11 named colors.

### 14.3 Configuration

```toml
[color_picker]
default_format = "hex"             # hex, rgb, hsl, hsv, cmyk
uppercase_hex = true               # #AABBCC vs #aabbcc
include_hash = true                # #AABBCC vs AABBCC when copying
history_size = 20
eyedropper_magnification = 8       # magnifier zoom during picking
```

### 14.4 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Color Picker
GenericName=Color Picker
Comment=Pick and convert colors
Exec=liquid-colorpicker
Icon=color-picker
Categories=Utility;Graphics;
Keywords=color;picker;eyedropper;hex;rgb;palette;
StartupNotify=true
```

---

## 15) Software Center

A graphical application for browsing, installing, updating, and removing Flatpak applications from Flathub and other configured repositories.

### 15.1 Core Features

| Feature | Description |
|---------|-------------|
| **Browse** | Featured apps, categories, editor's picks from Flathub |
| **Search** | Full-text search across app name, summary, description, keywords |
| **Install / Remove** | One-click install/uninstall with progress indication |
| **Update** | View pending updates, update individual apps or all at once |
| **App details** | Screenshots, description, version history, size, developer info, permissions |
| **Permission review** | Pre-install permission summary with danger highlighting |
| **Permission management** | Post-install per-app permission overrides (filesystem, network, devices) |
| **Ratings & reviews** | Display ODRS (Open Desktop Ratings Service) ratings and reviews |
| **Source badge** | Each app shows its source (Flathub, Flathub Beta, custom remote) |
| **Multi-remote** | Browse and install from any configured Flatpak remote |
| **Categories** | Audio & Video, Developer Tools, Education, Games, Graphics, Network, Office, Science, System, Utilities |

### 15.2 Flathub Integration

The Software Center uses the [Flathub API](https://flathub.org/api/) and local Flatpak metadata:

| Data Source | Purpose |
|-------------|---------|
| Flathub AppStream XML | App metadata, screenshots, categories, keywords |
| `flatpak search` | Offline fallback search via local appstream cache |
| `flatpak info` | Installed app details, runtime, size |
| ODRS API | User ratings and reviews |
| Local Flatpak state | Install status, available updates, overrides |

**Caching:**
- AppStream metadata is cached locally and refreshed on each update check (controlled by `flatpak.auto_update_schedule`).
- Screenshots are cached in `~/.cache/liquide/software-center/screenshots/` with LRU eviction at 200 MB.
- App metadata cache: `~/.cache/liquide/software-center/appstream/`.

### 15.3 UI Layout

```
┌─────────────────────────────────────────────────────────────┐
│ Software Center                                   ─ □ ✕     │
├─────────────────────────────────────────────────────────────┤
│  🔍 [Search apps...]                                        │
│                                                             │
│  [Explore]  [Installed]  [Updates (3)]                      │
│                                                             │
│  ── Featured ──────────────────────────────────             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │  Banner  │  │  Banner  │  │  Banner  │   ← carousel     │
│  └──────────┘  └──────────┘  └──────────┘                  │
│                                                             │
│  ── Categories ────────────────────────────                 │
│  [ Audio & Video ] [ Developer Tools ] [ Games ] ...        │
│                                                             │
│  ── Recently Updated ──────────────────────                 │
│  ┌────┐ Firefox           ┌────┐ GIMP                      │
│  │icon│ Web browser  [Install] │icon│ Image editor  [Install]│
│  └────┘                   └────┘                            │
│  ┌────┐ VLC               ┌────┐ LibreOffice               │
│  │icon│ Media player [Open]│icon│ Office suite  [Install]   │
│  └────┘                   └────┘                            │
└─────────────────────────────────────────────────────────────┘
```

**App detail page:**

```
┌─────────────────────────────────────────────────────────────┐
│ ← Back                                           ─ □ ✕     │
│                                                             │
│  ┌────┐  Firefox                                            │
│  │icon│  Mozilla                              [Install]     │
│  └────┘  ★★★★☆ (2,341 ratings)                             │
│                                                             │
│  ┌──────────────────────────────────────────┐               │
│  │              Screenshot carousel          │               │
│  └──────────────────────────────────────────┘               │
│                                                             │
│  Fast, private & safe web browser...                        │
│                                                             │
│  ── Permissions ────────────────────────────                │
│  ⚠ Network access        ✓ Wayland           ✓ Audio       │
│  ⚠ Filesystem: home      ✓ Notifications     ✓ GPU         │
│                                                             │
│  ── Details ────────────────────────────────                │
│  Version: 124.0.1    Size: 241 MB    Runtime: org.freedeskop│
│  Source: Flathub     License: MPL-2.0                       │
│                                                             │
│  ── Version History ────────────────────────                │
│  124.0.1  (2025-02-05)  Bug fixes                          │
│  124.0    (2025-02-01)  New tab groups feature              │
│  123.0.1  (2025-01-15)  Security update                    │
└─────────────────────────────────────────────────────────────┘
```

### 15.4 Update Management

The Software Center's **Updates** tab shows:

1. **Pending Flatpak updates** — app name, current version → new version, download size, changelog summary.
2. **LiquiDE component updates** — if available (read from `liquidctl update check`).
3. **"Update All"** button — applies all pending Flatpak updates in parallel, with per-app progress bars.
4. **Auto-update status** — shows whether auto-updates are enabled, last update time, next scheduled check.

**Background updates:** When `flatpak.auto_update = true`, updates are downloaded and applied in the background. A notification is shown: "N apps were updated" with an action to view details.

### 15.5 Install / Remove Flow

**Install:**
1. User clicks "Install" on an app.
2. Software Center checks policy (`flatpak.enabled`, `flatpak.blocked_apps`, `flatpak.allowed_apps`).
3. Permission summary is shown. "Potentially dangerous" permissions (host filesystem, network, X11) are highlighted.
4. User confirms. If the required runtime is not installed, it is fetched first.
5. Progress bar shows download + install progress. The user can continue browsing.
6. On completion, a toast shows "Firefox installed" with an "Open" action.
7. The app's `.desktop` file export is detected by the launcher via `inotify`.

**Remove:**
1. User clicks "Uninstall" on an installed app page.
2. Confirmation dialog: "Remove Firefox? App data will be kept. [Remove] [Remove with data] [Cancel]".
3. "Remove" runs `flatpak uninstall <app-id>`.
4. "Remove with data" also clears `~/.var/app/<app-id>/`.
5. Unused runtimes are cleaned if `flatpak.gc_unused_runtimes = true`.

### 15.6 Permission Editor

Accessible from the app detail page (installed apps only) or from Settings → Apps → [App] → Permissions:

```
┌─────────────────────────────────────────────────────────────┐
│ Permissions: Firefox                             ─ □ ✕     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ── Filesystem ─────────────────────────────                │
│  [✓] Home directory (~)                                     │
│  [✓] Downloads (~/Downloads)                                │
│  [ ] All files (host)                                       │
│  [ ] /tmp                                                   │
│  [+ Add path...]                                            │
│                                                             │
│  ── Network ────────────────────────────────                │
│  [✓] Network access                                         │
│                                                             │
│  ── Display ────────────────────────────────                │
│  [✓] Wayland                                                │
│  [ ] X11 (fallback)                                         │
│                                                             │
│  ── Devices ────────────────────────────────                │
│  [✓] GPU acceleration (DRI)                                 │
│  [ ] All devices                                            │
│  [✓] Shared memory (SHM)                                    │
│                                                             │
│  ── Session Bus ────────────────────────────                │
│  [✓] org.freedesktop.Notifications                          │
│  [✓] org.freedesktop.portal.*                               │
│  [+ Add service...]                                         │
│                                                             │
│  [Reset to defaults]              [Apply]                   │
└─────────────────────────────────────────────────────────────┘
```

Changes are written to `~/.local/share/flatpak/overrides/<app-id>` and take effect on next app launch.

### 15.7 Policy

| Policy Key | Default | Description |
|-----------|---------|-------------|
| `apps.software_center.enabled` | `true` | Show Software Center in launcher |
| `apps.software_center.allow_install` | `true` | Allow installing apps |
| `apps.software_center.allow_remove` | `true` | Allow removing apps |
| `apps.software_center.allow_permission_edit` | `true` | Allow modifying Flatpak permissions |
| `apps.software_center.show_ratings` | `true` | Show ODRS ratings and reviews |

### 15.8 Configuration

```toml
[software_center]
default_remote = "flathub"
show_beta_apps = false            # Show apps from Flathub beta
show_eol_runtimes = false         # Show end-of-life runtime warnings
screenshot_cache_mb = 200
auto_refresh_interval = "daily"   # AppStream metadata refresh
```

### 15.9 `.desktop` Entry

```ini
[Desktop Entry]
Type=Application
Name=Software Center
GenericName=Application Store
Comment=Browse and install applications from Flathub
Exec=liquid-software-center
Icon=liquid-software-center
Categories=System;PackageManager;
Keywords=flatpak;flathub;install;apps;store;software;package;
StartupNotify=true
MimeType=application/vnd.flatpak.ref;application/vnd.flatpak.repo;
```

**MIME handling:** The Software Center registers as the handler for `.flatpakref` and `.flatpakrepo` files. Opening a `.flatpakref` file shows the app's detail page with an "Install" button. Opening a `.flatpakrepo` file prompts to add the remote.

---

## 16) Shared Infrastructure

### 15.1 Common Application Conventions

All LiquiDE built-in applications follow these conventions:

| Convention | Requirement |
|------------|-------------|
| **Window icon** | Use icon-theme name matching the `.desktop` file `Icon=` key |
| **App ID** | Wayland `app_id` matches `.desktop` file basename (e.g., `liquid-files`) |
| **Settings storage** | User config in `~/.config/liquide/<app-name>.toml` |
| **State storage** | Runtime state (window size, sidebar widths) in `~/.local/state/liquide/<app-name>/` |
| **Data storage** | User data (palettes, bookmarks, history) in `~/.local/share/liquide/<app-name>/` |
| **D-Bus activation** | Each app can be activated via D-Bus for single-instance enforcement |
| **Dark mode** | Respond to `org.freedesktop.portal.Settings` `color-scheme` signal automatically |
| **Locale** | All user-visible strings are localizable via Fluent (`.ftl`) or gettext (`.po`) files |
| **Undo** | Destructive actions show an undo toast for 5 seconds before committing |
| **Print** | Use the LiquiDE print dialog (which routes through the portal) |

### 15.2 Icon Theme Entries

All built-in apps register icons in the LiquiDE icon theme at standard sizes (16, 24, 32, 48, 64, 128, 256, scalable SVG):

| App | Icon Name |
|-----|-----------|
| File Manager | `system-file-manager` |
| Text Editor | `text-editor` |
| Terminal | `utilities-terminal` |
| Image Viewer | `image-viewer` |
| Calculator | `accessories-calculator` |
| Screenshot | `applets-screenshooter` |
| Archive Manager | `liquid-archive` |
| System Monitor | `utilities-system-monitor` |
| Document Viewer | `liquid-docs` |
| Disk Usage | `liquid-diskusage` |
| Font Viewer | `preferences-desktop-font` |
| Character Map | `accessories-character-map` |
| Color Picker | `color-picker` |

Icon names follow the [freedesktop Icon Naming Specification](https://specifications.freedesktop.org/icon-naming-spec/latest/) where applicable. LiquiDE-specific icons (not in the spec) are provided in the LiquiDE icon theme and fallback to hicolor.

### 15.3 MIME Type Registrations

Built-in apps register as handlers for their supported MIME types in their `.desktop` files (see each app's entry above). Default associations are set in `/usr/share/applications/liquide-mimeapps.list`:

```ini
[Default Applications]
inode/directory=liquid-files.desktop
text/plain=liquid-edit.desktop
image/png=liquid-image.desktop
image/jpeg=liquid-image.desktop
image/gif=liquid-image.desktop
image/webp=liquid-image.desktop
image/svg+xml=liquid-image.desktop
application/pdf=liquid-docs.desktop
application/epub+zip=liquid-docs.desktop
application/zip=liquid-archive.desktop
application/x-tar=liquid-archive.desktop
application/gzip=liquid-archive.desktop
application/x-7z-compressed=liquid-archive.desktop
```

Users can override these defaults via Settings → Default Applications or via `xdg-mime`.

### 15.4 D-Bus Conventions

Each app optionally provides a D-Bus interface for programmatic control:

| App | Service Name | Key Methods |
|-----|-------------|-------------|
| File Manager | `org.liquide.Files` | `OpenFolder(s)`, `ShowFile(s)`, `EmptyTrash()` |
| Text Editor | `org.liquide.Edit` | `Open(s)`, `OpenAtLine(si)` |
| Terminal | `org.liquide.Terminal` | `Open()`, `RunCommand(s)` |
| Screenshot | `org.liquide.Screenshot` | `CaptureRegion()`, `CaptureWindow()`, `CaptureScreen()` |

### 15.5 Liquid UI Toolkit

All built-in apps are written in Rust using `liquid-ui`, LiquiDE's own UI toolkit:

1. `liquid-ui` renders directly to the Wayland surface via the LiquiDE compositor's rendering pipeline. No intermediate GUI toolkit (GTK, Qt) is involved.
2. The Liquid Glass CSS theme is applied natively — `liquid-ui` widgets read the same CSS custom properties as the compositor shell.
3. Apps use `liquid-ui` standard patterns: `Window`, `HeaderBar`, `NavigationView`, `ToastOverlay`, `ListView`, `Grid`, etc.
4. Glass blur effects on window backgrounds are composited by the LiquiDE compositor — the app requests a translucent surface and the compositor applies blur behind it.
5. Third-party GTK/Qt applications still work normally inside LiquiDE sessions via standard Wayland and XWayland support. Only the **built-in** apps use `liquid-ui` directly.

### 15.6 Global Policy

| Policy Key | Default | Description |
|-----------|---------|-------------|
| `apps.builtin.allow_launch` | `true` | Master switch for all built-in apps |
| `apps.builtin.allow_install` | `true` | Allow users to install additional apps (Flatpak, package manager) |
| `apps.builtin.allow_uninstall` | `false` | Allow users to remove built-in apps |

---

## 16) Test Plan

### Functional (Per App)

| App | Key Test Scenarios |
|-----|-------------------|
| File Manager | Create/rename/delete/move files and directories, navigate breadcrumb, dual pane DnD, archive browsing, trash/restore, bulk rename, thumbnail generation for all supported types, network share browsing, search with filters |
| Text Editor | Open/edit/save files, syntax highlighting correctness for 10+ languages, search & replace with regex, multi-tab with unsaved changes prompt, encoding detection/conversion, line ending conversion, large file (50 MB) handling |
| Terminal | True color rendering, Unicode/emoji display, ligature rendering, split panes, scrollback search, URL detection/click, shell integration (PWD tracking), profile switching, drop-down mode, copy-on-select |
| Image Viewer | Open all supported formats, zoom/pan/rotate, animated GIF playback, EXIF display, slideshow, navigate directory images, set wallpaper, copy to clipboard, HEIC/AVIF decoding |
| Calculator | All four modes correct results, expression parsing, history, unit conversions, programmer mode bit operations, copy/paste results, compact mode |
| Screenshot | All capture modes (full/window/region/timed), annotation tools (draw/rect/arrow/text/blur), save/copy/discard post-capture, screen recording start/stop |
| Archive Manager | Create/extract for all supported formats, password-protected archives, browse without extract, drag-and-drop integration, progress for large archives, split archives |
| System Monitor | Process list accuracy vs `/proc`, CPU/memory/disk/network graphs update, kill/renice operations, search filtering, tree view hierarchy |
| Document Viewer | PDF rendering fidelity, EPUB reflow, search within document, annotations save/load, form filling, bookmarks, print, presentation mode |
| Disk Usage | Scan accuracy vs `du`, treemap renders correctly, drill-down navigation, exclude patterns respected, cancel mid-scan |
| Font Viewer | All installed fonts listed, preview renders correctly, install/remove user fonts, character grid displays all glyphs, waterfall view |
| Character Map | Unicode categories correct, search by name/codepoint works, copy produces correct character, recent/favorites persist |
| Color Picker | Eyedropper picks correct color, all color space conversions are accurate, contrast checker matches WCAG spec, palette import/export |

### Integration

- Each app opens from the launcher search and from keyboard shortcuts.
- MIME type associations correctly open files with the right app.
- File manager integrates with archive manager (double-click archive, right-click compress).
- File manager "Open in Terminal" opens terminal at current directory.
- Screenshot copies work correctly with clipboard channel in remote sessions.
- All apps follow dark/light mode switching.
- All apps respect `reduce_motion`, `high_contrast`, `text_scale` accessibility settings.
- All apps are keyboard-navigable and screen-reader usable (AT-SPI tree complete).

### Remote Session

- All apps render correctly when streamed via LiquiDE protocol.
- Terminal offload mode reduces input latency measurably vs. streamed mode.
- File manager shows correct server filesystem, not client filesystem.
- System monitor shows server hardware info.
- Screenshot captures server compositor frame.
- Large file operations (file manager copy, archiving) do not stall the protocol or other channel traffic.

### Policy

- Each app respects its policy gate (disabled → does not launch, shows "This app has been disabled by your administrator").
- Per-action policies take effect (e.g., `apps.system_monitor.kill = false` disables process kill button).
- `apps.builtin.allow_launch = false` prevents all built-in apps from starting.

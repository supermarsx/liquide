# Liquide Complete Shell Services Roadmap

## Goal

Liquide already has a substantial compositor, shell, app, input, rendering, and platform foundation. The remaining gap against an NT5-style complete shell is the shared integration layer: the equivalent of what `shell32`, `explorer`, `shlwapi`, common dialogs, shell extensions, tray services, and Control Panel contracts provided together.

The roadmap is to build that layer as shared Shell Services, then migrate shell surfaces to consume it.

## What NT5 Teaches Us

The NT5 shell tree is broad because the shell is not one executable. Important responsibilities are split across:

- `explorer`: desktop host, taskbar, Start menu, tray, shell lifecycle.
- `shell32`: namespace, file operations, shell links, file associations, context menus, property sheets, thumbnails, special folders, Recycle Bin, printers, network places.
- `shlwapi`: path, URL, registry, association, and stream helpers.
- `browseui` and `shdocvw`: navigation frame, browser host, address/search UI, web folder integration.
- `comdlg32`: common file dialogs.
- `comctl32`: common controls.
- `ext`: shell extension families such as image preview, web folders, media, systray, search, and device-related UI.
- `services`: shell-adjacent daemons such as theme services.

For Liquide, the lesson is not to copy NT5 APIs. The lesson is that every shell-visible object needs a common identity, action, preview, property, launch, and integration contract.

## Existing Liquide Starting Points

Relevant existing pieces include:

- `crates/liquide-shell`
- `crates/liquide-desktop-model`
- `crates/liquide-dock`
- `crates/liquide-tray`
- `crates/liquide-statusbar`
- `crates/liquide-apps-files`
- `crates/liquide-dialogs`
- `crates/liquide-xdg`
- `crates/liquide-context-menu`
- `crates/liquide-plugins`
- `crates/liquide-notification-daemon`
- `crates/liquide-storage`
- `crates/liquide-network`
- `crates/liquide-bluetooth`
- `crates/liquide-app-harness`
- `crates/liquide-ime`
- `crates/liquide-input-method`

The problem is that many of these are local abstractions. A complete shell needs shared system contracts.

## Phase 1: Shell Execute Planning

Add `crates/liquide-shell-services` with pure, testable ShellExecute-style planning.

Scope:

- Represent shell targets as file paths or URIs.
- Represent verbs such as open, edit, print, properties, and custom verbs.
- Register app handlers for MIME types and schemes.
- Resolve default handlers and explicit Open With overrides.
- Expand `.desktop` Exec templates without launching processes.
- Preserve terminal and app identity metadata in the plan.

This unlocks Files, launcher, dialogs, search, desktop icons, and later portals.

## Phase 2: Namespace And Shell Items

Promote the Files namespace into shared shell identity.

New shared concepts:

- `ShellItemId`
- `ShellItemKind`
- `ShellItem`
- `ShellNamespaceProvider`
- `ShellNamespaceService`

Provider families:

- filesystem
- trash
- recent
- search
- apps
- settings
- network
- storage devices
- printers
- Bluetooth devices
- portals or sandboxed locations

## Phase 3: Association Catalog

Build an app catalog and association database.

Capabilities:

- XDG `.desktop` discovery.
- MIME handler indexing.
- user default app persistence.
- URL scheme handlers.
- app visibility filtering.
- startup notification metadata.
- launch context planning.

## Phase 4: Context Actions

Replace ad hoc menu actions with a provider model.

Capabilities:

- Query available actions for selected shell items.
- Provide built-in actions for files, folders, devices, apps, windows, desktop, dock, and tray.
- Allow plugins to contribute actions.
- Keep action planning separate from execution.

## Phase 5: Preview, Thumbnail, Metadata

Centralize provider registration for:

- previews
- thumbnails
- metadata
- columns
- infotips
- property panes

This service should be used by Files, file dialogs, search, desktop icons, and properties UI.

## Phase 6: Portals And D-Bus

Turn `liquide-xdg` portal abstractions into app compatibility surfaces.

Important portals:

- file chooser
- open URI
- screenshot
- screencast
- notification
- account
- inhibit
- background
- global shortcuts
- permission store

D-Bus implementation should be feature-gated until CI and packaging are ready.

## Phase 7: Tray Protocols

Move beyond local tray handles.

Capabilities:

- StatusNotifierWatcher
- StatusNotifierItem
- D-Bus menu
- attention state
- overflow and pinning
- per-app persistence
- notification identity linking

## Phase 8: Devices And Removable Media

Make device workflows first-class shell workflows.

Capabilities:

- mount
- unmount
- eject
- open in Files
- properties
- AutoPlay policy
- low-space warnings
- printer queues
- Bluetooth pairing
- network connection selection

## Phase 9: IME And International Input

Wire real composition through the app harness.

Capabilities:

- preedit and commit
- candidate windows
- CJK input
- dead keys and compose
- focus and surrounding text
- password field behavior
- Wayland text-input/input-method integration

## Phase 10: Settings, Policy, And E2E

Expose shell-service configuration in Settings and add end-to-end coverage.

Settings areas:

- default apps
- file previews
- portal permissions
- tray visibility
- removable media
- input methods
- context action providers

E2E gates:

- open a document through default app planning
- override default app with Open With
- show a removable device in Files
- show a tray item
- return a file URI through portal file chooser
- commit IME text into a text field

## First Slice Status

Implementation begins with the pure shell execute planner in `crates/liquide-shell-services`.

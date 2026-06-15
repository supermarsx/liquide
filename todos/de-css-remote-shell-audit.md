# DE CSS and Remote Shell Audit TODOs

Date: 2026-06-15

Scope:
- Whole DE shell path, with emphasis on CSS-driven design.
- CSS engine completeness versus actual shell usage.
- Remote-first client/gateway/session/supervisor path.
- Targeted library and visual wiring tests.

Severity legend:
- Critical: current binary or core path is non-functional, drops sessions, or can silently ignore major runtime state.
- High: advertised or expected feature is materially incomplete, unreliable, or split across incompatible paths.
- Medium: important completeness or coverage gap that can produce visible drift, but is not the first blocker.

## Remediation Status — 2026-06-15 (post t65; authoritative — supersedes per-item "Status:" lines below)

Verify reports: `.orchestration/reports/tv-{remote,css,complete}-verify.md`. Commits: f2afa68 (SHM), 9f31f61 (S1 props), 8cda7bc (remote 1-7), 7e5b468 (CSS-engine).

- **Remote-first (TODO 1–7): ALL DONE (8cda7bc).** Gateway TLS (--tls-cert/key + handshake test), stream relay to backend, --backend registration, client --password/--token, resume tokens (build/parse/validate), protocol↔transport frame unification (canonical 22-byte), supervisor real process spawn + liveness. Remainder: signal_session terminating-signals-only (no libc dep).
- **CSS engine (TODO 8–16):**
  - DONE: 10 (compute_style↔restyle parity + 14 tests migrated), 12 (subgrid), 13 (!important cascade), 14 (responsive units — library ready; **shell must call StyleResolver::set_context**), 16 partially via prior load-chain (variables+components loaded; split components/*.css still orphaned — STILL-OPEN).
  - STILL-OPEN (shell/session-side, queued for shell chain): 8 (pipeline cache invalidation), 9 (load_css_theme→pipeline), 11 (container-query 2nd pass), 15 (decoration selector mismatch), 16 (load split components/*.css).
- **CSS completeness (TODO 17–21):** DONE: 17 (border width>0⇒solid), 19 (pseudo custom-props un-ignored + fixed), 21 (`all` whole-value + list-style-image). PARTIAL: 20 (transition write-back at t=0 still #[ignore]d — escalated). STILL-OPEN: 18 (hardcoded add_default_backdrop — queued for shell S4).
- **Related security:** SHM size-overflow/UB **FIXED (f2afa68)**; ws://, notification rate-limit, AuthChain, LDAP fixed; **authorization + audit/event-log planes STILL UNWIRED** (zero production consumers). **font-style italic FIXED**; word-break/text-emphasis/caret-color/background-size **wired (9f31f61)**.
- **Remaining shell-chain waves (queued):** S2 input (capture-phase wiring, keyboard→DOM, 14 dead action arms, StyleResolver::set_context, preventDefault) · S3 dialogs/lockscreen→DOM · S4 overview/cursor/loading + backdrop (TODO 18) · S5 windows→DOM + decoration selectors (TODO 15) · S6 link apps. Plus shell-side CSS TODOs 8/9/11/16.

## Critical

### TODO 1: Configure TLS in the gateway binary

Status: Open

Finding:
The gateway binary accepts TCP connections but never configures TLS. `GatewayRuntime::handle_tcp_connection` requires `tls_acceptor` and drops the client when it is `None`.

Evidence:
- `crates/liquide-gateway/src/main.rs:64` creates `GatewayRuntime`.
- `crates/liquide-gateway/src/main.rs:80` starts the listener.
- `crates/liquide-gateway/src/main.rs:110` accepts and calls `handle_tcp_connection`.
- `crates/liquide-gateway/src/runtime.rs:506` requires `tls_acceptor`.
- `crates/liquide-gateway/src/runtime.rs:509` drops the connection when TLS is missing.

Impact:
The stock gateway binary cannot accept a real remote client.

Remediation:
- [ ] Add certificate/key config to gateway CLI/config.
- [ ] Build a `rustls::ServerConfig` in `main.rs`.
- [ ] Call `runtime.set_tls_config(...)` before entering the accept loop.
- [ ] Add an integration test that starts the real gateway binary path and completes TLS handshake.

### TODO 2: Keep the authenticated remote stream alive and relay it

Status: Open

Finding:
After authentication and routing, the gateway logs that the connection is established, then `handle_tcp_connection` returns and drops the TLS stream. The relay module does not forward bytes.

Evidence:
- `crates/liquide-gateway/src/runtime.rs:721` routes to a session server.
- `crates/liquide-gateway/src/runtime.rs:744` logs fully established.
- `crates/liquide-gateway/src/runtime.rs:755` exits the handler.
- `crates/liquide-gateway/src/relay.rs:117` creates metadata only.

Impact:
Even if TLS and auth succeed, no remote desktop stream remains connected.

Remediation:
- [ ] Define the post-login ownership model for `tls_stream`.
- [ ] Hand the stream to a relay/session task instead of returning.
- [ ] Wire backend session connection establishment.
- [ ] Add an end-to-end test proving frames/input survive after login.

### TODO 3: Register or spawn backend sessions before gateway routing

Status: Open

Finding:
`GatewayRuntime::new` starts with an empty `ServerRegistry`, and the gateway main path never registers a backend. Routing rejects empty healthy server sets.

Evidence:
- `crates/liquide-gateway/src/runtime.rs:169` initializes an empty `ServerRegistry`.
- `crates/liquide-gateway/src/routing.rs:117` rejects routing with no healthy servers.

Impact:
The remote gateway has no real session target.

Remediation:
- [ ] Decide whether gateway launches sessions directly, talks to supervisor, or requires registered backends.
- [ ] Wire backend registration in the real startup path.
- [ ] Add a gateway startup test with at least one routable session backend.

### TODO 4: Add real client credentials to the CLI/runtime path

Status: Open

Finding:
The client CLI accepts `--username` but no password. The runtime path calls `connect(server)`, which sends empty username/password credentials.

Evidence:
- `crates/liquide-client/src/main.rs:21` defines username only.
- `crates/liquide-client/src/main.rs:84` calls `runtime.connect(&cli.server)`.
- `crates/liquide-client/src/runtime.rs:87` delegates to `connection_manager.connect(server)`.
- `crates/liquide-client/src/connection.rs:317` sends `"{username}:{password}"`.

Impact:
The real CLI path cannot authenticate normal users.

Remediation:
- [ ] Add password/token input support, preferably through prompt, env, profile, or keychain.
- [ ] Route CLI credentials into `connect_with_credential`.
- [ ] Avoid logging secrets.
- [ ] Add CLI integration coverage for auth success and failure.

### TODO 5: Wire session resume tokens end to end

Status: Open

Finding:
The client always sends `resume_token: None`, ignores `LoginSuccess.session_token`, and reconnects through fresh empty-credential login. The gateway creates a fresh session id and returns `resume_accepted: None`; token verification exists but is not used in the handshake.

Evidence:
- `crates/liquide-client/src/connection.rs:292` sends no resume token.
- `crates/liquide-client/src/connection.rs:326` decodes `LoginSuccess`.
- `crates/liquide-client/src/connection.rs:393` reconnects via fresh `connect`.
- `crates/liquide-gateway/src/runtime.rs:554` creates a new session id.
- `crates/liquide-gateway/src/runtime.rs:564` sets `resume_accepted: None`.
- `crates/liquide-gateway/src/runtime.rs:397` has `verify_session_token`.

Impact:
Remote-first reconnection and session continuity are not functional.

Remediation:
- [ ] Store `session_id` and `session_token` client-side.
- [ ] Send resume token in `ClientHello`.
- [ ] Validate token in gateway before issuing a new session.
- [ ] Add reconnect/resume integration tests.

### TODO 6: Unify protocol and transport frame formats

Status: Open

Finding:
`liquide-protocol` and `liquide-transport` define incompatible frame headers. Protocol uses a 22-byte big-endian frame with magic/version/timestamp/message type. Transport uses a 10-byte little-endian simplified header and reconstructs missing fields as zero.

Evidence:
- `crates/liquide-protocol/src/frame.rs:163` defines 22-byte protocol frame size.
- `crates/liquide-protocol/src/frame.rs:193` encodes big-endian fields.
- `crates/liquide-transport/src/codec.rs:11` defines 10-byte transport header.
- `crates/liquide-transport/src/codec.rs:66` encodes the simplified header.
- `crates/liquide-transport/src/codec.rs:97` reconstructs missing fields.
- `crates/liquide-transport/src/connection.rs:42` uses the transport codec.

Impact:
Peers using the protocol codec and peers using the transport codec cannot interoperate reliably.

Remediation:
- [ ] Choose one canonical on-wire frame format.
- [ ] Make transport reuse protocol frame encode/decode or explicitly version the transport wrapper.
- [ ] Add cross-crate round-trip tests proving interoperability.

### TODO 7: Make supervisor spawn real sessions

Status: Open

Finding:
The supervisor marks sessions as running even though `SessionSpawner` only returns a synthetic PID.

Evidence:
- `crates/liquide-supervisor/src/runtime.rs:126` calls `spawn_session`.
- `crates/liquide-supervisor/src/runtime.rs:130` records `SessionState::Running`.
- `crates/liquide-supervisor/src/spawn.rs:42` says real fork/exec is not implemented.
- `crates/liquide-supervisor/src/spawn.rs:45` returns a synthetic PID.

Impact:
Remote sessions can appear running without an actual session process.

Remediation:
- [ ] Implement platform-specific process spawning.
- [ ] Track process liveness.
- [ ] Fail session creation if the child process cannot start.
- [ ] Add supervisor integration tests around process lifecycle.

### TODO 8: Invalidate CSS pipeline caches on stylesheet, theme, viewport, and scheme changes

Status: Open

Finding:
`DesktopPipeline` mutates style state but does not clear `last_styles`, `last_layout`, or `last_display_list`. The fast path can return stale cached output when DOM dirty sets are empty.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:67` appends stylesheet only.
- `crates/liquide-shell/src/pipeline/stages.rs:79` replaces theme engine only.
- `crates/liquide-shell/src/pipeline/stages.rs:86` updates viewport only.
- `crates/liquide-shell/src/pipeline/stages.rs:93` updates preferred color scheme only.
- `crates/liquide-shell/src/pipeline/stages.rs:128` reuses cached output on a clean DOM.

Impact:
Runtime CSS changes, theme reloads, monitor resize, and color-scheme changes can be ignored until unrelated DOM dirtiness occurs.

Remediation:
- [ ] Add explicit style/layout/paint cache invalidation to pipeline mutation methods.
- [ ] Mark viewport changes as at least layout and paint dirty.
- [ ] Mark stylesheet/theme/color-scheme changes as style dirty for the full DOM.
- [ ] Add regression tests that render, mutate CSS/theme/viewport, then render again with no DOM mutation.

## High

### TODO 9: Unify theme loading with the CSS-rendered chrome pipeline

Status: Open

Finding:
`Shell::load_css_theme` updates `ShellTheme`, `StyleResolver`, and color scheme, but not the `css_pipeline` that renders dock, status bar, launcher, notifications, and menus. Session startup compensates by separately calling `shell.add_stylesheet(&css)`, but direct shell/devtools paths can desync.

Evidence:
- `crates/liquide-shell/src/shell/theme.rs:50` loads CSS theme into legacy state/resolver.
- `crates/liquide-shell/src/shell/theme.rs:75` only syncs preferred color scheme.
- `crates/liquide-shell/src/shell/scene.rs:365` renders shell chrome through `css_pipeline`.
- `crates/liquide-session/src/desktop/mod.rs:521` manually calls `load_css_theme`.
- `crates/liquide-session/src/desktop/mod.rs:522` manually appends stylesheet.

Impact:
Theme results depend on which public API path is used.

Remediation:
- [ ] Make `load_css_theme` update the pipeline as the authoritative theme source.
- [ ] Decide whether loaded themes replace or append to existing styles.
- [ ] Add tests for direct runtime theme switching after an initial render.

### TODO 10: Fix CSS engine API parity between `compute_style` and `restyle_all`

Status: Partial (t62-logical fixed the padding/margin clobber on the restyle path; compute_style still skips @container/var-scope/assembly and parity tests still use it)

Finding:
`StyleEngine::compute_style` is not equivalent to the full tree restyle path. It skips `@container` rules, applies `var()` with an empty scope, and returns before some assembly that `restyle_all` performs.

Evidence:
- `crates/liquide-style-engine/src/engine/cascade.rs:113` skips container rules.
- `crates/liquide-style-engine/src/engine/cascade.rs:145` uses empty scoped vars.
- `crates/liquide-style-engine/src/engine/cascade.rs:538` performs tree-path variable and style assembly.
- `crates/liquide-style-engine/tests/css_feature_parity.rs:15` uses `compute_style` for many parity tests.

Impact:
Tests can pass while the real shell path behaves differently.

Remediation:
- [ ] Define whether `compute_style` is a limited helper or a full public contract.
- [ ] If public, route it through the same cascade/assembly semantics as tree restyle.
- [ ] Move feature parity tests onto `restyle_all` where tree semantics matter.

### TODO 11: Correct container query sizing and restyle scheduling

Status: Open

Finding:
Container query evaluation falls back to viewport dimensions when real container sizes are missing. The shell records real container sizes after layout for a later evaluation, but does not force that later style pass.

Evidence:
- `crates/liquide-style-engine/src/engine/media.rs:858` falls back to viewport size.
- `crates/liquide-shell/src/pipeline/stages.rs:223` stores container sizes after layout.
- `crates/liquide-shell/src/pipeline/stages.rs:128` can reuse cached output on the next frame.

Impact:
`@container` rules can evaluate against viewport size or stale container dimensions.

Remediation:
- [ ] Track whether container sizes changed during layout.
- [ ] Force a bounded second style/layout pass when container query inputs change.
- [ ] Add tests using real container hosts and changing layout sizes.

### TODO 12: Make `@supports` reflect computed-style capability

Status: Open

Finding:
Support checks can accept values the engine cannot represent or apply. Example: `grid-template-columns: subgrid` can pass support checks but is not parsed into computed tracks.

Evidence:
- `crates/liquide-style-engine/src/engine/media.rs:93` includes grid support entries.
- `crates/liquide-style-engine/src/engine/media.rs:178` includes `subgrid`.
- `crates/liquide-theme-css/src/stylesheet.rs:1462` falls back to property-supported validation.
- `crates/liquide-style-engine/src/engine/apply.rs:565` applies grid tracks.
- `crates/liquide-style-engine/src/value_resolve.rs:820` lacks a `subgrid` branch in track parsing.

Impact:
CSS guarded by `@supports` can execute even when the actual computed output drops the value.

Remediation:
- [ ] Validate supported declarations through the same parser/applier capability as computed styles.
- [ ] Add negative tests for unsupported values inside `@supports`.

### TODO 13: Honor `!important` in `liquide-theme-css` public cascade

Status: Open

Finding:
`liquide-theme-css` records important declarations, but its public `compute_styles*` cascade sorts by layer, specificity, and source order only. Merge order can overwrite and clear important flags.

Evidence:
- `crates/liquide-theme-css/src/parser/mod.rs:106` records `!important`.
- `crates/liquide-theme-css/src/stylesheet.rs:431` sorts without importance.
- `crates/liquide-theme-css/src/stylesheet.rs:440` merges matched rules.
- `crates/liquide-theme-css/src/property.rs:58` says later merge wins.
- `crates/liquide-theme-css/src/property.rs:65` can remove important status.

Impact:
Manual theme/style resolver paths can compute CSS precedence incorrectly.

Remediation:
- [ ] Include importance in cascade priority.
- [ ] Preserve important declarations against later non-important declarations.
- [ ] Add cascade tests for same-specificity and lower-specificity important rules.

### TODO 14: Resolve responsive units in `StyleResolver`

Status: Open

Finding:
`StyleResolver` uses environment-less queries and returns raw values for `%`, `vw`, `vh`, dynamic viewport units, and container units rather than resolving them.

Evidence:
- `crates/liquide-renderer-css/src/resolver.rs:52` uses default query path.
- `crates/liquide-renderer-css/src/resolver.rs:327` returns percent as-is.
- `crates/liquide-renderer-css/src/resolver.rs:328` returns viewport units as-is.
- `crates/liquide-renderer-css/src/resolver.rs:335` returns dynamic viewport units as-is.
- `crates/liquide-renderer-css/src/resolver.rs:337` approximates container units as raw values.
- `assets/themes/components/launcher.css:24` uses `70vh`.
- `assets/themes/components/notifications.css:15` uses `100vh`.

Impact:
Any live shell path using `StyleResolver` for geometry or decoration can compute wrong sizes, especially on remote/mobile displays.

Remediation:
- [ ] Pass viewport/container/base font context into resolver queries.
- [ ] Resolve viewport and percent units against the correct base.
- [ ] Avoid using this resolver for layout-critical values unless it has context.

### TODO 15: Fix window decoration selector mismatch

Status: Open

Finding:
Manual window rendering queries selectors that do not match template/theme selectors. Theme-defined titlebar and button sizing is bypassed.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:394` resolves decoration layout.
- `crates/liquide-shell/src/css_integration.rs:181` queries `titlebar` and `titlebar-button`.
- `assets/templates/window.html:14` uses `window-titlebar` and related classes.
- `assets/themes/night.css:214` styles the actual theme selectors.

Impact:
Window decoration layout and hit testing can drift from CSS.

Remediation:
- [ ] Align resolver selectors with template/theme selectors.
- [ ] Prefer deriving decoration layout from the same DOM/CSS path as visual chrome.
- [ ] Add tests for titlebar/button CSS changing actual manual layout and hit boxes.

### TODO 16: Load or remove split component CSS files

Status: Open

Finding:
The runtime loads monolithic `components.css`, but not `assets/themes/components/*.css`, even though devtools watches that folder.

Evidence:
- `crates/liquide-session/src/desktop/mod.rs:511` loads `variables.css`.
- `crates/liquide-session/src/desktop/mod.rs:512` loads monolithic `components.css`.
- `crates/liquide-devtools/src/live_reload.rs:123` watches split component CSS.
- `assets/themes/components/statusbar.css:99` contains `status-indicator.connected::before`.
- `crates/liquide-shell/src/shell/dom_sync.rs:299` emits empty `status-indicator`.

Impact:
Maintained CSS files can have no runtime effect, and some pseudo-element glyph styling is absent from the loaded style set.

Remediation:
- [ ] Decide whether split files are source-of-truth or dev-only.
- [ ] Load split files in a deterministic order, or generate monolithic `components.css` from them.
- [ ] Add tests proving watched files affect rendered output.

## Medium

### TODO 17: Make border rendering expectations explicit

Status: Open

Finding:
Several theme rules set border width/color without `border-style`. Computed borders default to `None`, and renderers skip `None` borders.

Evidence:
- `crates/liquide-style-engine/src/computed/mod.rs:510` defaults border style to `None`.
- `crates/liquide-style-engine/src/engine/apply.rs:242` applies width/color separately.
- `crates/liquide-renderer-cpu/src/renderer/borders.rs:303` skips `None` borders.
- `crates/liquide-renderer-wgpu/src/renderer.rs:890` skips `None` borders.
- `assets/themes/night.css:30` sets visible border values without style.
- `assets/themes/liquid_glass.css:33` sets visible border values without style.

Impact:
Intended dividers/outlines can silently disappear.

Remediation:
- [ ] Update theme CSS to include `border-style`.
- [ ] Or decide whether nonzero width/color should imply solid in this design system.
- [ ] Add visual tests for expected borders.

### TODO 18: Remove hardcoded backdrop coupling if themes own the desktop

Status: Open

Finding:
`Shell::add_default_backdrop` always adds hardcoded dark/blue/purple backdrop nodes after the CSS pipeline runs.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:886` starts `add_default_backdrop`.
- `crates/liquide-shell/src/shell/scene.rs:890` adds hardcoded dark background.
- `crates/liquide-shell/src/shell/scene.rs:904` adds hardcoded blue accent.
- `crates/liquide-shell/src/shell/scene.rs:918` adds hardcoded purple accent.

Impact:
Themes are not fully authoritative over the desktop backdrop.

Remediation:
- [ ] Decide if backdrop is fallback-only or permanent brand layer.
- [ ] If fallback-only, add it only when CSS background is absent.
- [ ] Add tests showing distinct theme backgrounds remain distinct.

### TODO 19: Implement or explicitly scope pseudo-element support

Status: Partial (::before/::after/::first-line/::first-letter now computed in cascade.rs; the `pseudo_elements_use_local_custom_properties` regression test remains #[ignore]d)

Finding:
Pseudo-element routing is not fully covered. A regression test for pseudo-element custom property routing remains ignored.

Evidence:
- `crates/liquide-style-engine/tests/t13_e5_regressions.rs:134` ignores `pseudo_elements_use_local_custom_properties`.
- `crates/liquide-style-engine/src/engine/cascade.rs:119` skips pseudo-element rules in normal style computation.

Impact:
CSS that depends on `::before`/`::after` content or scoped custom properties can be absent in rendered shell chrome.

Remediation:
- [ ] Define supported pseudo-elements for shell components.
- [ ] Route pseudo-element boxes through style, layout, paint, and hit-test as needed.
- [ ] Unignore or replace the ignored regression with working coverage.

### TODO 20: Finish transition and animation runtime coverage

Status: Open

Finding:
Some transition/animation tests assert only parsing/storage or are ignored. The integration test for transition override behavior is ignored.

Evidence:
- `crates/liquide-style-engine/src/engine/mod.rs:491` ignores `apply_transitions_detects_opacity_change`.
- `crates/liquide-style-engine/tests/css_feature_parity.rs:1284` prints transition storage rather than asserting behavior.
- `crates/liquide-style-engine/tests/css_feature_parity.rs:1293` prints animation storage rather than asserting behavior.

Impact:
Motion CSS can appear supported while frame-to-frame behavior is not proven.

Remediation:
- [ ] Add frame-based assertions for transition interpolation.
- [ ] Add animation scheduler coverage for computed keyframes and display output.
- [ ] Remove or replace ignored tests.

### TODO 21: Tighten miscellaneous CSS property completeness

Status: Open

Finding:
Some parsed properties are discarded or handled loosely.

Evidence:
- `list-style-image` is inherited at `crates/liquide-style-engine/src/inheritance.rs:34`.
- It is explicitly no-op applied at `crates/liquide-style-engine/src/engine/apply_ext.rs:1817`.
- `ComputedStyle` has list type/position but no list image at `crates/liquide-style-engine/src/computed/mod.rs:157`.
- `all` keyword handling uses substring matching at `crates/liquide-style-engine/src/engine/apply_ext.rs:19`.

Impact:
CSS completeness claims exceed actual computed/rendered behavior in edge cases.

Remediation:
- [ ] Add conformance tests around parsed-but-discarded properties.
- [ ] Fix `all` to use whole-value CSS-wide keyword matching.
- [ ] Document unsupported properties in one authoritative compatibility table.

## Verification Run During Audit

Passed:
- `cargo test -p liquide-style-engine -p liquide-theme-css -p liquide-renderer-css -p liquide-shell --lib`
- `cargo test -p liquide-protocol -p liquide-transport -p liquide-client -p liquide-gateway -p liquide-session -p liquide-supervisor --lib`
- `cargo test -p liquide-visual-test --test wiring_audit`

Warnings observed:
- `liquide-platform`: dead `handle` field.
- `liquide-shell`: dead `chrome_shell_services` field.
- `liquide-shell`: unused `tooltip_manager_opacity`.

Important note:
Passing unit suites do not prove the remote-first path works. The most severe findings are in binary/runtime wiring and cross-crate integration, not isolated library behavior.

# Pipeline Status (Updated 2026-02-19)

## Current State

The end-to-end shell rendering pipeline is operational:

- DOM -> Style -> Layout -> Paint -> Scene -> CPU renderer is working.
- Shell chrome text is rendering (dock labels, status bar text, launcher text).
- `liquide-shell` integration tests for rendering and pipeline stages pass.
- `cargo test --workspace --all-targets` passes.

## Recently Resolved

- Flaky `liquide-dom` event test (`events::tests::test_capturing`) fixed by serializing counter-based tests.
- Text rendering blocker documentation is outdated; integration tests now validate non-black pixel output and non-zero text nodes.
- `liquide-shell` -> `liquide-components` mapping in `sync_dom()` is implemented and active.
- Preferred color-scheme propagation and media query handling were wired through shell -> pipeline -> style engine.
- Thread coordinator frame-budget wait logic was corrected (single global 16ms deadline).

## Parity Improvements Landed

- Style value resolution:
  - named color parsing support in value resolution path
  - `resolve_color` support for `Keyword`/`String` property values
- Incremental pipeline:
  - style/layout/paint cache reuse
  - style invalidation path for dirty style nodes
  - layout incremental entrypoint (`relayout_subtree`) implemented for simple block-flow chains
- Layout API parity:
  - `LayoutInput`
  - `layout_with_input`
  - `relayout_subtree`
- Shell integration:
  - theme -> preferred color-scheme sync
  - sandbox registration/unregistration wiring for app windows
  - notification mapping parity for urgency/icon/actions payload fields

## Known Remaining Gaps (Not New Regressions)

The following advanced CSS/compositor features are still partial or missing in renderer/pipeline wiring:

1. `RenderLayer` isolated compositing groups
2. Non-rect `clip-path`
3. Full CSS `filter`/`backdrop-filter` raster pipeline
4. Full gradient/background stack emission from painter
5. `mask` and `border-image` rendering parity

## Risk Notes

- Thread coordinator output is now normalized and composited as fallback when main pipeline chrome output is empty. This avoids duplicate chrome while preserving threaded render-path integration.
- Incremental relayout currently targets simple block-flow ancestor chains; unsupported structures intentionally fall back to full layout for correctness.

## Validation Snapshot

Recent validation run:

- `cargo test --workspace --all-targets` -> pass
- `cargo test -p liquide-shell --test integration_rendering` -> pass (all tests)
- `cargo test -p liquide-shell --test pipeline_stages` -> pass (all tests)
- `cargo test -p liquide-layout --lib` -> pass


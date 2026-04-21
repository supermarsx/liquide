# CSS Conformance Test Fixtures

This directory contains fixture files for the CSS conformance test harness
in `crates/liquide-style-engine/src/tests/css_conformance.rs`.

## Structure

- `*.css` — CSS test input files
- `*.expected` — Expected computed values (property=value per line)

## Usage

The primary test harness uses inline CSS and programmatic DOM construction.
These fixture files are available for file-based test scenarios.

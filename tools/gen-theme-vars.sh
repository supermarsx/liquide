#!/usr/bin/env bash
# gen-theme-vars.sh -- Extract CSS custom properties from theme files
#
# Usage:
#   ./tools/gen-theme-vars.sh
#
# Scans assets/themes/*.css for :root custom properties and generates
# a Rust enum / map so themes can be validated at compile time.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

THEMES_DIR="$REPO_ROOT/assets/themes"
OUTPUT_FILE="$REPO_ROOT/crates/liquid-theme/src/generated_vars.rs"

echo "==> Scanning themes in $THEMES_DIR"

# TODO: Implement actual extraction.
# For each .css file, parse :root { --var-name: value; } blocks and collect
# all custom property names. Output a Rust file with:
#   pub const THEME_VARS: &[&str] = &["--var-one", "--var-two", ...];

for css in "$THEMES_DIR"/*.css; do
    echo "    Found: $(basename "$css")"
done

echo "==> Generation not yet implemented (stub)."
echo "    Would write to: $OUTPUT_FILE"

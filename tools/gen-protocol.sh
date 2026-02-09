#!/usr/bin/env bash
# gen-protocol.sh -- Regenerate protocol constants from the spec
#
# Usage:
#   ./tools/gen-protocol.sh [path/to/spec.toml]
#
# Reads the protocol specification file and outputs Rust constant definitions
# to crates/liquid-protocol/src/generated.rs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SPEC_FILE="${1:-$REPO_ROOT/spec/protocol.toml}"
OUTPUT_FILE="$REPO_ROOT/crates/liquid-protocol/src/generated.rs"

if [ ! -f "$SPEC_FILE" ]; then
    echo "Error: spec file not found at $SPEC_FILE" >&2
    exit 1
fi

echo "==> Generating protocol constants from $SPEC_FILE"
echo "    Output: $OUTPUT_FILE"

# TODO: Implement actual code generation.
# The generator should parse the TOML spec and emit:
#   - Message-type numeric constants
#   - CBOR tag values
#   - Error code enumerations
#   - Channel identifiers

echo "// @generated -- DO NOT EDIT (see tools/gen-protocol.sh)" > "$OUTPUT_FILE"
echo "// Generated from: $SPEC_FILE" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "// TODO: implement code generation" >> "$OUTPUT_FILE"

echo "==> Done"

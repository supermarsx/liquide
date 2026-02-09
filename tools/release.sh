#!/usr/bin/env bash
# release.sh -- Build and package release artifacts
#
# Usage:
#   ./tools/release.sh [--target <triple>] [--version <semver>]
#
# Steps:
#   1. Run full test suite
#   2. Build optimised release binaries
#   3. Strip debug symbols
#   4. Package for each enabled format (deb, rpm, tar.gz, Docker image)
#   5. Generate checksums

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
VERSION="${VERSION:-0.0.0}"
DIST_DIR="$REPO_ROOT/dist"

echo "==> Liquid Desktop release builder"
echo "    Version : $VERSION"
echo "    Target  : $TARGET"
echo "    Output  : $DIST_DIR"

# ---- Parse arguments ----
while [[ $# -gt 0 ]]; do
    case $1 in
        --target)  TARGET="$2"; shift 2;;
        --version) VERSION="$2"; shift 2;;
        *)         echo "Unknown option: $1" >&2; exit 1;;
    esac
done

# ---- Step 1: Tests ----
echo "==> Running test suite"
cargo test --workspace

# ---- Step 2: Build ----
echo "==> Building release binaries (target=$TARGET)"
cargo build --release --target "$TARGET"

# ---- Step 3: Strip ----
echo "==> Stripping binaries"
RELEASE_DIR="$REPO_ROOT/target/$TARGET/release"
strip "$RELEASE_DIR/liquid-desktopd" || true
strip "$RELEASE_DIR/liquid-session"  || true
strip "$RELEASE_DIR/liquid-client"   || true

# ---- Step 4: Package ----
mkdir -p "$DIST_DIR"

echo "==> Creating tarball"
tar -czf "$DIST_DIR/liquid-desktop-$VERSION-$TARGET.tar.gz" \
    -C "$RELEASE_DIR" \
    liquid-desktopd liquid-session liquid-client

# TODO: Build .deb, .rpm, Docker image, etc.
echo "==> TODO: deb / rpm / docker packaging not yet implemented"

# ---- Step 5: Checksums ----
echo "==> Generating checksums"
cd "$DIST_DIR"
sha256sum ./* > "SHA256SUMS"

echo "==> Release artifacts written to $DIST_DIR"

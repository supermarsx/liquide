#!/usr/bin/env bash
# build-linux.sh — Build LiquiDE on Linux (native) or verify prerequisites.
#
# Usage:
#   ./scripts/build-linux.sh          # check + build (debug)
#   ./scripts/build-linux.sh release  # check + build (release)
#   ./scripts/build-linux.sh check    # cargo check only (no link)
#
# Prerequisites (Debian/Ubuntu):
#   sudo apt-get install -y \
#       build-essential pkg-config \
#       libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev \
#       libwayland-dev libxkbcommon-dev \
#       cmake
#
# Prerequisites (Fedora/RHEL):
#   sudo dnf install -y \
#       gcc gcc-c++ pkg-config \
#       libX11-devel libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel \
#       wayland-devel libxkbcommon-devel \
#       cmake
#
# Prerequisites (Arch):
#   sudo pacman -S --needed \
#       base-devel pkg-config \
#       libx11 libxrandr libxinerama libxcursor libxi \
#       wayland libxkbcommon \
#       cmake

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ── Check we are on Linux ───────────────────────────────────────────────
if [[ "$(uname -s)" != "Linux" ]]; then
    error "This script must be run on Linux."
    echo "  For cross-compilation from Windows, use:"
    echo "    rustup target add x86_64-unknown-linux-gnu"
    echo "    cargo check --target x86_64-unknown-linux-gnu"
    exit 1
fi

# ── Check Rust toolchain ────────────────────────────────────────────────
if ! command -v rustc &>/dev/null; then
    error "Rust toolchain not found. Install via https://rustup.rs"
    exit 1
fi

RUST_VERSION=$(rustc --version | awk '{print $2}')
info "Rust version: $RUST_VERSION"

# ── Check system dependencies ───────────────────────────────────────────
MISSING=()

check_pkg() {
    if ! pkg-config --exists "$1" 2>/dev/null; then
        MISSING+=("$1")
    fi
}

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        MISSING+=("$1 (command)")
    fi
}

check_cmd pkg-config
check_cmd cc
check_cmd cmake

# X11 libraries (used by liquide-platform x11 backend)
check_pkg x11
check_pkg xrandr

# Wayland libraries (used by liquide-platform wayland backend)
check_pkg wayland-client

# xkbcommon (keyboard handling)
check_pkg xkbcommon

if [[ ${#MISSING[@]} -gt 0 ]]; then
    warn "Missing dependencies: ${MISSING[*]}"
    echo ""
    echo "  Debian/Ubuntu:"
    echo "    sudo apt-get install -y build-essential pkg-config cmake \\"
    echo "      libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev \\"
    echo "      libwayland-dev libxkbcommon-dev"
    echo ""
    echo "  Fedora/RHEL:"
    echo "    sudo dnf install -y gcc gcc-c++ pkg-config cmake \\"
    echo "      libX11-devel libXrandr-devel libXinerama-devel libXcursor-devel libXi-devel \\"
    echo "      wayland-devel libxkbcommon-devel"
    echo ""
    echo "  Arch:"
    echo "    sudo pacman -S --needed base-devel pkg-config cmake \\"
    echo "      libx11 libxrandr libxinerama libxcursor libxi \\"
    echo "      wayland libxkbcommon"
    echo ""
    error "Please install the missing dependencies and re-run."
    exit 1
fi

info "All system dependencies found."

# ── Build ────────────────────────────────────────────────────────────────
MODE="${1:-debug}"

cd "$(dirname "$0")/.."

case "$MODE" in
    check)
        info "Running cargo check..."
        cargo check --workspace
        info "cargo check passed."
        ;;
    release)
        info "Building in release mode..."
        cargo build --workspace --release
        info "Release build complete."
        ;;
    debug|*)
        info "Building in debug mode..."
        # Deep layout recursion in debug builds may need extra stack space.
        export RUST_MIN_STACK="${RUST_MIN_STACK:-8388608}"
        cargo build --workspace
        info "Debug build complete."
        ;;
esac

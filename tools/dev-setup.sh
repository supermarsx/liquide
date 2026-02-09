#!/usr/bin/env bash
# dev-setup.sh -- Bootstrap a development environment for Liquid Desktop
#
# Usage:
#   ./tools/dev-setup.sh
#
# This script:
#   1. Installs / updates the Rust toolchain via rustup
#   2. Installs required system dependencies (Debian/Ubuntu assumed)
#   3. Installs project-specific cargo tools (cargo-fuzz, criterion, etc.)
#   4. Sets up git pre-commit hooks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Installing / updating Rust toolchain"
if command -v rustup &>/dev/null; then
    rustup update stable
    rustup default stable
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

echo "==> Adding rustup components"
rustup component add clippy rustfmt

echo "==> Installing system dependencies (requires sudo)"
if command -v apt-get &>/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libwayland-dev \
        libxkbcommon-dev \
        libgbm-dev \
        libdrm-dev \
        libvulkan-dev \
        libpipewire-0.3-dev \
        libpam0g-dev \
        protobuf-compiler
fi

echo "==> Installing cargo tools"
cargo install cargo-fuzz cargo-watch cargo-deny

echo "==> Setting up pre-commit hooks"
HOOK_FILE="$REPO_ROOT/.git/hooks/pre-commit"
cat > "$HOOK_FILE" << 'HOOK'
#!/usr/bin/env bash
set -e
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
HOOK
chmod +x "$HOOK_FILE"

echo "==> Development environment ready!"

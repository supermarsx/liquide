#!/usr/bin/env bash
# dev.sh — Cross-platform developer task runner for LiquiDE (Linux/macOS entry point).
#
# Usage:
#   ./scripts/dev/dev.sh <task> [args...]
#   ./scripts/dev/dev.sh --help
#
# Tasks:
#   build        cargo build (debug by default; pass --release for release mode)
#   check        cargo check --all-targets (fast feedback, no codegen)
#   test         cargo test (pass -p <crate> and/or a test name filter)
#   fmt          cargo fmt (pass --check for CI verification mode)
#   lint         cargo clippy --all-targets (skips cleanly if clippy is missing)
#   run          launch the standalone DE binary (liquid-standalone); extra args are forwarded
#   run-example  run an example target (e.g. run-example optimizations); lists examples if omitted
#   snapshot     render the headless desktop to a PNG (fast eyeball-debug loop); args forwarded
#   help         show this help
#
# All tasks operate on the whole workspace unless a -p/--package argument is given.
# Extra arguments are forwarded to cargo verbatim. CARGO_TARGET_DIR is honored.
# The counterpart for Windows PowerShell is scripts/dev/dev.ps1.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo ""; echo -e "${CYAN}==>${NC} $*"; }

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# The standalone DE binary (crates/liquide-standalone, [[bin]] liquid-standalone).
STANDALONE_PACKAGE="liquide-standalone"
STANDALONE_BIN="liquid-standalone"

usage() {
    cat <<EOF
LiquiDE developer task runner

Usage: ./scripts/dev/dev.sh <task> [args...]

Tasks:
  build        cargo build (debug by default; pass --release for release mode)
  check        cargo check --all-targets (fast feedback, no codegen)
  test         cargo test (pass -p <crate> and/or a test name filter)
  fmt          cargo fmt (pass --check for CI verification mode)
  lint         cargo clippy --all-targets (skips cleanly if clippy is missing)
  run          launch the standalone DE binary (${STANDALONE_BIN}); extra args forwarded
  run-example  run an example target; lists available examples when none is given
  snapshot     render the headless desktop to a PNG (fast eyeball-debug loop)
  help         show this help

Examples:
  ./scripts/dev/dev.sh build --release
  ./scripts/dev/dev.sh test -p liquide-layout
  ./scripts/dev/dev.sh fmt --check
  ./scripts/dev/dev.sh run -- --help
  ./scripts/dev/dev.sh run-example optimizations

Snapshot (no window / GPU; writes target/visual-test/snapshot.png):
  ./scripts/dev/dev.sh snapshot                          # 1280x720 liquid-glass
  ./scripts/dev/dev.sh snapshot --theme night --width 800 --height 600
  ./scripts/dev/dev.sh snapshot --scenario context_menu
  ./scripts/dev/dev.sh snapshot --scenario status_bar

Windowed mode at 1270x768:
  # --dev-mode opens a resizable host window; --width/--height set its size.
  ./scripts/dev/dev.sh run -- --dev-mode --width 1270 --height 768
EOF
}

has_package_arg() {
    local arg
    for arg in "$@"; do
        case "$arg" in
            -p|--package|--package=*) return 0 ;;
        esac
    done
    return 1
}

run_cargo() {
    step "cargo $*"
    (cd "$REPO_ROOT" && cargo "$@")
}

list_examples() {
    local dir crate file found=0
    for dir in "$REPO_ROOT"/crates/*/examples; do
        [[ -d "$dir" ]] || continue
        crate="$(basename "$(dirname "$dir")")"
        for file in "$dir"/*.rs; do
            [[ -e "$file" ]] || continue
            echo "  $(basename "${file%.rs}")  (crate: $crate)"
            found=1
        done
    done
    if [[ "$found" -eq 0 ]]; then
        warn "No example targets found under crates/*/examples."
    else
        echo ""
        echo "Run one with: ./scripts/dev/dev.sh run-example <name>"
    fi
}

find_example_crate() {
    # Prints the crate directory name that owns example "$1", if any.
    local name="$1" dir file
    for dir in "$REPO_ROOT"/crates/*/examples; do
        [[ -d "$dir" ]] || continue
        for file in "$dir"/*.rs; do
            [[ -e "$file" ]] || continue
            if [[ "$(basename "${file%.rs}")" == "$name" ]]; then
                basename "$(dirname "$dir")"
                return 0
            fi
        done
    done
    return 1
}

# split_run_args: separates cargo build flags (--release) from program arguments.
# Results are written into BUILD_FLAGS and PROGRAM_ARGS arrays.
split_run_args() {
    BUILD_FLAGS=()
    PROGRAM_ARGS=()
    local arg
    for arg in "$@"; do
        case "$arg" in
            --) ;;
            --release) BUILD_FLAGS+=("$arg") ;;
            *) PROGRAM_ARGS+=("$arg") ;;
        esac
    done
}

TASK="${1:-help}"
if [[ $# -gt 0 ]]; then
    shift
fi

case "$TASK" in
    help|--help|-h)
        usage
        exit 0
        ;;
    build)
        cmd=(build)
        has_package_arg "$@" || cmd+=(--workspace)
        run_cargo "${cmd[@]}" "$@"
        ;;
    check)
        cmd=(check)
        has_package_arg "$@" || cmd+=(--workspace)
        cmd+=(--all-targets)
        run_cargo "${cmd[@]}" "$@"
        ;;
    test)
        cmd=(test)
        has_package_arg "$@" || cmd+=(--workspace)
        run_cargo "${cmd[@]}" "$@"
        ;;
    fmt)
        cmd=(fmt)
        has_package_arg "$@" || cmd+=(--all)
        run_cargo "${cmd[@]}" "$@"
        ;;
    lint)
        if ! cargo clippy -V >/dev/null 2>&1; then
            warn "cargo clippy is not installed (rustup component add clippy); skipping lint."
            exit 0
        fi
        cmd=(clippy)
        has_package_arg "$@" || cmd+=(--workspace)
        cmd+=(--all-targets)
        run_cargo "${cmd[@]}" "$@"
        ;;
    run)
        split_run_args "$@"
        cmd=(run -p "$STANDALONE_PACKAGE" --bin "$STANDALONE_BIN")
        cmd+=(${BUILD_FLAGS[@]+"${BUILD_FLAGS[@]}"})
        if [[ ${#PROGRAM_ARGS[@]} -gt 0 ]]; then
            cmd+=(-- "${PROGRAM_ARGS[@]}")
        fi
        run_cargo "${cmd[@]}"
        ;;
    run-example)
        if [[ $# -eq 0 || "${1:-}" == -* ]]; then
            echo -e "${CYAN}Available examples:${NC}"
            list_examples
            exit 0
        fi
        example_name="$1"
        shift
        if ! example_crate="$(find_example_crate "$example_name")"; then
            error "Unknown example '$example_name'."
            echo -e "${CYAN}Available examples:${NC}"
            list_examples
            exit 1
        fi
        split_run_args "$@"
        cmd=(run -p "$example_crate" --example "$example_name")
        cmd+=(${BUILD_FLAGS[@]+"${BUILD_FLAGS[@]}"})
        if [[ ${#PROGRAM_ARGS[@]} -gt 0 ]]; then
            cmd+=(-- "${PROGRAM_ARGS[@]}")
        fi
        run_cargo "${cmd[@]}"
        ;;
    snapshot)
        # Render the headless desktop to a PNG for the fast eyeball-debug loop.
        # Extra args (--theme/--width/--height/--scenario/--out) are forwarded
        # to the snapshot bin verbatim.
        split_run_args "$@"
        cmd=(run -p liquide-visual-test --bin snapshot)
        cmd+=(${BUILD_FLAGS[@]+"${BUILD_FLAGS[@]}"})
        if [[ ${#PROGRAM_ARGS[@]} -gt 0 ]]; then
            cmd+=(-- "${PROGRAM_ARGS[@]}")
        fi
        run_cargo "${cmd[@]}"
        ;;
    *)
        error "Unknown task '$TASK'."
        echo ""
        usage
        exit 1
        ;;
esac

#!/usr/bin/env bash
# Run the liquide-drm test suite inside a Linux container against either a
# real DRM device or a vkms-allocated virtual one.
#
# Prerequisites on the HOST (must be Linux):
#   - Docker installed and running.
#   - The `vkms` (Virtual KMS) kernel module loaded:
#         sudo modprobe vkms
#     This creates a virtual /dev/dri/cardN with no real display panel,
#     suitable for ioctl-level testing in CI without a GPU.
#   - Alternatively, a real GPU's /dev/dri/card0 (developer workstation).
#
# Usage:
#     ./scripts/docker/run-drm-tests.sh                  # auto-detect cardN
#     ./scripts/docker/run-drm-tests.sh /dev/dri/card1   # explicit device
#     DRM_DEVICE=/dev/dri/card0 ./scripts/docker/run-drm-tests.sh
#
# The container runs as the unprivileged `cargo` user inside the rust image
# but is granted access to the DRM device via `--device=`. We do NOT pass
# `--privileged` — only the specific char device node and the `video` group.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_TAG="liquide-drm-test:latest"
DOCKERFILE="$REPO_ROOT/scripts/docker/Dockerfile.drm"

# --- Pick the DRM device --------------------------------------------------
DRM_DEVICE="${1:-${DRM_DEVICE:-}}"
if [[ -z "$DRM_DEVICE" ]]; then
    # Auto-detect: prefer the lowest-numbered cardN that exists.
    for candidate in /dev/dri/card0 /dev/dri/card1 /dev/dri/card2; do
        if [[ -c "$candidate" ]]; then
            DRM_DEVICE="$candidate"
            break
        fi
    done
fi

if [[ -z "$DRM_DEVICE" || ! -c "$DRM_DEVICE" ]]; then
    cat >&2 <<EOF
ERROR: No DRM device available.

To create a virtual one for testing without a GPU, on the host run:
    sudo modprobe vkms
This loads the Virtual KMS kernel module which exposes /dev/dri/card0 with
a fully software-rendered output suitable for ioctl-level test coverage.

If vkms is built as a module on your distribution but not loaded, the above
modprobe call should succeed. On distributions without vkms in the default
kernel package, install kernel headers + build a vkms module, OR run these
tests in a Linux VM where vkms is available.

Hosts where this script CANNOT run:
- Windows / macOS (no Linux kernel = no /dev/dri).
  Use a Linux CI runner or a Linux VM instead.
- WSL2 (no /dev/dri exposure by default).
EOF
    exit 1
fi

# Verify the device is accessible.
if [[ ! -r "$DRM_DEVICE" ]]; then
    echo "WARNING: $DRM_DEVICE exists but is not readable. The container may need" >&2
    echo "         to run with --user 0 or be added to the 'video' group." >&2
fi

# --- Build the image (cached) --------------------------------------------
echo "==> Building $IMAGE_TAG from $DOCKERFILE"
docker build -f "$DOCKERFILE" -t "$IMAGE_TAG" "$REPO_ROOT"

# --- Run tests -----------------------------------------------------------
echo "==> Running liquide-drm tests against $DRM_DEVICE"

# Pass through additional cargo args after the device argument.
# Example: ./run-drm-tests.sh /dev/dri/card0 --lib --test-threads=1
shift || true
EXTRA_CARGO_ARGS=("$@")

# Mount the workspace read-only by default; cargo writes to a target dir
# that lives in a separate named volume so successive runs are cached.
docker run --rm -i \
    --device="$DRM_DEVICE" \
    --group-add="video" \
    -v "$REPO_ROOT:/workspace:ro" \
    -v liquide-drm-target:/workspace/target \
    -v liquide-drm-cargo-registry:/usr/local/cargo/registry \
    -e LIQUIDE_DRM_TEST_DEVICE="$DRM_DEVICE" \
    -e CARGO_TARGET_DIR=/workspace/target \
    -e RUST_BACKTRACE=1 \
    "$IMAGE_TAG" \
    cargo test -p liquide-drm "${EXTRA_CARGO_ARGS[@]:-}"

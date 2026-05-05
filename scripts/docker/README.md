# Docker-based DRM test harness

This directory hosts the container recipe and runner script that lets us
validate `liquide-drm` against a **real Linux DRM character device** (`/dev/dri/cardN`)
without depending on a developer-workstation GPU.

## Why this exists

`liquide-drm` is split across two test tiers:

1. **Host-safe synthetic tests** (already 66+ regressions passing in
   `crates/liquide-drm/src/tests.rs`) — exercise pure translation helpers,
   typed newtypes, encoded buffers, and the mockable ioctl backend (t40).
   These run on Windows, macOS, and Linux without any device access.
2. **Linux-only integration tests** (forthcoming, t31–t35) — exercise the
   real `libc::ioctl` syscall path: dumb-buffer alloc, AddFB2/RmFB lifecycle,
   page-flip submission, atomic commits, vblank waits.

Tier 2 needs a real DRM character device. Two practical paths:

- **Real GPU**: a developer workstation with `/dev/dri/card0`. Pass it into
  the container with `--device=/dev/dri/card0`. **You will be on the GPU's
  master at test time**, which can interfere with your active compositor.
  Run from a TTY or a dedicated test box.
- **vkms (Virtual KMS)** — a Linux kernel module that creates a virtual
  DRM device with no real display panel, designed exactly for this kind of
  testing. **Recommended for CI.** No GPU required.

## Setup

### Option A: vkms (recommended for CI / headless)

```bash
# On the host (Linux only):
sudo modprobe vkms

# Verify:
ls /dev/dri/                     # should show cardN, renderDN
lsmod | grep vkms                # should list the module
```

vkms is part of the upstream Linux kernel since 4.19 and ships in most
distributions' kernel packages. If `modprobe vkms` fails with "module not
found", check for a `linux-modules-extra-$(uname -r)` package (Debian/Ubuntu)
or build the module from kernel source.

### Option B: real GPU

Just verify `/dev/dri/card0` exists and is readable. Note that running DRM
master tests against your daily-driver GPU **will briefly take master from
your active compositor** — best done from a TTY.

## Running the suite

```bash
# Auto-detect /dev/dri/cardN:
./scripts/docker/run-drm-tests.sh

# Or pin to a specific node:
./scripts/docker/run-drm-tests.sh /dev/dri/card0

# Or pass through cargo args:
./scripts/docker/run-drm-tests.sh /dev/dri/card0 --lib --test-threads=1
```

The script builds `liquide-drm-test:latest` (Rust 1.82 + libc/libdrm headers)
and runs `cargo test -p liquide-drm` inside the container with the device
passed through and the `video` supplementary group granted.

## What's NOT supported

| Platform | Status | Reason |
|----------|--------|--------|
| Linux + vkms or real GPU | ✅ Supported | Standard path |
| Linux without vkms / GPU | ❌ Tier 2 unrunnable | No /dev/dri/cardN |
| WSL2 | ❌ | DRM nodes not exposed by default |
| macOS | ❌ | No Linux kernel |
| Windows host | ❌ for tier 2 | No Linux kernel; **tier 1 still runs natively via `cargo test -p liquide-drm` on Windows** |

For Windows-host development, tier 1's mock-ioctl regressions (added in t40)
provide unit-level coverage of the real-ioctl wiring without any device
access. Run them with the regular `cargo test -p liquide-drm` — no Docker
needed.

## CI integration

A GitHub Actions job that uses this harness should:

1. Run on `ubuntu-latest` (which has Docker preinstalled).
2. `sudo modprobe vkms` before the build step.
3. Invoke `./scripts/docker/run-drm-tests.sh` with no arguments
   (auto-detects `/dev/dri/card0` from vkms).
4. Cache the `liquide-drm-target` and `liquide-drm-cargo-registry` volumes
   to keep iteration fast.

The `vkms` module is reliably available on `ubuntu-latest` GitHub-hosted
runners (kernel ≥ 5.15, vkms in `linux-modules-extra`). Self-hosted Linux
runners should verify with `modprobe vkms` once at provisioning time.

## Troubleshooting

- **"failed to open /dev/dri/card0: Permission denied"** — the container
  user must be in the `video` group. The runner script passes
  `--group-add=video`, but if your distro uses a different group (e.g.
  `render` for renderDN nodes), update the script.
- **"No DRM device available"** — either no card node exists or vkms
  failed to load. Run `dmesg | grep vkms` on the host to check for
  load-time errors.
- **vkms-allocated tests pass locally but fail in CI** — vkms behaves
  identically across kernel versions in our test surface, but very old
  kernels (<5.4) don't support all atomic properties we exercise. Pin
  CI runners to ubuntu-22.04 or newer.

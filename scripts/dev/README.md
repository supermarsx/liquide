# LiquiDE developer scripts

A single dispatcher entry point per platform, mirroring the same task names so the
workflow is identical everywhere:

- **Windows (PowerShell):** `./scripts/dev/dev.ps1 <task> [args...]`
- **Linux/macOS (bash):** `./scripts/dev/dev.sh <task> [args...]`

A dispatcher (rather than many single-purpose scripts) was chosen to match the
existing repo style: `scripts/e2e.ps1` is already a single parameterized runner and
`scripts/build-linux.sh` already routes on a positional mode argument
(`debug`/`release`/`check`). One entry point per platform keeps `scripts/` flat and
the task list discoverable via `-Help` / `--help`.

Both scripts:

- operate on the whole workspace unless you pass `-p <crate>` (then the
  `--workspace`/`--all` flag is dropped automatically, since cargo rejects the
  combination),
- forward all extra arguments to cargo verbatim and propagate cargo's exit code,
- honor `CARGO_TARGET_DIR` if set in the environment,
- skip `lint` with a clear warning (exit 0) when the clippy component is not installed.

## Task reference

| Task | Windows | Linux/macOS |
|---|---|---|
| Build (debug) | `./scripts/dev/dev.ps1 build` | `./scripts/dev/dev.sh build` |
| Build (release) | `./scripts/dev/dev.ps1 build --release` | `./scripts/dev/dev.sh build --release` |
| Fast check | `./scripts/dev/dev.ps1 check` | `./scripts/dev/dev.sh check` |
| Test everything | `./scripts/dev/dev.ps1 test` | `./scripts/dev/dev.sh test` |
| Test one crate | `./scripts/dev/dev.ps1 test -p liquide-layout` | `./scripts/dev/dev.sh test -p liquide-layout` |
| Format | `./scripts/dev/dev.ps1 fmt` | `./scripts/dev/dev.sh fmt` |
| Format check (CI) | `./scripts/dev/dev.ps1 fmt --check` | `./scripts/dev/dev.sh fmt --check` |
| Lint (clippy) | `./scripts/dev/dev.ps1 lint` | `./scripts/dev/dev.sh lint` |
| Run the DE | `./scripts/dev/dev.ps1 run` | `./scripts/dev/dev.sh run` |
| List examples | `./scripts/dev/dev.ps1 run-example` | `./scripts/dev/dev.sh run-example` |
| Run an example | `./scripts/dev/dev.ps1 run-example optimizations` | `./scripts/dev/dev.sh run-example optimizations` |
| Help | `./scripts/dev/dev.ps1 -Help` | `./scripts/dev/dev.sh --help` |

## Copy-pasteable run examples

Build the workspace in release mode:

```powershell
./scripts/dev/dev.ps1 build --release
```

```bash
./scripts/dev/dev.sh build --release
```

Run the standalone desktop environment binary (`liquid-standalone` from the
`liquide-standalone` crate). Arguments after the task name are forwarded to the
binary itself:

```powershell
./scripts/dev/dev.ps1 run -- --help
./scripts/dev/dev.ps1 run --release
```

```bash
./scripts/dev/dev.sh run -- --help
./scripts/dev/dev.sh run --release
```

> Note: `liquid-standalone` is the TTY/DRM-KMS compositor; it is fully functional
> on a Linux console. On other platforms the command still builds and launches the
> binary, which reports what it can or cannot do on that host.

### Windowed mode at 1270x768

`--dev-mode` runs the compositor in a resizable host window (instead of
borderless fullscreen), and `--width`/`--height` set that window's size:

```powershell
./scripts/dev/dev.ps1 run -- --dev-mode --width 1270 --height 768
```

```bash
./scripts/dev/dev.sh run -- --dev-mode --width 1270 --height 768
```

> Note: `--dev-mode` is the windowed trigger (there is no separate `--windowed`
> flag). `--width`/`--height` are optional; when omitted the surface size tracks
> the primary output's mode (falling back to 1920x1080 when no mode metadata is
> available). The override also applies without `--dev-mode`, where it sets the
> fullscreen surface size.

Run a real example target (`optimizations` from `crates/liquide-renderer-cpu/examples/`):

```powershell
./scripts/dev/dev.ps1 run-example optimizations
```

```bash
./scripts/dev/dev.sh run-example optimizations
```

Run one crate's tests, optionally with a test name filter:

```powershell
./scripts/dev/dev.ps1 test -p liquide-layout
./scripts/dev/dev.ps1 test -p liquide-compositor damage
```

```bash
./scripts/dev/dev.sh test -p liquide-layout
./scripts/dev/dev.sh test -p liquide-compositor damage
```

Check formatting in CI mode (fails with a diff, changes nothing):

```powershell
./scripts/dev/dev.ps1 fmt --check
```

```bash
./scripts/dev/dev.sh fmt --check
```

Fast type-check of a single crate (handy before committing):

```powershell
./scripts/dev/dev.ps1 check -p liquide-common
```

```bash
./scripts/dev/dev.sh check -p liquide-common
```

## Related scripts

- `scripts/e2e.ps1` — manifest-driven end-to-end scenario runner.
- `scripts/build-linux.sh` — Linux prerequisite checks + workspace build.
- `scripts/docker/` — DRM test environment in Docker.

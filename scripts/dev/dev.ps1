#!/usr/bin/env pwsh
# dev.ps1 — Cross-platform developer task runner for LiquiDE (PowerShell entry point).
#
# Usage:
#   ./scripts/dev/dev.ps1 <task> [args...]
#   ./scripts/dev/dev.ps1 -Help
#
# Tasks:
#   build        cargo build (debug by default; pass --release for release mode)
#   check        cargo check --all-targets (fast feedback, no codegen)
#   test         cargo test (pass -p <crate> and/or a test name filter)
#   fmt          cargo fmt (pass --check for CI verification mode)
#   lint         cargo clippy --all-targets (skips cleanly if clippy is missing)
#   run          launch the standalone DE binary (liquid-standalone); extra args are forwarded
#   run-example  run an example target (e.g. run-example optimizations); lists examples if omitted
#   help         show this help
#
# All tasks operate on the whole workspace unless a -p/--package argument is given.
# Extra arguments are forwarded to cargo verbatim. CARGO_TARGET_DIR is honored.
# The counterpart for Linux/macOS shells is scripts/dev/dev.sh.

# Note: deliberately NOT an advanced function ([CmdletBinding()]), so that
# pass-through arguments such as "-p <crate>" reach $args verbatim instead of
# colliding with PowerShell common parameters (-p is ambiguous with
# -PipelineVariable / -ProgressAction on advanced functions).
param(
    [string]$Task,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$scriptRootPath = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($scriptRootPath)) {
    $scriptRootPath = Split-Path -Parent $MyInvocation.MyCommand.Path
}
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path (Join-Path $scriptRootPath "..") ".."))

# The standalone DE binary (crates/liquide-standalone, [[bin]] liquid-standalone).
$standalonePackage = "liquide-standalone"
$standaloneBin = "liquid-standalone"

function Show-Usage {
    Write-Host "LiquiDE developer task runner" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Usage: ./scripts/dev/dev.ps1 <task> [args...]"
    Write-Host ""
    Write-Host "Tasks:"
    Write-Host "  build        cargo build (debug by default; pass --release for release mode)"
    Write-Host "  check        cargo check --all-targets (fast feedback, no codegen)"
    Write-Host "  test         cargo test (pass -p <crate> and/or a test name filter)"
    Write-Host "  fmt          cargo fmt (pass --check for CI verification mode)"
    Write-Host "  lint         cargo clippy --all-targets (skips cleanly if clippy is missing)"
    Write-Host "  run          launch the standalone DE binary ($standaloneBin); extra args forwarded"
    Write-Host "  run-example  run an example target; lists available examples when none is given"
    Write-Host "  help         show this help"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  ./scripts/dev/dev.ps1 build --release"
    Write-Host "  ./scripts/dev/dev.ps1 test -p liquide-layout"
    Write-Host "  ./scripts/dev/dev.ps1 fmt --check"
    Write-Host "  ./scripts/dev/dev.ps1 run -- --help"
    Write-Host "  ./scripts/dev/dev.ps1 run-example optimizations"
    Write-Host ""
    Write-Host "Windowed mode at 1270x768:"
    Write-Host "  # --dev-mode opens a resizable host window; --width/--height set its size."
    Write-Host "  ./scripts/dev/dev.ps1 run -- --dev-mode --width 1270 --height 768"
}

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host ("==> {0}" -f $Message) -ForegroundColor Cyan
}

function Test-HasPackageArg {
    param([string[]]$Arguments)

    foreach ($argument in @($Arguments)) {
        if (($argument -eq "-p") -or ($argument -eq "--package") -or ($argument -like "--package=*")) {
            return $true
        }
    }

    return $false
}

function Invoke-Cargo {
    param([string[]]$Arguments)

    Write-Step ("cargo {0}" -f ($Arguments -join " "))
    Push-Location $repoRoot
    try {
        & cargo @Arguments
        if ($null -ne $LASTEXITCODE) {
            return [int]$LASTEXITCODE
        }
        return 0
    }
    finally {
        Pop-Location
    }
}

function Get-ExampleTargets {
    $targets = @()
    $cratesDir = Join-Path $repoRoot "crates"

    foreach ($crateDir in @(Get-ChildItem -Path $cratesDir -Directory)) {
        $examplesDir = Join-Path $crateDir.FullName "examples"
        if (-not (Test-Path $examplesDir)) {
            continue
        }
        foreach ($file in @(Get-ChildItem -Path $examplesDir -Filter "*.rs" -File)) {
            $targets += [pscustomobject]@{
                Crate   = $crateDir.Name
                Example = [System.IO.Path]::GetFileNameWithoutExtension($file.Name)
            }
        }
    }

    return $targets
}

function Show-ExampleTargets {
    $targets = @(Get-ExampleTargets)
    if ($targets.Count -eq 0) {
        Write-Host "No example targets found under crates/*/examples." -ForegroundColor Yellow
        return
    }

    Write-Host "Available examples:" -ForegroundColor Cyan
    foreach ($target in $targets) {
        Write-Host ("  {0}  (crate: {1})" -f $target.Example, $target.Crate)
    }
    Write-Host ""
    Write-Host "Run one with: ./scripts/dev/dev.ps1 run-example <name>"
}

function Split-RunArguments {
    # Separates cargo build flags (--release) from arguments forwarded to the program.
    param([string[]]$Arguments)

    $buildFlags = @()
    $programArgs = @()
    foreach ($argument in @($Arguments)) {
        if ($argument -eq "--") {
            continue
        }
        if ($argument -eq "--release") {
            $buildFlags += $argument
        }
        else {
            $programArgs += $argument
        }
    }

    return [pscustomobject]@{
        BuildFlags  = @($buildFlags)
        ProgramArgs = @($programArgs)
    }
}

$arguments = @()
foreach ($item in @($args)) {
    if ($null -ne $item) {
        $arguments += [string]$item
    }
}

if ($Help -or [string]::IsNullOrWhiteSpace($Task) -or
    ($Task -ieq "help") -or ($Task -eq "--help") -or ($Task -eq "-h")) {
    Show-Usage
    exit 0
}

$exitCode = 0

switch ($Task.ToLowerInvariant()) {
    "build" {
        $cargoArgs = @("build")
        if (-not (Test-HasPackageArg -Arguments $arguments)) {
            $cargoArgs += "--workspace"
        }
        $cargoArgs += $arguments
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "check" {
        $cargoArgs = @("check")
        if (-not (Test-HasPackageArg -Arguments $arguments)) {
            $cargoArgs += "--workspace"
        }
        $cargoArgs += "--all-targets"
        $cargoArgs += $arguments
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "test" {
        $cargoArgs = @("test")
        if (-not (Test-HasPackageArg -Arguments $arguments)) {
            $cargoArgs += "--workspace"
        }
        $cargoArgs += $arguments
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "fmt" {
        $cargoArgs = @("fmt")
        if (-not (Test-HasPackageArg -Arguments $arguments)) {
            $cargoArgs += "--all"
        }
        $cargoArgs += $arguments
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "lint" {
        & cargo clippy -V *> $null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "cargo clippy is not installed (rustup component add clippy); skipping lint." -ForegroundColor Yellow
            exit 0
        }

        $cargoArgs = @("clippy")
        if (-not (Test-HasPackageArg -Arguments $arguments)) {
            $cargoArgs += "--workspace"
        }
        $cargoArgs += "--all-targets"
        $cargoArgs += $arguments
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "run" {
        $split = Split-RunArguments -Arguments $arguments
        $cargoArgs = @("run", "-p", $standalonePackage, "--bin", $standaloneBin)
        $cargoArgs += $split.BuildFlags
        if ($split.ProgramArgs.Count -gt 0) {
            $cargoArgs += "--"
            $cargoArgs += $split.ProgramArgs
        }
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    "run-example" {
        if (($arguments.Count -eq 0) -or ($arguments[0] -like "-*")) {
            Show-ExampleTargets
            exit 0
        }

        $exampleName = $arguments[0]
        $remaining = @()
        if ($arguments.Count -gt 1) {
            $remaining = @($arguments[1..($arguments.Count - 1)])
        }

        $match = @(Get-ExampleTargets) | Where-Object { $_.Example -eq $exampleName } | Select-Object -First 1
        if ($null -eq $match) {
            Write-Host ("Unknown example '{0}'." -f $exampleName) -ForegroundColor Red
            Show-ExampleTargets
            exit 1
        }

        $split = Split-RunArguments -Arguments $remaining
        $cargoArgs = @("run", "-p", $match.Crate, "--example", $match.Example)
        $cargoArgs += $split.BuildFlags
        if ($split.ProgramArgs.Count -gt 0) {
            $cargoArgs += "--"
            $cargoArgs += $split.ProgramArgs
        }
        $exitCode = Invoke-Cargo -Arguments $cargoArgs
    }
    default {
        Write-Host ("Unknown task '{0}'." -f $Task) -ForegroundColor Red
        Write-Host ""
        Show-Usage
        exit 1
    }
}

exit $exitCode

#!/usr/bin/env pwsh

[CmdletBinding()]
param(
    [string[]]$Suite,
    [string[]]$Tier,
    [switch]$List,
    [switch]$ContinueOnFailure,
    [string]$OutputDir = "target/e2e",
    [string]$CargoTargetDir = "target/e2e/cargo-target",
    [switch]$NoCapture
)

$ErrorActionPreference = "Stop"

function ConvertTo-StringArray {
    param(
        $Value
    )

    $items = @()
    if ($null -eq $Value) {
        return $items
    }

    foreach ($item in @($Value)) {
        if ($null -ne $item) {
            $text = [string]$item
            if (-not [string]::IsNullOrWhiteSpace($text)) {
                $items += $text
            }
        }
    }

    return $items
}

function Get-CurrentPlatform {
    $isWindowsValue = $false
    $isLinuxValue = $false
    $isMacOSValue = $false

    $isWindowsVariable = Get-Variable -Name IsWindows -ErrorAction SilentlyContinue
    if ($null -ne $isWindowsVariable) {
        $isWindowsValue = [bool]$isWindowsVariable.Value
    }

    $isLinuxVariable = Get-Variable -Name IsLinux -ErrorAction SilentlyContinue
    if ($null -ne $isLinuxVariable) {
        $isLinuxValue = [bool]$isLinuxVariable.Value
    }

    $isMacOSVariable = Get-Variable -Name IsMacOS -ErrorAction SilentlyContinue
    if ($null -ne $isMacOSVariable) {
        $isMacOSValue = [bool]$isMacOSVariable.Value
    }

    if ($isWindowsValue) {
        return "windows"
    }
    if ($isLinuxValue) {
        return "linux"
    }
    if ($isMacOSValue) {
        return "macos"
    }

    $platform = [System.Environment]::OSVersion.Platform.ToString()
    switch ($platform) {
        "Win32NT" { return "windows" }
        "Win32S" { return "windows" }
        "Win32Windows" { return "windows" }
        "WinCE" { return "windows" }
        "MacOSX" { return "macos" }
        "Unix" {
            $uname = Get-Command uname -ErrorAction SilentlyContinue
            if ($null -ne $uname) {
                try {
                    $name = & uname -s 2>$null
                    if ($name -match "Darwin") {
                        return "macos"
                    }
                }
                catch {
                    return "linux"
                }
            }
            return "linux"
        }
        default { return $platform.ToLowerInvariant() }
    }
}

function Get-EntryPlatforms {
    param(
        $Entry
    )

    $platforms = @(ConvertTo-StringArray -Value $Entry.platforms)
    if ($platforms.Count -eq 0) {
        return @("all")
    }

    return $platforms
}

function Test-FilterMatch {
    param(
        [string]$Value,
        [string[]]$Filter
    )

    $filters = @()
    foreach ($filterItem in @(ConvertTo-StringArray -Value $Filter)) {
        foreach ($part in ($filterItem -split ',')) {
            if (-not [string]::IsNullOrWhiteSpace($part)) {
                $filters += $part.Trim()
            }
        }
    }
    if ($filters.Count -eq 0) {
        return $true
    }

    foreach ($filterValue in $filters) {
        if ($filterValue -ieq $Value) {
            return $true
        }
    }

    return $false
}

function Test-PlatformMatch {
    param(
        $Entry,
        [string]$CurrentPlatform
    )

    foreach ($platform in @(Get-EntryPlatforms -Entry $Entry)) {
        if (($platform -ieq "all") -or ($platform -ieq $CurrentPlatform)) {
            return $true
        }
    }

    return $false
}

function Get-EffectiveArgs {
    param(
        $Entry,
        [switch]$StripNoCapture
    )

    $args = @(ConvertTo-StringArray -Value $Entry.args)

    if ($StripNoCapture) {
        while (($args.Count -gt 0) -and ($args[$args.Count - 1] -ieq "--nocapture")) {
            if ($args.Count -eq 1) {
                $args = @()
            }
            else {
                $args = @($args[0..($args.Count - 2)])
            }
        }

        if (($args.Count -gt 0) -and ($args[$args.Count - 1] -eq "--")) {
            if ($args.Count -eq 1) {
                $args = @()
            }
            else {
                $args = @($args[0..($args.Count - 2)])
            }
        }
    }

    return $args
}

function ConvertTo-ProcessArgument {
    param(
        [string]$Argument
    )

    if ($null -eq $Argument) {
        return '""'
    }

    if ($Argument.Length -eq 0) {
        return '""'
    }

    if ($Argument -notmatch '[\s"]') {
        return $Argument
    }

    $escaped = $Argument -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

function Join-ProcessArguments {
    param(
        [string[]]$Arguments
    )

    $parts = @()
    foreach ($argument in @(ConvertTo-StringArray -Value $Arguments)) {
        $parts += (ConvertTo-ProcessArgument -Argument $argument)
    }

    return ($parts -join " ")
}

function Get-CommandLine {
    param(
        $Entry,
        [string[]]$Arguments
    )

    $parts = @((ConvertTo-ProcessArgument -Argument ([string]$Entry.command)))
    foreach ($argument in @(ConvertTo-StringArray -Value $Arguments)) {
        $parts += (ConvertTo-ProcessArgument -Argument $argument)
    }

    return ($parts -join " ")
}

function Write-Utf8NoBom {
    param(
        [string]$Path,
        [string]$Content
    )

    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Set-CargoTargetDirForEntry {
    param(
        [AllowNull()]
        [string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        return
    }

    $env:CARGO_TARGET_DIR = $Value
}

function Assert-ManifestEntry {
    param(
        $Entry
    )

    foreach ($field in @("id", "suite", "tier", "platforms", "description", "command", "args")) {
        if ($Entry.PSObject.Properties.Match($field).Count -eq 0) {
            throw "Manifest entry is missing required field '$field'."
        }
    }

    foreach ($field in @("id", "suite", "tier", "description", "command")) {
        if ([string]::IsNullOrWhiteSpace([string]$Entry.$field)) {
            throw "Manifest entry field '$field' must not be empty."
        }
    }
}

function Invoke-E2EEntry {
    param(
        $Entry,
        [string]$Root,
        [string]$LogsDir,
        [string]$CargoTargetDir,
        [switch]$StripNoCapture
    )

    $arguments = @(Get-EffectiveArgs -Entry $Entry -StripNoCapture:$StripNoCapture)
    $platforms = @(Get-EntryPlatforms -Entry $Entry)
    $commandLine = Get-CommandLine -Entry $Entry -Arguments $arguments
    $safeId = [regex]::Replace([string]$Entry.id, '[^A-Za-z0-9_.-]', '_')
    $logPath = Join-Path $LogsDir ($safeId + ".log")
    $startedAt = Get-Date
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $encoding = New-Object System.Text.UTF8Encoding($false)
    $logWriter = New-Object System.IO.StreamWriter($logPath, $false, $encoding)
    $exitCode = -1

    try {
        $logWriter.WriteLine("# $($Entry.id)")
        $logWriter.WriteLine("# $commandLine")
        if (-not [string]::IsNullOrWhiteSpace($CargoTargetDir)) {
            $logWriter.WriteLine("# CARGO_TARGET_DIR: $CargoTargetDir")
        }
        $logWriter.WriteLine("# Started: $($startedAt.ToUniversalTime().ToString("o"))")
        $logWriter.WriteLine("")
        $logWriter.Flush()

        Write-Host ""
        Write-Host ("==> {0}" -f $Entry.id) -ForegroundColor Cyan
        Write-Host ("    {0}" -f $commandLine)
        if (-not [string]::IsNullOrWhiteSpace($CargoTargetDir)) {
            Write-Host ("    CARGO_TARGET_DIR={0}" -f $CargoTargetDir)
        }

        $previousCargoTargetDir = [System.Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "Process")
        $restoreCargoTargetDir = -not [string]::IsNullOrWhiteSpace($previousCargoTargetDir)
        Push-Location $Root
        try {
            if (-not [string]::IsNullOrWhiteSpace($CargoTargetDir)) {
                Set-CargoTargetDirForEntry -Value $CargoTargetDir
            }
            else {
                Set-CargoTargetDirForEntry -Value $null
            }

            & ([string]$Entry.command) @arguments 2>&1 | ForEach-Object {
                $line = [string]$_
                Write-Host $line
                $logWriter.WriteLine($line)
                $logWriter.Flush()
            }

            if ($null -ne $LASTEXITCODE) {
                $exitCode = [int]$LASTEXITCODE
            }
            else {
                $exitCode = 0
            }
        }
        finally {
            if ($restoreCargoTargetDir) {
                Set-CargoTargetDirForEntry -Value $previousCargoTargetDir
            }
            else {
                Set-CargoTargetDirForEntry -Value $null
            }
            Pop-Location
        }
    }
    catch {
        $exitCode = -1
        $message = "Failed to run command for '$($Entry.id)': $($_.Exception.Message)"
        Write-Host $message -ForegroundColor Red
        $logWriter.WriteLine($message)
    }
    finally {
        $stopwatch.Stop()
        $durationMs = [int64]$stopwatch.ElapsedMilliseconds
        $logWriter.WriteLine("")
        $logWriter.WriteLine("# ExitCode: $exitCode")
        $logWriter.WriteLine("# DurationMs: $durationMs")
        $logWriter.Flush()
        $logWriter.Dispose()
    }

    return [pscustomobject]@{
        id = [string]$Entry.id
        suite = [string]$Entry.suite
        tier = [string]$Entry.tier
        platforms = @($platforms)
        commandLine = $commandLine
        exitCode = $exitCode
        durationMs = $durationMs
        logPath = $logPath
    }
}

$scriptRootPath = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($scriptRootPath)) {
    $scriptRootPath = Split-Path -Parent $MyInvocation.MyCommand.Path
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptRootPath ".."))
$manifestPath = Join-Path $scriptRootPath "e2e.manifest.json"

if (-not (Test-Path $manifestPath)) {
    throw "E2E manifest not found: $manifestPath"
}

$manifest = Get-Content -Path $manifestPath -Raw | ConvertFrom-Json
$entries = @($manifest)

foreach ($entry in $entries) {
    Assert-ManifestEntry -Entry $entry
}

$currentPlatform = Get-CurrentPlatform
$selectedEntries = @()

foreach ($entry in $entries) {
    if ((Test-FilterMatch -Value ([string]$entry.suite) -Filter $Suite) -and
        (Test-FilterMatch -Value ([string]$entry.tier) -Filter $Tier) -and
        (Test-PlatformMatch -Entry $entry -CurrentPlatform $currentPlatform)) {
        $selectedEntries += $entry
    }
}

if ($List) {
    Write-Host ("Manifest: {0}" -f $manifestPath)
    Write-Host ("Root:     {0}" -f $repoRoot)
    Write-Host ("Platform: {0}" -f $currentPlatform)
    Write-Host ("Selected: {0}" -f $selectedEntries.Count)
    Write-Host ""

    foreach ($entry in $selectedEntries) {
        $arguments = @(Get-EffectiveArgs -Entry $entry -StripNoCapture:$NoCapture)
        $platformText = (@(Get-EntryPlatforms -Entry $entry) -join ",")
        $commandLine = Get-CommandLine -Entry $entry -Arguments $arguments

        Write-Host ("{0} [{1}/{2}] ({3})" -f $entry.id, $entry.suite, $entry.tier, $platformText) -ForegroundColor Cyan
        Write-Host ("  {0}" -f $entry.description)
        Write-Host ("  {0}" -f $commandLine)
    }

    return
}

if ([System.IO.Path]::IsPathRooted($OutputDir)) {
    $resolvedOutputDir = [System.IO.Path]::GetFullPath($OutputDir)
}
else {
    $resolvedOutputDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDir))
}

if ([string]::IsNullOrWhiteSpace($CargoTargetDir)) {
    $resolvedCargoTargetDir = ""
}
elseif ([System.IO.Path]::IsPathRooted($CargoTargetDir)) {
    $resolvedCargoTargetDir = [System.IO.Path]::GetFullPath($CargoTargetDir)
}
else {
    $resolvedCargoTargetDir = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $CargoTargetDir))
}

$logsDir = Join-Path $resolvedOutputDir "logs"
New-Item -ItemType Directory -Force -Path $logsDir | Out-Null
if (-not [string]::IsNullOrWhiteSpace($resolvedCargoTargetDir)) {
    New-Item -ItemType Directory -Force -Path $resolvedCargoTargetDir | Out-Null
}

$results = @()
foreach ($entry in $selectedEntries) {
    $result = Invoke-E2EEntry -Entry $entry -Root $repoRoot -LogsDir $logsDir -CargoTargetDir $resolvedCargoTargetDir -StripNoCapture:$NoCapture
    $results += $result

    if (($result.exitCode -ne 0) -and (-not $ContinueOnFailure)) {
        break
    }
}

$failedCount = @($results | Where-Object { $_.exitCode -ne 0 }).Count
$summaryPath = Join-Path $resolvedOutputDir "summary.json"
$summary = [pscustomobject][ordered]@{
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
    root = $repoRoot
    cargoTargetDir = $resolvedCargoTargetDir
    selectedCount = $selectedEntries.Count
    failedCount = $failedCount
    results = @($results)
}

$summaryJson = $summary | ConvertTo-Json -Depth 10
Write-Utf8NoBom -Path $summaryPath -Content ($summaryJson + [System.Environment]::NewLine)

Write-Host ""
Write-Host ("Summary: {0}" -f $summaryPath)
Write-Host ("Selected: {0}; Ran: {1}; Failed: {2}" -f $selectedEntries.Count, $results.Count, $failedCount)

if ($failedCount -gt 0) {
    if ($ContinueOnFailure) {
        Write-Host "One or more e2e commands failed; ContinueOnFailure is set, so the runner exits 0." -ForegroundColor Yellow
        exit 0
    }

    exit 1
}

exit 0
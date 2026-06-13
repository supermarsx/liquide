#!/usr/bin/env pwsh
# download-fonts.ps1 — Downloads the required base fonts for the LiquiDE desktop.
#
# Fonts:
#   - Manrope         (primary UI)
#   - Inter            (data dense, fallback)
#   - Space Grotesk    (display / branding)
#   - JetBrains Mono   (terminal / code)
#   - Noto Sans        (accessibility)
#
# All fonts are SIL Open Font License (OFL) 1.1.
#
# NOTE: Running this script is now OPTIONAL for basic functionality. The desktop
# environment embeds a fallback UI font (Roboto Regular, Apache-2.0) directly in
# the binary via include_bytes! (see crates/liquide-font-rasterizer/src/database.rs,
# EMBEDDED_FALLBACK_FONT). If assets/fonts/ is empty, the DE registers that
# embedded font so text always renders with a real proportional face instead of
# the blocky 8x16 bitmap fallback. Run this script to install the full, higher-
# quality font set (Manrope/Inter/Space Grotesk/JetBrains Mono/Noto Sans), which
# the loader always PREFERS over the embedded fallback when present.
#
# Files are written to the exact names the loader expects
# (crates/liquide-font-rasterizer/src/database.rs::load_default_fonts), e.g.
# assets/fonts/Inter/InterVariable.ttf, assets/fonts/SpaceGrotesk/SpaceGrotesk-{weight}.ttf.

param(
    [string]$OutputDir = (Join-Path (Join-Path (Join-Path $PSScriptRoot "..") "assets") "fonts"),
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# Ensure output directory exists
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# Google Fonts download URLs (latest stable releases)
$fonts = @(
    @{
        Name    = "Manrope"
        Url     = "https://github.com/sharanda/manrope/releases/download/v4.504/Manrope%5Bwght%5D.ttf"
        File    = "Manrope-VariableFont_wght.ttf"
        License = "OFL-1.1"
    },
    @{
        Name    = "Inter"
        Url     = "https://github.com/rsms/inter/releases/download/v4.0/Inter-VariableFont_opsz%2Cwght.ttf"
        # The loader (database.rs load_default_fonts) reads Inter/InterVariable.ttf,
        # so save the variable font under exactly that name.
        File    = "InterVariable.ttf"
        License = "OFL-1.1"
    },
    @{
        Name    = "SpaceGrotesk"
        Url     = "https://github.com/floriankarsten/space-grotesk/releases/download/3.0.0/SpaceGrotesk%5Bwght%5D.ttf"
        File    = "SpaceGrotesk-VariableFont_wght.ttf"
        License = "OFL-1.1"
        # The loader expects per-weight static names; this is one variable file,
        # so copy it to each expected name. The shaper applies the wght variation.
        StaticCopies = @(
            "SpaceGrotesk-Light.ttf",
            "SpaceGrotesk-Regular.ttf",
            "SpaceGrotesk-Medium.ttf",
            "SpaceGrotesk-Bold.ttf"
        )
    },
    @{
        Name    = "JetBrainsMono"
        Url     = "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip"
        File    = "JetBrainsMono-2.304.zip"
        License = "OFL-1.1"
        Extract = $true
    },
    @{
        Name    = "NotoSans"
        Url     = "https://github.com/notofonts/latin-greek-cyrillic/releases/download/NotoSans-v2.015/NotoSans-v2.015.zip"
        File    = "NotoSans-v2.015.zip"
        License = "OFL-1.1"
        Extract = $true
    }
)

function Download-Font {
    param(
        [hashtable]$Font
    )
    
    $destPath = Join-Path $OutputDir $Font.Name
    New-Item -ItemType Directory -Force -Path $destPath | Out-Null

    $filePath = Join-Path $destPath $Font.File

    if ((Test-Path $filePath) -and -not $Force) {
        Write-Host "  [SKIP] $($Font.Name) already downloaded" -ForegroundColor Yellow
        return
    }

    Write-Host "  [GET]  $($Font.Name) ..." -ForegroundColor Cyan
    try {
        Invoke-WebRequest -Uri $Font.Url -OutFile $filePath -UseBasicParsing
    }
    catch {
        Write-Host "  [FAIL] $($Font.Name): $($_.Exception.Message)" -ForegroundColor Red
        return
    }

    if ($Font.Extract) {
        Write-Host "  [ZIP]  Extracting $($Font.Name) ..." -ForegroundColor Cyan
        $extractDir = Join-Path $destPath "extracted"
        Expand-Archive -Path $filePath -DestinationPath $extractDir -Force

        # Move TTF files to font directory
        Get-ChildItem -Path $extractDir -Filter "*.ttf" -Recurse | ForEach-Object {
            $target = Join-Path $destPath $_.Name
            Move-Item -Path $_.FullName -Destination $target -Force
        }
        
        # Clean up
        Remove-Item -Path $extractDir -Recurse -Force
    }

    # Some fonts ship as a single variable file but the loader expects several
    # per-weight static filenames. Duplicate the downloaded file under each
    # expected name so load_default_fonts() picks them up.
    if ($Font.StaticCopies) {
        foreach ($copyName in $Font.StaticCopies) {
            $copyPath = Join-Path $destPath $copyName
            if (Test-Path $filePath) {
                Copy-Item -Path $filePath -Destination $copyPath -Force
            }
        }
    }

    # Write license marker
    $licensePath = Join-Path $destPath "LICENSE.txt"
    if (-not (Test-Path $licensePath)) {
        @"
Font: $($Font.Name)
License: $($Font.License)
Source: $($Font.Url)

This font is licensed under the SIL Open Font License, Version 1.1.
http://scripts.sil.org/OFL
"@ | Set-Content -Path $licensePath
    }

    Write-Host "  [OK]   $($Font.Name)" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== LiquiDE Font Downloader ===" -ForegroundColor White
Write-Host "Output: $OutputDir" -ForegroundColor Gray
Write-Host ""

foreach ($font in $fonts) {
    Download-Font -Font $font
}

Write-Host ""
Write-Host "Done. Fonts saved to: $OutputDir" -ForegroundColor Green
Write-Host ""

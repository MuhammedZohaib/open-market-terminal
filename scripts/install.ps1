# Open Market Terminal (OMT) — One-line installer for Windows.
# Usage:
#   irm https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.ps1 | iex

[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir
)

$ErrorActionPreference = "Stop"

$repo = "MuhammedZohaib/open-market-terminal"

# Detect architecture
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -eq "ARM64") {
    $target = "aarch64-pc-windows-msvc"
} else {
    $target = "x86_64-pc-windows-msvc"
}

Write-Host "==> Detected target: $target" -ForegroundColor Cyan

# Resolve Version
if (-not $Version) {
    Write-Host "==> Resolving latest release from GitHub ($repo)..." -ForegroundColor Cyan
    try {
        $releaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -UseBasicParsing
        $Version = $releaseInfo.tag_name
    } catch {
        $Version = "v0.1.0"
        Write-Host "    (Could not query GitHub API; falling back to default $Version)" -ForegroundColor Yellow
    }
}

if (-not $Version.StartsWith("v")) {
    $Version = "v$Version"
}

Write-Host "    Target release: $Version" -ForegroundColor Green

# Resolve Installation Directory
if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\omt\bin"
}

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$zipName = "omt-$Version-$target.zip"
$downloadUrl = "https://github.com/$repo/releases/download/$Version/$zipName"
$shaUrl = "$downloadUrl.sha256"

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

try {
    $zipPath = Join-Path $tempDir $zipName
    $shaPath = Join-Path $tempDir "$zipName.sha256"

    Write-Host "==> Downloading $zipName..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing

    $hasSha = $false
    try {
        Invoke-WebRequest -Uri $shaUrl -OutFile $shaPath -UseBasicParsing
        $hasSha = $true
    } catch {
        # SHA file optional if not uploaded
    }

    if ($hasSha -and (Test-Path $shaPath)) {
        Write-Host "==> Verifying SHA256 checksum..." -ForegroundColor Cyan
        $expectedHash = (Get-Content $shaPath).Split(" `t", [System.StringSplitOptions]::RemoveEmptyEntries)[0].Trim().ToLowerInvariant()
        $actualHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()

        if ($expectedHash -ne $actualHash) {
            Write-Error "Checksum verification failed! Expected: $expectedHash, Actual: $actualHash"
            exit 1
        }
        Write-Host "    Checksum verified: OK" -ForegroundColor Green
    }

    Write-Host "==> Extracting into $InstallDir..." -ForegroundColor Cyan
    Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

    $sourceExe = Join-Path $tempDir "omt.exe"
    if (-not (Test-Path $sourceExe)) {
        # Check subfolder
        $nested = Get-ChildItem -Path $tempDir -Recurse -Filter "omt.exe" | Select-Object -First 1
        if ($nested) {
            $sourceExe = $nested.FullName
        }
    }

    if (-not (Test-Path $sourceExe)) {
        Write-Error "Error: omt.exe was not found inside the downloaded archive."
        exit 1
    }

    Copy-Item $sourceExe (Join-Path $InstallDir "omt.exe") -Force

    # Ensure $InstallDir is in PATH
    $userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
    $pathParts = $userPath -split ';'
    if ($pathParts -notcontains $InstallDir) {
        Write-Host "==> Adding $InstallDir to user PATH..." -ForegroundColor Cyan
        $newPath = "$userPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("Path", $newPath, [EnvironmentVariableTarget]::User)
        $env:PATH = "$env:PATH;$InstallDir"
        Write-Host "    Added $InstallDir to PATH (restart terminal to take full effect)." -ForegroundColor Green
    }

    Write-Host "==> Successfully installed Open Market Terminal (OMT) to $(Join-Path $InstallDir 'omt.exe')" -ForegroundColor Green
    Write-Host ""
    Write-Host "Quickstart:" -ForegroundColor Yellow
    Write-Host "  omt                   # Launch interactive terminal"
    Write-Host "  omt BTCUSDT           # Launch with BTCUSDT chart"
    Write-Host "  omt --help            # View all options"
} finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}

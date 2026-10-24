[CmdletBinding()]
param(
    [string]$Version,
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$BinaryPath,
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"

$workspaceRoot = (Get-Item $PSScriptRoot).Parent.FullName

if (-not $Version) {
    $cargoToml = Get-Content (Join-Path $workspaceRoot "Cargo.toml") -Raw
    if ($cargoToml -match '\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"') {
        $Version = "v$($matches[1])"
    } else {
        $Version = "v0.1.0"
    }
}
if (-not $Version.StartsWith("v")) {
    $Version = "v$Version"
}

if (-not $BinaryPath) {
    $candidates = @(
        (Join-Path $workspaceRoot "target\$Target\release\omt.exe"),
        (Join-Path $workspaceRoot "target\release\omt.exe")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            $BinaryPath = $candidate
            break
        }
    }
}

if (-not $BinaryPath -or -not (Test-Path $BinaryPath)) {
    Write-Error "Release binary not found. Please run 'cargo build --release' first or specify -BinaryPath."
    exit 1
}

$outPath = Join-Path $workspaceRoot $OutputDir
if (-not (Test-Path $outPath)) {
    New-Item -ItemType Directory -Path $outPath -Force | Out-Null
}

$packageName = "omt-$Version-$Target"
$stageDir = Join-Path $outPath $packageName
if (Test-Path $stageDir) {
    Remove-Item -Recurse -Force $stageDir
}
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null

Write-Host "Staging release package: $packageName"
Copy-Item $BinaryPath (Join-Path $stageDir "omt.exe")
Copy-Item (Join-Path $workspaceRoot "README.md") (Join-Path $stageDir "README.md")
Copy-Item (Join-Path $workspaceRoot "LICENSE") (Join-Path $stageDir "LICENSE")
Copy-Item (Join-Path $workspaceRoot "THIRD_PARTY_NOTICES.md") (Join-Path $stageDir "THIRD_PARTY_NOTICES.md")

$examplesDir = Join-Path $stageDir "examples"
New-Item -ItemType Directory -Path $examplesDir -Force | Out-Null
Copy-Item (Join-Path $workspaceRoot "examples\config.toml") (Join-Path $examplesDir "config.toml")

$zipName = "$packageName.zip"
$zipPath = Join-Path $outPath $zipName
if (Test-Path $zipPath) {
    Remove-Item -Force $zipPath
}

Write-Host "Creating archive: $zipPath"
Compress-Archive -Path "$stageDir\*" -DestinationPath $zipPath -CompressionLevel Optimal

# Compute SHA256 checksum
$hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$shaFile = "$zipPath.sha256"
"$hash  $zipName" | Set-Content -Path $shaFile -Encoding ASCII -NoNewline
"$hash  $zipName" | Write-Host

# Clean up staging directory
Remove-Item -Recurse -Force $stageDir

Write-Host "Successfully packaged $zipName (SHA256: $hash)"

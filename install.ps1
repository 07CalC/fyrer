# Install fyrer from GitHub Releases.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/07calc/fyrer/main/install.ps1 | iex"
#   powershell -File install.ps1 -Version v0.3.0
#   powershell -File install.ps1 -InstallDir "C:\tools\bin"

param(
  [string]$Version = "",
  [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"
$Repo = "07calc/fyrer"

if ([string]::IsNullOrEmpty($InstallDir)) {
  $InstallDir = Join-Path $HOME ".local\bin"
}

switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" { $Target = "x86_64-pc-windows-msvc" }
  "ARM64" { $Target = "aarch64-pc-windows-msvc" }
  default { Write-Error "unsupported architecture: $env:PROCESSOR_ARCHITECTURE"; exit 1 }
}

if ([string]::IsNullOrEmpty($Version)) {
  $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
  $Version = $release.tag_name
}

$Asset = "fyrer-$Target.zip"
$Base = "https://github.com/$Repo/releases/download/$Version"

$tmp = Join-Path $env:TEMP "fyrer-install-$PID"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
  $zip = Join-Path $tmp $Asset
  Write-Host "downloading $Base/$Asset"
  Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $zip

  $expected = ((Invoke-WebRequest -Uri "$Base/fyrer-$Target.sha256").Content -split "\s+")[0]
  $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLower()
  if ($actual -ne $expected.ToLower()) {
    throw "checksum mismatch: expected $expected, got $actual"
  }

  Expand-Archive -Path $zip -DestinationPath $tmp -Force

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Copy-Item -Path (Join-Path $tmp "fyrer.exe") -Destination (Join-Path $InstallDir "fyrer.exe") -Force

  Write-Host "installed fyrer $Version to $InstallDir"
  Write-Host "ensure $InstallDir is on your PATH"
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
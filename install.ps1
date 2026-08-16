$ErrorActionPreference = "Stop"

$Repo = "07calc/fyrer"


function Info($Message) {
    Write-Host "  " -NoNewline
    Write-Host "→" -ForegroundColor Cyan -NoNewline
    Write-Host " $Message"
}

function Success($Message) {
    Write-Host "  " -NoNewline
    Write-Host "✓" -ForegroundColor Green -NoNewline
    Write-Host " $Message"
}

function Warn($Message) {
    Write-Host "  " -NoNewline
    Write-Host "!" -ForegroundColor Yellow -NoNewline
    Write-Host " $Message"
}

function Fail($Message) {
    Write-Host "  " -NoNewline
    Write-Host "✗" -ForegroundColor Red -NoNewline
    Write-Host " $Message"
    exit 1
}

Write-Host ""
Write-Host "  Fyrer Installer" -ForegroundColor White
Write-Host "  Fast monorepo task runner" -ForegroundColor DarkGray
Write-Host ""


$Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture

switch ($Architecture) {
    "X64" {
        $Target = "x86_64-pc-windows-msvc"
        $Arch = "x86_64"
    }

    "Arm64" {
        $Target = "aarch64-pc-windows-msvc"
        $Arch = "aarch64"
    }

    default {
        Fail "Unsupported architecture: $Architecture"
    }
}

Info "Detected Windows $Arch"
Info "Target: $Target"


$Version = $env:FYRER_VERSION

if (-not $Version) {
    Info "Finding latest release..."

    $Release = Invoke-RestMethod `
        -Uri "https://api.github.com/repos/$Repo/releases/latest"

    $Version = $Release.tag_name
}

if (-not $Version) {
    Fail "Could not determine release version"
}

Success "Version $Version"

$BaseUrl = $env:FYRER_BASE_URL

if (-not $BaseUrl) {
    $BaseUrl = "https://github.com/$Repo/releases/download/$Version"
}

$Asset = "fyrer-$Target.exe"
$Checksum = "$Asset.sha256"


$TempDir = Join-Path $env:TEMP "fyrer-install-$([Guid]::NewGuid())"

New-Item `
    -ItemType Directory `
    -Path $TempDir `
    -Force | Out-Null

try {

    $BinaryPath = Join-Path $TempDir $Asset
    $ChecksumPath = Join-Path $TempDir $Checksum


    Info "Downloading Fyrer..."

    try {
        Invoke-WebRequest `
            -Uri "$BaseUrl/$Asset" `
            -OutFile $BinaryPath `
            -UseBasicParsing
    }
    catch {
        Fail "Failed to download $Asset"
    }


    Info "Verifying checksum..."

    try {
        Invoke-WebRequest `
            -Uri "$BaseUrl/$Checksum" `
            -OutFile $ChecksumPath `
            -UseBasicParsing

        $Expected = (Get-Content $ChecksumPath -Raw).Trim().Split(" ")[0]

        $Actual = (Get-FileHash `
            -Path $BinaryPath `
            -Algorithm SHA256).Hash

        if ($Expected.ToUpper() -ne $Actual.ToUpper()) {
            Fail "Checksum verification failed"
        }

        Success "Checksum verified"
    }
    catch {
        Warn "No checksum available for this release"
    }


    $InstallDir = $env:FYRER_INSTALL_DIR

    if (-not $InstallDir) {
        $InstallDir = Join-Path $env:LOCALAPPDATA "fyrer\bin"
    }

    Info "Installing to $InstallDir..."

    New-Item `
        -ItemType Directory `
        -Path $InstallDir `
        -Force | Out-Null

    Copy-Item `
        -Path $BinaryPath `
        -Destination (Join-Path $InstallDir "fyrer.exe") `
        -Force

    Success "Installed Fyrer $Version"


    $UserPath = [Environment]::GetEnvironmentVariable(
        "Path",
        "User"
    )

    $PathEntries = $UserPath -split ";" | Where-Object {
        $_ -ne ""
    }

    if ($PathEntries -notcontains $InstallDir) {

        Info "Adding Fyrer to your PATH..."

        $NewPath = ($PathEntries + $InstallDir) -join ";"

        [Environment]::SetEnvironmentVariable(
            "Path",
            $NewPath,
            "User"
        )

        Success "Added Fyrer to PATH"

        Warn "Restart your terminal for the PATH change to take effect."
    }
    else {
        Success "Fyrer is already on PATH"
    }

}
finally {
    Remove-Item `
        -Path $TempDir `
        -Recurse `
        -Force `
        -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "  Fyrer is ready!" -ForegroundColor Green
Write-Host ""
Write-Host "  Run " -NoNewline
Write-Host "fyrer --help" -ForegroundColor White -NoNewline
Write-Host " to get started."
Write-Host ""

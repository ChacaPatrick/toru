# Builds the release exe and compiles the installer into installer\output.
# Usage: powershell -ExecutionPolicy Bypass -File installer\build.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$iscc = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    throw 'ISCC.exe not found. Install Inno Setup 6: winget install --id JRSoftware.InnoSetup -e --scope user'
}

# The workspace version, from the first `version = "..."` in Cargo.toml.
$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

Push-Location $root
try {
    cargo build --release -p toru
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    & $iscc "/DAppVersion=$version" "$PSScriptRoot\toru.iss"
    if ($LASTEXITCODE -ne 0) { throw 'ISCC failed' }
} finally {
    Pop-Location
}

Write-Host "Installer: $PSScriptRoot\output\Toru-$version-setup.exe"

# Builds the release exe, writes the crates' licence notices and compiles the installer into
# installer\output.
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

if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
    throw 'cargo-about not found. It writes the third-party licence notices: cargo install cargo-about --locked --features cli'
}

# The workspace version, from the first `version = "..."` in Cargo.toml.
$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

Push-Location $root
try {
    cargo build --release -p toru
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    # The licence notices for the crates compiled into toru.exe (about.toml says which and how).
    # toru.iss ships the file; it isn't committed, so it always matches Cargo.lock.
    # cargo-about only warns, and still exits 0, when a clarification's checksum no longer
    # matches; it then drops that clarification's notices. A clean run prints nothing, so any
    # [WARN] or [ERROR] line fails the build. cmd collects stderr, which PowerShell 5.1 would
    # otherwise turn into error records; the colour codes are stripped before matching.
    $notices = 'target\THIRD-PARTY-NOTICES.html'
    $log = cmd /c "cargo about generate --fail --locked -c about.toml -m crates\toru\Cargo.toml -o $notices about.hbs 2>&1"
    $log = @($log) -replace '\x1b\[[0-9;]*m', ''
    $problems = $log | Where-Object { $_ -match '\[(WARN|ERROR)\]' }
    if ($LASTEXITCODE -ne 0 -or $problems) {
        $log | Write-Host
        Remove-Item $notices -ErrorAction SilentlyContinue
        throw 'cargo about generate reported problems (above); see about.toml'
    }
    & $iscc "/DAppVersion=$version" "$PSScriptRoot\toru.iss"
    if ($LASTEXITCODE -ne 0) { throw 'ISCC failed' }
} finally {
    Pop-Location
}

Write-Host "Installer: $PSScriptRoot\output\Toru-$version-setup.exe"

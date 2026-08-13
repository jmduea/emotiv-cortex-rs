<#
.SYNOPSIS
    Install neuroclient-tui from this repository checkout.

.DESCRIPTION
    Builds and installs the neuroclient-tui binary using cargo install.
    Use -Lsl to enable Lab Streaming Layer support (Windows/macOS only).

.PARAMETER Lsl
    Enable LSL (Lab Streaming Layer) support.

.PARAMETER InstallRoot
    Override the install root directory.
    Defaults to $env:NEUROCLIENT_INSTALL_ROOT, then the legacy
    $env:EMOTIV_CLI_INSTALL_ROOT, then $env:CARGO_HOME, then $HOME\.cargo.

.EXAMPLE
    .\scripts\install-neuroclient-tui.ps1
    # Install without LSL support

.EXAMPLE
    .\scripts\install-neuroclient-tui.ps1 -Lsl
    # Install with LSL support enabled
#>

[CmdletBinding()]
param(
    [switch]$Lsl,
    [string]$InstallRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir

# Verify cargo is available.
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo is not installed or not on PATH"
    exit 1
}

# Determine install root.
if (-not $InstallRoot) {
    $InstallRoot = if ($env:NEUROCLIENT_INSTALL_ROOT) {
        $env:NEUROCLIENT_INSTALL_ROOT
    }
    elseif ($env:EMOTIV_CLI_INSTALL_ROOT) {
        $env:EMOTIV_CLI_INSTALL_ROOT
    }
    elseif ($env:CARGO_HOME) {
        $env:CARGO_HOME
    }
    else {
        Join-Path $HOME '.cargo'
    }
}

# Build the cargo install arguments.
$cargoArgs = @(
    'install'
    '--path', (Join-Path $repoRoot 'crates' 'neuroclient-tui')
    '--root', $InstallRoot
    '--force'
)

if ($Lsl) {
    if ($IsLinux) {
        Write-Error "LSL is currently unsupported on Linux. Build without -Lsl, or use Windows/macOS."
        exit 1
    }
    $cargoArgs += '--features'
    $cargoArgs += 'lsl'
    Write-Host "Installing neuroclient-tui (with LSL) to: $InstallRoot\bin"
}
else {
    Write-Host "Installing neuroclient-tui to: $InstallRoot\bin"
    Write-Host "  Tip: use -Lsl to enable Lab Streaming Layer support" -ForegroundColor DarkGray
}

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error "cargo install failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

# Check if bin directory is on PATH.
$binDir = Join-Path $InstallRoot 'bin'
$pathDirs = $env:PATH -split [IO.Path]::PathSeparator
if ($binDir -notin $pathDirs) {
    Write-Host ""
    Write-Host "Add this to your profile to run neuroclient-tui from anywhere:"
    Write-Host "  `$env:PATH = `"$binDir;`$env:PATH`""
}

Write-Host ""
Write-Host "Installed. Try:"
Write-Host "  neuroclient-tui --help"

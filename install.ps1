#Requires -Version 5.1
<#
.SYNOPSIS
    Greppy Installer for Windows
.DESCRIPTION
    Downloads and installs the latest version of Greppy.
    Automatically stops any running daemon before installation.
.LINK
    https://github.com/KBLCode/greppy
.EXAMPLE
    irm https://raw.githubusercontent.com/KBLCode/greppy/main/install.ps1 | iex
#>

$ErrorActionPreference = "Stop"

$Repo = "KBLCode/greppy"
$InstallDir = if ($env:GREPPY_INSTALL_DIR) { $env:GREPPY_INSTALL_DIR } else { "$env:LOCALAPPDATA\greppy\bin" }
$GreppyHome = if ($env:GREPPY_HOME) { $env:GREPPY_HOME } else { "$env:USERPROFILE\.greppy" }

# ─────────────────────────────────────────────────────────────────────────────
# Output helpers
# ─────────────────────────────────────────────────────────────────────────────

function Write-Info  { Write-Host "[INFO] " -ForegroundColor Green -NoNewline; Write-Host $args }
function Write-Warn  { Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline; Write-Host $args }
function Write-Err   { Write-Host "[ERROR] " -ForegroundColor Red -NoNewline; Write-Host $args; exit 1 }
function Write-Step  { Write-Host "[STEP] " -ForegroundColor Blue -NoNewline; Write-Host $args }

# ─────────────────────────────────────────────────────────────────────────────
# Stop existing daemon (CRITICAL for clean upgrades)
# ─────────────────────────────────────────────────────────────────────────────

function Stop-ExistingDaemon {
    Write-Step "Checking for running greppy daemon..."

    # Method 1: Use existing greppy binary if available
    $greppy = Get-Command greppy -ErrorAction SilentlyContinue
    if ($greppy) {
        try {
            $status = & greppy status 2>$null
            if ($status -match "running") {
                Write-Warn "Stopping existing greppy daemon..."
                & greppy stop 2>$null
                Start-Sleep -Seconds 1
            }
        } catch {
            # Ignore errors
        }
    }

    # Method 2: Check PID file directly
    $pidFile = Join-Path $GreppyHome "daemon.pid"
    if (Test-Path $pidFile) {
        $pid = Get-Content $pidFile -ErrorAction SilentlyContinue
        if ($pid) {
            $proc = Get-Process -Id $pid -ErrorAction SilentlyContinue
            if ($proc) {
                Write-Warn "Killing daemon process $pid..."
                Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue
                Start-Sleep -Seconds 1
            }
        }
        Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
    }

    # Method 3: Clean up port file (Windows uses TCP)
    $portFile = Join-Path $GreppyHome "daemon.port"
    Remove-Item $portFile -Force -ErrorAction SilentlyContinue

    Write-Info "Daemon cleanup complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Download and install
# ─────────────────────────────────────────────────────────────────────────────

function Install-Greppy {
    # Detect architecture
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { 
        Write-Err "32-bit Windows is not supported"
    }
    $platform = "windows-$arch"
    Write-Info "Detected platform: $platform"

    # Get latest release
    Write-Step "Fetching latest release..."
    try {
        $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
        $version = $release.tag_name
    } catch {
        Write-Err "Failed to fetch latest release. Check your internet connection."
    }
    Write-Info "Latest version: $version"

    # Download
    $zipName = "greppy-$version-$platform.zip"
    $url = "https://github.com/$Repo/releases/download/$version/$zipName"
    
    $tmpDir = New-TemporaryFile | ForEach-Object { 
        Remove-Item $_
        New-Item -ItemType Directory -Path $_
    }
    $zipPath = Join-Path $tmpDir $zipName

    Write-Step "Downloading $zipName..."
    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing
    } catch {
        Write-Err "Download failed. URL: $url"
    }

    # Extract
    Write-Step "Extracting..."
    Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force

    # Install
    Write-Step "Installing to $InstallDir..."
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Move-Item (Join-Path $tmpDir "greppy.exe") (Join-Path $InstallDir "greppy.exe") -Force

    # Verify
    $greppyPath = Join-Path $InstallDir "greppy.exe"
    try {
        $versionOutput = & $greppyPath --version 2>$null
        Write-Info "Installation successful!"
    } catch {
        Write-Err "Installation verification failed"
    }

    # Clean up
    Remove-Item $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}

# ─────────────────────────────────────────────────────────────────────────────
# Post-install setup
# ─────────────────────────────────────────────────────────────────────────────

function Show-PostInstall {
    $greppyPath = Join-Path $InstallDir "greppy.exe"
    $version = & $greppyPath --version 2>$null | Select-Object -First 1

    Write-Host ""
    Write-Host "Installation complete!" -ForegroundColor Green
    Write-Host "  Version:  $version"
    Write-Host "  Location: $greppyPath"
    Write-Host ""

    # Check if install dir is in PATH
    if ($env:PATH -notlike "*$InstallDir*") {
        Write-Host "Add greppy to your PATH:" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "  # Run this command (or add to your profile):"
        Write-Host "  `$env:PATH += `";$InstallDir`""
        Write-Host ""
        Write-Host "  # Or permanently add via System Properties > Environment Variables"
        Write-Host ""
    }

    Write-Host "Quick start:" -ForegroundColor Blue
    Write-Host "  cd your-project"
    Write-Host "  greppy index              # Index your codebase"
    Write-Host "  greppy login              # (Optional) Enable AI reranking"
    Write-Host "  greppy search `"query`"     # Search!"
    Write-Host ""
    Write-Host "Commands:" -ForegroundColor Blue
    Write-Host "  greppy search <query>     # Semantic search (AI-powered)"
    Write-Host "  greppy search -d <query>  # Direct BM25 search (no AI)"
    Write-Host "  greppy start              # Start background daemon"
    Write-Host "  greppy logout             # Remove credentials"
    Write-Host ""
    Write-Host "Documentation: https://github.com/$Repo"
}

# ─────────────────────────────────────────────────────────────────────────────
# Logo
# ─────────────────────────────────────────────────────────────────────────────

function Show-Logo {
    $logo = @"
┌──────────────────────────────────────────────────┐
│ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
│██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
│██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
│██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
│╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
│ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
└──────────────────────────────────────────────────┘
"@
    Write-Host $logo
    Write-Host ""
    Write-Host "Sub-millisecond local semantic code search"
    Write-Host ""
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

Show-Logo

# CRITICAL: Stop daemon before replacing binary
Stop-ExistingDaemon

# Install new version
Install-Greppy

# Show post-install info
Show-PostInstall

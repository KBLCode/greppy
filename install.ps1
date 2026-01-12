# Greppy Windows Installer
# Run: irm https://raw.githubusercontent.com/KBLCode/greppy/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$REPO = "KBLCode/greppy"
$VERSION = "v0.8.0"
$TARGET = "x86_64-pc-windows-msvc"

function Write-Logo {
    Write-Host @"

 ┌──────────────────────────────────────────────────┐
 │ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
 │██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
 │██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
 │██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
 │╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
 │ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
 └──────────────────────────────────────────────────┘

"@ -ForegroundColor Cyan
    Write-Host " Sub-millisecond code search for AI tools" -ForegroundColor White
    Write-Host " ────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host ""
}

function Write-Step {
    param([string]$Message)
    Write-Host "✔ $Message" -ForegroundColor Green
}

function Write-Error {
    param([string]$Message)
    Write-Host "✖ $Message" -ForegroundColor Red
}

Write-Logo

Write-Step "Detecting platform..."
Write-Step "Platform: $TARGET"
Write-Step "Version: $VERSION"

# Create install directory
$InstallDir = "$env:LOCALAPPDATA\greppy"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Download
$Url = "https://github.com/$REPO/releases/download/$VERSION/greppy-$TARGET.zip"
$ZipPath = "$env:TEMP\greppy.zip"

Write-Step "Downloading greppy..."
try {
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
} catch {
    Write-Error "Failed to download release."
    Write-Host "The release might still be building. Please try again in a few minutes." -ForegroundColor Yellow
    exit 1
}

Write-Step "Downloaded successfully"

# Extract
Write-Step "Extracting..."
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force
Remove-Item $ZipPath

Write-Step "Installed to $InstallDir\greppy.exe"

# Add to PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Step "Adding to PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Step "Added to PATH (restart terminal to use)"
} else {
    Write-Step "Already in PATH"
}

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host " ────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Quick Start" -ForegroundColor White -NoNewline
Write-Host "" 
Write-Host ""
Write-Host "1. Restart your terminal (or run: `$env:Path = [Environment]::GetEnvironmentVariable('Path', 'User'))" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Start daemon & index your project" -ForegroundColor Gray
Write-Host "   " -NoNewline
Write-Host "greppy daemon start" -ForegroundColor Cyan
Write-Host "   " -NoNewline
Write-Host "cd C:\your\project; greppy index" -ForegroundColor Cyan
Write-Host ""
Write-Host "3. Search your code" -ForegroundColor Gray
Write-Host "   " -NoNewline
Write-Host 'greppy search "your query"' -ForegroundColor Cyan
Write-Host ""
Write-Host "Commands" -ForegroundColor White
Write-Host "  greppy daemon   Start/stop background daemon" -ForegroundColor Gray
Write-Host "  greppy index    Index a project (--watch for auto-update)" -ForegroundColor Gray
Write-Host "  greppy search   Search for code semantically" -ForegroundColor Gray
Write-Host "  greppy ask      Ask questions about codebase" -ForegroundColor Gray
Write-Host ""
Write-Host "Docs: https://github.com/$REPO" -ForegroundColor DarkGray

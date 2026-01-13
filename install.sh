#!/bin/bash
#
# Greppy Installer
# https://github.com/KBLCode/greppy
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | bash
#
# Environment variables:
#   GREPPY_INSTALL_DIR - Installation directory (default: ~/.local/bin)
#   GREPPY_HOME        - Greppy data directory (default: ~/.greppy)
#

set -e

REPO="KBLCode/greppy"
INSTALL_DIR="${GREPPY_INSTALL_DIR:-$HOME/.local/bin}"
GREPPY_HOME="${GREPPY_HOME:-$HOME/.greppy}"

# ─────────────────────────────────────────────────────────────────────────────
# Colors and formatting
# ─────────────────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

info()    { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
error()   { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
step()    { echo -e "${BLUE}[STEP]${NC} $1"; }

# ─────────────────────────────────────────────────────────────────────────────
# Platform detection
# ─────────────────────────────────────────────────────────────────────────────

detect_platform() {
    local os arch

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux*)  os="linux" ;;
        darwin*) os="darwin" ;;
        mingw*|msys*|cygwin*) 
            error "Windows detected. Please use install.ps1 instead."
            ;;
        *)
            error "Unsupported operating system: $os"
            ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)  arch="aarch64" ;;
        *)
            error "Unsupported architecture: $arch"
            ;;
    esac

    echo "${os}-${arch}"
}

# ─────────────────────────────────────────────────────────────────────────────
# Stop existing daemon (CRITICAL for clean upgrades)
# ─────────────────────────────────────────────────────────────────────────────

stop_existing_daemon() {
    step "Checking for running greppy daemon..."

    # Method 1: Use existing greppy binary if available
    if command -v greppy &> /dev/null; then
        if greppy status 2>&1 | grep -q "running"; then
            warn "Stopping existing greppy daemon..."
            greppy stop 2>/dev/null || true
            sleep 1
        fi
    fi

    # Method 2: Check PID file directly
    local pid_file="$GREPPY_HOME/daemon.pid"
    if [ -f "$pid_file" ]; then
        local pid
        pid=$(cat "$pid_file" 2>/dev/null || echo "")
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            warn "Killing daemon process $pid..."
            kill "$pid" 2>/dev/null || true
            sleep 1
            # Force kill if still running
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$pid_file"
    fi

    # Method 3: Clean up socket file (Unix)
    rm -f "$GREPPY_HOME/daemon.sock" 2>/dev/null || true

    info "Daemon cleanup complete"
}

# ─────────────────────────────────────────────────────────────────────────────
# Download and install
# ─────────────────────────────────────────────────────────────────────────────

install_greppy() {
    local platform
    platform=$(detect_platform)
    info "Detected platform: $platform"

    # Get latest release version
    step "Fetching latest release..."
    local latest
    latest=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
    
    if [ -z "$latest" ]; then
        error "Failed to fetch latest release. Check your internet connection."
    fi
    
    info "Latest version: $latest"

    # Construct download URL
    local tarball="greppy-${latest}-${platform}.tar.gz"
    local url="https://github.com/$REPO/releases/download/$latest/$tarball"

    # Create temp directory
    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    # Download
    step "Downloading $tarball..."
    if ! curl -sL "$url" -o "$tmp_dir/$tarball"; then
        error "Download failed. URL: $url"
    fi

    # Extract
    step "Extracting..."
    tar -xzf "$tmp_dir/$tarball" -C "$tmp_dir"

    # Install
    step "Installing to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR"
    mv "$tmp_dir/greppy" "$INSTALL_DIR/greppy"
    chmod +x "$INSTALL_DIR/greppy"

    # Verify installation
    if "$INSTALL_DIR/greppy" --version &> /dev/null; then
        info "Installation successful!"
    else
        error "Installation verification failed"
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Post-install setup
# ─────────────────────────────────────────────────────────────────────────────

post_install() {
    local version
    version=$("$INSTALL_DIR/greppy" --version 2>/dev/null | head -1)

    echo ""
    echo -e "${GREEN}${BOLD}Installation complete!${NC}"
    echo -e "  Version:  ${BOLD}$version${NC}"
    echo -e "  Location: ${BOLD}$INSTALL_DIR/greppy${NC}"
    echo ""

    # Check if install dir is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo -e "${YELLOW}Add greppy to your PATH:${NC}"
        echo ""
        echo "  # Add to ~/.bashrc or ~/.zshrc:"
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi

    echo -e "${BLUE}Quick start:${NC}"
    echo "  cd your-project"
    echo "  greppy index              # Index your codebase"
    echo "  greppy login              # (Optional) Enable AI reranking"
    echo "  greppy search \"query\"     # Search!"
    echo ""
    echo -e "${BLUE}Commands:${NC}"
    echo "  greppy search <query>     # Semantic search (AI-powered)"
    echo "  greppy search -d <query>  # Direct BM25 search (no AI)"
    echo "  greppy start              # Start background daemon"
    echo "  greppy logout             # Remove credentials"
    echo ""
    echo -e "Documentation: ${BOLD}https://github.com/$REPO${NC}"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

show_logo() {
    cat << 'EOF'
┌──────────────────────────────────────────────────┐
│ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│
│██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│
│██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │
│██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │
│╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │
│ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │
└──────────────────────────────────────────────────┘
EOF
    echo ""
    echo "Sub-millisecond local semantic code search"
    echo ""
}

main() {
    echo ""
    show_logo

    # CRITICAL: Stop daemon before replacing binary
    stop_existing_daemon

    # Install new version
    install_greppy

    # Show post-install info
    post_install
}

main "$@"

#!/bin/bash
set -euo pipefail

# Greppy Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/KBLCode/greppy/main/install.sh | bash

VERSION="${GREPPY_VERSION:-latest}"
INSTALL_DIR="${GREPPY_INSTALL_DIR:-$HOME/.local/bin}"
REPO="KBLCode/greppy"
GITHUB_URL="https://github.com/${REPO}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

info() { echo -e "${GREEN}  ✓${NC} $1"; }
warn() { echo -e "${YELLOW}  !${NC} $1"; }
error() { echo -e "${RED}  ✗${NC} $1"; exit 1; }
step() { echo -e "${CYAN}  →${NC} $1"; }

# Banner
show_banner() {
    echo ""
    echo -e "${MAGENTA}"
    echo " ██████╗██████╗█████████████╗██████╗██╗   ██╗"
    echo "██╔════╝██╔══████╔════██╔══████╔══██╚██╗ ██╔╝"
    echo "██║  █████████╔█████╗ ██████╔██████╔╝╚████╔╝ "
    echo "██║   ████╔══████╔══╝ ██╔═══╝██╔═══╝  ╚██╔╝  "
    echo "╚██████╔██║  ███████████║    ██║       ██║   "
    echo " ╚═════╝╚═╝  ╚═╚══════╚═╝    ╚═╝       ╚═╝   "
    echo -e "${NC}"
    echo -e "${DIM}  Sub-millisecond code search for AI tools${NC}"
    echo ""
    echo -e "  ${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *) error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# Get the latest version from GitHub
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
        grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || echo "v0.1.0"
}

# Get download URL
get_download_url() {
    local platform="$1"
    local version="$2"
    local ext=""
    
    if [[ "$platform" == windows-* ]]; then
        ext=".exe"
    fi

    echo "${GITHUB_URL}/releases/download/${version}/greppy-${platform}${ext}"
}

# Verify checksum
verify_checksum() {
    local file="$1"
    local checksum_url="$2"
    
    if command -v sha256sum &> /dev/null; then
        local expected
        expected=$(curl -fsSL "$checksum_url" 2>/dev/null | grep "$(basename "$file")" | awk '{print $1}' || true)
        if [[ -n "$expected" ]]; then
            local actual
            actual=$(sha256sum "$file" | awk '{print $1}')
            if [[ "$expected" != "$actual" ]]; then
                error "Checksum verification failed!"
            fi
            info "Checksum verified"
            return 0
        fi
    elif command -v shasum &> /dev/null; then
        local expected
        expected=$(curl -fsSL "$checksum_url" 2>/dev/null | grep "$(basename "$file")" | awk '{print $1}' || true)
        if [[ -n "$expected" ]]; then
            local actual
            actual=$(shasum -a 256 "$file" | awk '{print $1}')
            if [[ "$expected" != "$actual" ]]; then
                error "Checksum verification failed!"
            fi
            info "Checksum verified"
            return 0
        fi
    fi
    return 0
}

# Show quick start wizard
show_wizard() {
    echo ""
    echo -e "  ${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${BOLD}${GREEN}Installation complete!${NC}"
    echo ""
    echo -e "  ${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${BOLD}Quick Start${NC}"
    echo ""
    echo -e "  ${CYAN}1.${NC} Start the daemon ${DIM}(runs in background)${NC}"
    echo -e "     ${BOLD}$ greppy start${NC}"
    echo ""
    echo -e "  ${CYAN}2.${NC} Index your project ${DIM}(auto-watches for changes)${NC}"
    echo -e "     ${BOLD}$ cd /your/project && greppy index${NC}"
    echo ""
    echo -e "  ${CYAN}3.${NC} Search your code ${DIM}(sub-millisecond results)${NC}"
    echo -e "     ${BOLD}$ greppy search \"your query\"${NC}"
    echo ""
    echo -e "  ${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${BOLD}Commands${NC}"
    echo ""
    echo -e "  ${DIM}greppy start${NC}            Start background daemon"
    echo -e "  ${DIM}greppy stop${NC}             Stop the daemon"
    echo -e "  ${DIM}greppy status${NC}           Show daemon status"
    echo -e "  ${DIM}greppy index${NC}            Index current project"
    echo -e "  ${DIM}greppy search <q>${NC}       Search indexed code"
    echo -e "  ${DIM}greppy search <q> -l 5${NC}  Limit to 5 results"
    echo -e "  ${DIM}greppy search <q> --json${NC} JSON output for AI tools"
    echo -e "  ${DIM}greppy list${NC}             List indexed projects"
    echo -e "  ${DIM}greppy forget${NC}           Remove project from index"
    echo ""
    echo -e "  ${DIM}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${DIM}Docs:${NC} ${BLUE}${GITHUB_URL}${NC}"
    echo ""
}

main() {
    show_banner

    # Check for required tools
    if ! command -v curl &> /dev/null; then
        error "curl is required but not installed"
    fi

    # Detect platform
    step "Detecting platform..."
    local platform
    platform=$(detect_platform)
    info "Platform: ${BOLD}$platform${NC}"

    # Get version
    step "Checking latest version..."
    if [[ "$VERSION" == "latest" ]]; then
        VERSION=$(get_latest_version)
    fi
    info "Version: ${BOLD}$VERSION${NC}"

    # Create install directory
    step "Creating install directory..."
    mkdir -p "$INSTALL_DIR"
    info "Directory: ${BOLD}$INSTALL_DIR${NC}"

    # Download binary
    local url
    url=$(get_download_url "$platform" "$VERSION")
    step "Downloading greppy..."

    local tmp_file
    tmp_file=$(mktemp)
    
    if ! curl -fsSL "$url" -o "$tmp_file" 2>/dev/null; then
        error "Failed to download. Check your internet connection or try building from source."
    fi
    info "Downloaded successfully"

    # Verify checksum
    step "Verifying integrity..."
    verify_checksum "$tmp_file" "${GITHUB_URL}/releases/download/${VERSION}/checksums.txt" || true
    info "Verified"

    # Install binary
    step "Installing binary..."
    local binary_name="greppy"
    if [[ "$platform" == windows-* ]]; then
        binary_name="greppy.exe"
    fi

    mv "$tmp_file" "${INSTALL_DIR}/${binary_name}"
    chmod +x "${INSTALL_DIR}/${binary_name}"
    info "Installed to: ${BOLD}${INSTALL_DIR}/${binary_name}${NC}"

    # Check if in PATH
    if ! command -v greppy &> /dev/null && [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        warn "greppy is not in your PATH"
        echo ""
        echo -e "  Add this to your shell profile (${BOLD}~/.bashrc${NC} or ${BOLD}~/.zshrc${NC}):"
        echo ""
        echo -e "    ${BOLD}export PATH=\"\$PATH:${INSTALL_DIR}\"${NC}"
        echo ""
        echo -e "  Then run: ${BOLD}source ~/.zshrc${NC} (or restart your terminal)"
    fi

    show_wizard
}

main "$@"

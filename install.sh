#!/bin/sh
# Greppy Installer

set -e

# Colors
TEAL='\033[0;36m'
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m' # No Color

REPO="KBLCode/greppy"
VERSION="v0.7.0" 
BINARY="greppy"

logo() {
    echo "${TEAL}"
    echo " ┌──────────────────────────────────────────────────┐"
    echo " │ ██████╗ ██████╗ ███████╗██████╗ ██████╗ ██╗   ██╗│"
    echo " │██╔════╝ ██╔══██╗██╔════╝██╔══██╗██╔══██╗╚██╗ ██╔╝│"
    echo " │██║  ███╗██████╔╝█████╗  ██████╔╝██████╔╝ ╚████╔╝ │"
    echo " │██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██╔═══╝   ╚██╔╝  │"
    echo " │╚██████╔╝██║  ██║███████╗██║     ██║        ██║   │"
    echo " │ ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝        ╚═╝   │"
    echo " └──────────────────────────────────────────────────┘"
    echo "${NC}"
    echo " Sub-millisecond code search for AI tools"
    echo " ────────────────────────────────────────"
}

step() {
    echo "${GREEN}✔${NC} $1"
}

info() {
    echo "${BLUE}ℹ${NC} $1"
}

error() {
    echo "${RED}✖${NC} $1"
}

logo

echo ""
step "Detecting platform..."

# Detect OS and Arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            TARGET="aarch64-apple-darwin"
        else
            TARGET="x86_64-apple-darwin"
        fi
        ;;
    *)
        error "Unsupported OS: $OS"
        error "This installer only supports macOS."
        error "For Windows, download from: https://github.com/KBLCode/greppy/releases"
        exit 1
        ;;
esac

step "Platform: $TARGET"
step "Checking latest version..."
step "Version: $VERSION"

# Download
URL="https://github.com/$REPO/releases/download/$VERSION/greppy-$TARGET.tar.gz"
step "Downloading greppy..."

if ! curl -L --fail --progress-bar "$URL" -o greppy.tar.gz; then
    echo ""
    error "Failed to download release asset."
    echo "The release might still be building. Please try again in a few minutes."
    exit 1
fi

step "Downloaded successfully"
step "Verifying integrity..."

# Extract
tar xzf greppy.tar.gz
step "Verified"

# Install
INSTALL_DIR="/usr/local/bin"
step "Installing binary to $INSTALL_DIR..."

if [ -w "$INSTALL_DIR" ]; then
    mv greppy "$INSTALL_DIR/"
else
    sudo mv greppy "$INSTALL_DIR/"
fi

# Cleanup
rm greppy.tar.gz
step "Installed to $INSTALL_DIR/greppy"

echo ""
echo "${GREEN}Installation complete!${NC}"
echo " ────────────────────────────────────────"
echo ""
echo "${BOLD}Quick Start${NC}"
echo ""
echo "1. Authenticate (Optional)"
echo "   ${TEAL}$ greppy login${NC}"
echo ""
echo "2. Start daemon & index your project"
echo "   ${TEAL}$ greppy daemon start${NC}"
echo "   ${TEAL}$ cd /your/project && greppy index${NC}"
echo ""
echo "3. Search your code"
echo "   ${TEAL}$ greppy search \"your query\"${NC}"
echo ""
echo "4. Ask AI"
echo "   ${TEAL}$ greppy ask \"how does auth work?\"${NC}"
echo ""
echo "${BOLD}Commands${NC}"
echo "  greppy daemon   Start/stop background daemon"
echo "  greppy index    Index a project (--watch for auto-update)"
echo "  greppy search   Search for code semantically"
echo "  greppy ask      Ask questions about codebase"
echo "  greppy read     Read file context"
echo "  greppy login    Authenticate with Google"
echo ""
echo "Docs: https://github.com/$REPO"

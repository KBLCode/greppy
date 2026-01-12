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
VERSION="v0.5.0" 
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
}

logo

echo "${BOLD}Installing greppy $VERSION...${NC}"

# Detect OS and Arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        TARGET="x86_64-unknown-linux-gnu"
        ;;
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            TARGET="aarch64-apple-darwin"
        else
            TARGET="x86_64-apple-darwin"
        fi
        ;;
    *)
        echo "${RED}Unsupported OS: $OS${NC}"
        exit 1
        ;;
esac

echo "Detected target: ${BLUE}$TARGET${NC}"

# Download
URL="https://github.com/$REPO/releases/download/$VERSION/greppy-$TARGET.tar.gz"
echo "Downloading from ${BLUE}$URL${NC}..."

if ! curl -L --fail "$URL" -o greppy.tar.gz; then
    echo "${RED}Error: Failed to download release asset.${NC}"
    echo "The release might still be building. Please try again in a few minutes."
    exit 1
fi

# Extract
echo "Extracting..."
tar xzf greppy.tar.gz

# Install
echo "Installing to ${GREEN}/usr/local/bin${NC} (requires sudo)..."
if [ -w /usr/local/bin ]; then
    mv greppy /usr/local/bin/
else
    sudo mv greppy /usr/local/bin/
fi

# Cleanup
rm greppy.tar.gz

echo ""
echo "${GREEN}${BOLD}Success!${NC} Greppy has been installed."
echo "Run '${TEAL}greppy --help${NC}' to get started."

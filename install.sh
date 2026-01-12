#!/bin/sh
# Greppy Installer

set -e

REPO="KBLCode/greppy"
VERSION="v0.5.0" # Default, but script should ideally fetch latest
BINARY="greppy"

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
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

echo "Installing greppy $VERSION for $TARGET..."

# Download
URL="https://github.com/$REPO/releases/download/$VERSION/greppy-$TARGET.tar.gz"
echo "Downloading from $URL..."

curl -L "$URL" -o greppy.tar.gz

# Extract
tar xzf greppy.tar.gz

# Install
echo "Installing to /usr/local/bin (requires sudo)..."
if [ -w /usr/local/bin ]; then
    mv greppy /usr/local/bin/
else
    sudo mv greppy /usr/local/bin/
fi

# Cleanup
rm greppy.tar.gz

echo "Success! Run 'greppy --help' to get started."

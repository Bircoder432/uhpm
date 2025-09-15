#!/bin/bash
set -e

# Check for Rust and Cargo
if ! command -v cargo &> /dev/null; then
    echo "Rust and Cargo are not installed. Install Rust: https://www.rust-lang.org/tools/install"
    exit 1
fi

# Define directories
UHPM_DIR="$HOME/.uhpm"
PACKAGES_DIR="$UHPM_DIR/packages"
TMP_DIR="$UHPM_DIR/tmp"
LOCALE_SRC="locale"
LOCALE_DST="$UHPM_DIR/locale"
BIN_DIR="$HOME/.local/bin"

# Create necessary directories
mkdir -p "$PACKAGES_DIR" "$TMP_DIR" "$BIN_DIR"

# Build the project
echo "Building UHPM and utilities..."
cargo build --release

# Install or update binaries
for bin in uhpm uhpmk uhprepo; do
    SRC="target/release/$bin"
    DST="$BIN_DIR/$bin"

    if [ -f "$SRC" ]; then
        if [ -f "$DST" ]; then
            echo "Updating $bin..."
        else
            echo "Installing $bin..."
        fi
        cp "$SRC" "$DST"
    fi
done

# Copy locale folder to ~/.uhpm/locale
if [ -d "$LOCALE_SRC" ]; then
    echo "Copying locale files to $LOCALE_DST..."
    mkdir -p "$LOCALE_DST"
    cp -r "$LOCALE_SRC/"* "$LOCALE_DST/"
fi

# Generate shell completions
COMPLETIONS_DIR="$HOME/.config"
echo "Generating shell completions..."

# Bash
mkdir -p "$COMPLETIONS_DIR/bash/completions"
"$BIN_DIR/uhpm" completions bash > "$COMPLETIONS_DIR/bash/completions/uhpm.bash"

# Zsh
mkdir -p "$COMPLETIONS_DIR/zsh/completions"
"$BIN_DIR/uhpm" completions zsh > "$COMPLETIONS_DIR/zsh/completions/_uhpm"

# Fish
mkdir -p "$COMPLETIONS_DIR/fish/completions"
"$BIN_DIR/uhpm" completions fish > "$COMPLETIONS_DIR/fish/completions/uhpm.fish"

# Verify installation
echo "Verifying installation..."
for bin in uhpm uhpmk uhprepo; do
    if command -v "$BIN_DIR/$bin" &> /dev/null; then
        echo "$bin installed successfully: $($BIN_DIR/$bin --version || echo 'version unavailable')"
    else
        echo "Error installing $bin"
    fi
done

echo "Installation complete. UHPM is ready to use."

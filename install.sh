#!/bin/bash

# fdc installer script
set -e

echo "Installing fdc (Find Dead Code)..."

# Check if running on compatible system
if [[ "$OSTYPE" != "linux-gnu"* ]] && [[ "$OSTYPE" != "darwin"* ]]; then
    echo "This installer supports Linux and macOS only"
    exit 1
fi

# Create bin directory if it doesn't exist
mkdir -p "$HOME/bin"

# Copy binary
if [ -f "./target/release/fdc" ]; then
    cp ./target/release/fdc "$HOME/bin/fdc"
    chmod +x "$HOME/bin/fdc"
    echo "✅ fdc installed to $HOME/bin/fdc"
else
    echo "❌ Binary not found. Please run 'cargo build --release' first"
    exit 1
fi

# Add to PATH if not already there
if ! echo "$PATH" | grep -q "$HOME/bin"; then
    echo ""
    echo "Add this to your ~/.bashrc or ~/.zshrc:"
    echo "export PATH=\"\$HOME/bin:\$PATH\""
    echo ""
    echo "Then run: source ~/.bashrc (or ~/.zshrc)"
fi

echo "🎉 Installation complete! Run 'fdc --help' to get started."
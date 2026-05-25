#!/bin/bash
# Install AIntegriX as a systemd user service
set -e

INSTALL_DIR="${1:-/opt/aintegrix}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SYSTEMD_DIR="$HOME/.config/systemd/user"

echo "=== Installing AIntegriX to $INSTALL_DIR ==="

# Create directory
mkdir -p "$INSTALL_DIR"

# Copy binary
if [ -f "$PROJECT_DIR/target/release/aintegrix" ]; then
    cp "$PROJECT_DIR/target/release/aintegrix" "$INSTALL_DIR/"
else
    echo "Binary not found. Run: cargo build --release"
    exit 1
fi

# Copy config if not exists
[ -f "$INSTALL_DIR/aintegrix.yaml" ] || cp "$PROJECT_DIR/samples/aintegrix-local.yaml" "$INSTALL_DIR/aintegrix.yaml"

# Create .env if not exists
[ -f "$INSTALL_DIR/.env" ] || echo "AINTEGRIX_API_KEY=aintegrix-local-key-2026" > "$INSTALL_DIR/.env"

# Copy start script
cp "$SCRIPT_DIR/start.sh" "$INSTALL_DIR/"

# Install systemd service
mkdir -p "$SYSTEMD_DIR"
sed "s|/opt/aintegrix|$INSTALL_DIR|g" "$SCRIPT_DIR/aintegrix-local.service" > "$SYSTEMD_DIR/aintegrix.service"
systemctl --user daemon-reload
systemctl --user enable aintegrix
systemctl --user start aintegrix

sleep 2
if curl -s http://127.0.0.1:8050/health > /dev/null 2>&1; then
    echo "✅ AIntegriX installed and running"
    echo ""
    echo "Commands:"
    echo "  systemctl --user status aintegrix"
    echo "  systemctl --user restart aintegrix"
    echo "  systemctl --user stop aintegrix"
    echo "  journalctl --user -u aintegrix -f"
else
    echo "❌ Failed. Check: journalctl --user -u aintegrix --no-pager"
    exit 1
fi

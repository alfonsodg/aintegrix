#!/bin/bash
# AIntegriX Local Setup & Start
# Run once to install, then use systemctl to manage
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SERVICE_FILE="$SCRIPT_DIR/aintegrix-local.service"
SYSTEMD_DIR="$HOME/.config/systemd/user"

echo "=== AIntegriX Local Setup ==="
echo "Project: $PROJECT_DIR"

# 1. Build release
echo "[1/4] Building release..."
cd "$PROJECT_DIR"
cargo build --release 2>&1 | tail -1

# 2. Install systemd user service
echo "[2/4] Installing systemd user service..."
mkdir -p "$SYSTEMD_DIR"
cp "$SERVICE_FILE" "$SYSTEMD_DIR/aintegrix.service"
systemctl --user daemon-reload

# 3. Enable and start
echo "[3/4] Starting service..."
systemctl --user enable aintegrix
systemctl --user start aintegrix

# 4. Verify
sleep 2
echo "[4/4] Verifying..."
if curl -s http://127.0.0.1:8050/health > /dev/null 2>&1; then
    echo ""
    echo "✅ AIntegriX running at http://localhost:8050"
    echo ""
    echo "MCP config for your agent:"
    echo '  {'
    echo '    "mcpServers": {'
    echo '      "aintegrix": {'
    echo '        "url": "http://localhost:8050/mcp",'
    echo '        "type": "sse",'
    echo '        "headers": {"Authorization": "Bearer aintegrix-local-key-2026"}'
    echo '      }'
    echo '    }'
    echo '  }'
    echo ""
    echo "Commands:"
    echo "  systemctl --user status aintegrix"
    echo "  systemctl --user restart aintegrix"
    echo "  systemctl --user stop aintegrix"
    echo "  journalctl --user -u aintegrix -f"
else
    echo "❌ Failed to start. Check logs:"
    echo "  journalctl --user -u aintegrix --no-pager"
    exit 1
fi

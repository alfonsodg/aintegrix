#!/bin/bash
# Run AIntegriX locally
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG="$PROJECT_DIR/samples/aintegrix-local.yaml"
BINARY="$PROJECT_DIR/target/release/aintegrix"

export AINTEGRIX_API_KEY="${AINTEGRIX_API_KEY:-aintegrix-local-key-2026}"

# Build if binary doesn't exist or source is newer
if [ ! -f "$BINARY" ] || [ "$(find "$PROJECT_DIR/src" -newer "$BINARY" | head -1)" ]; then
    echo "Building release..."
    cd "$PROJECT_DIR" && cargo build --release 2>&1 | tail -1
fi

echo "Starting AIntegriX (http://localhost:8050)"
echo "Config: $CONFIG"
echo "API Key: $AINTEGRIX_API_KEY"
echo "Press Ctrl+C to stop"
echo ""

exec "$BINARY" --config "$CONFIG"

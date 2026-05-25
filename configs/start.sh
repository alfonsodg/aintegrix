#!/bin/bash
# Start AIntegriX server
DIR="$(cd "$(dirname "$0")" && pwd)"
export AINTEGRIX_API_KEY="${AINTEGRIX_API_KEY:-aintegrix-local-key-2026}"
exec "$DIR/aintegrix" --config "$DIR/aintegrix.yaml"

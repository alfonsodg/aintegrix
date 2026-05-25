# Installation

## From Source

```bash
# Requirements: Rust toolchain (latest stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone git@scovil.labtau.com:ccvass/model-shared/aintegrix.git
cd aintegrix
cargo build --release

# Binary at: target/release/aintegrix
```

## Deploy (systemd)

```bash
# Copy binary
sudo mkdir -p /opt/aintegrix
sudo cp target/release/aintegrix /opt/aintegrix/

# Copy config
sudo cp samples/aintegrix.yaml /opt/aintegrix/aintegrix.yaml
# Edit with your agent paths and API keys

# Create .env
echo "AINTEGRIX_API_KEY=your-secret-key" > /opt/aintegrix/.env

# Install service
sudo cp configs/aintegrix.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now aintegrix

# Verify
curl http://localhost:8050/health
```

## Nginx + SSL

```bash
# Copy nginx config
sudo cp configs/nginx/coord-acp.conf /etc/nginx/sites-available/
sudo ln -sf /etc/nginx/sites-available/coord-acp.conf /etc/nginx/sites-enabled/

# Get SSL certificate
sudo certbot --nginx -d coord-acp.apulab.info

# Reload
sudo nginx -t && sudo systemctl reload nginx
```

## Agent Prerequisites

Agents must be installed on the same server:

| Agent | Install |
|-------|---------|
| Kiro CLI | `curl -fsSL https://cli.kiro.dev/install \| bash` |
| Copilot | `npm install -g @anthropic-ai/copilot-cli` |
| OpenCode | `npm install -g opencode-ai` |
| Claude adapter | `npm install -g @agentclientprotocol/claude-agent-acp` |
| Codex adapter | `npm install -g @agentclientprotocol/codex-acp` |

Each agent must be authenticated independently (`kiro-cli login`, `copilot login`, etc.).

## CLI Tool

```bash
# Build CLI
cargo build --release --bin aintegrix-cli

# Use
aintegrix-cli --url https://coord-acp.apulab.info --token <key> status
aintegrix-cli agents
aintegrix-cli prompt kiro "fix the bug"
```

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

## Run Locally

For local development, AIntegriX can run on your machine with direct filesystem access:

```bash
# Set API key
export AINTEGRIX_API_KEY=local-dev-key

# Run with local config
cargo run -- --config samples/aintegrix-local.yaml
```

### Local vs Remote

| | Local | Remote (dev-gcp) |
|---|---|---|
| URL | `http://localhost:8050` via mcp-proxy | `https://coord-acp.apulab.info/mcp` |
| MCP config | `command: mcp-proxy` (stdio) | `type: http` (direct) |
| Code access | `workspace_root` with local paths | `workspace_root` with repo path (auto-clones) |
| Needs push | No | Yes |
| Sees uncommitted changes | Yes | No |
| Tool schema | Only `agent`, `workspace_root`, `model` | Adds `branch` param |
| Config | `samples/aintegrix-local.yaml` | `/opt/aintegrix/aintegrix.yaml` |

### MCP config for agents

**Local (via mcp-proxy — required for Kiro/agents that expect stdio):**
```json
{
  "mcpServers": {
    "aintegrix": {
      "command": "mcp-proxy",
      "args": ["-H", "Authorization", "Bearer aintegrix-local-key-2026", "--transport", "streamablehttp", "http://localhost:8050/mcp"],
      "env": {}
    }
  }
}
```

**Remote (direct HTTP):**
```json
{
  "mcpServers": {
    "aintegrix": {
      "url": "https://coord-acp.apulab.info/mcp",
      "type": "http",
      "headers": {
        "Authorization": "Bearer aintegrix-dev-key-2026"
      }
    }
  }
}
```

Requires: `npm install -g mcp-proxy`

> **Why mcp-proxy for local?** Agents like Kiro expect stdio or attempt OAuth on HTTP URLs. `mcp-proxy` bridges stdio ↔ HTTP, bypassing OAuth discovery on localhost.

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

## SSH Access for Repo Cloning

AIntegriX can auto-clone repos when creating sessions. Configure SSH:

```bash
# Ensure the server has SSH access to GitLab
cat ~/.ssh/config
# Should have:
# Host scovil.labtau.com
#     HostName 35.192.105.88
#     User git
#     IdentityFile ~/.ssh/id_rsa
#     IdentitiesOnly yes

# Test access
ssh -T git@scovil.labtau.com
# → Welcome to GitLab, @supergod!
```

## CLI Tool

```bash
# Build CLI
cargo build --release --bin aintegrix-cli

# Use
aintegrix-cli --url https://coord-acp.apulab.info --token <key> status
aintegrix-cli agents
aintegrix-cli prompt kiro "fix the bug"
```

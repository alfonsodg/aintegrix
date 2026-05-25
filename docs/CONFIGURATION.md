# Configuration

AIntegriX is configured via a single YAML file. Default path: `/etc/aintegrix/aintegrix.yaml`

## Full Schema

```yaml
server:
  host: "127.0.0.1"        # Bind address
  port: 8050                # Listen port

logging:
  level: info               # debug, info, warn, error
  format: json              # json or text

agents:
  <name>:
    command: /path/to/binary  # Agent binary path
    args: [acp]               # Arguments
    mode: native              # native or adapter
    max_sessions: 3           # Max concurrent sessions
    auto_restart: true        # Restart on crash
    default_model: model-name # Default model
    models:                   # Available models
      - model-a
      - model-b
    env:                      # Extra env vars for agent
      API_KEY: ${ENV_VAR}

permissions:
  default_policy: deny        # deny or approve
  auto_approve:
    - fs/read_text_file
  require_approval:
    - fs/write_text_file
    - terminal/create
  always_deny:
    - terminal/kill
```

## Environment Variable Interpolation

Use `${VAR_NAME}` in any string value. Resolved at load time from process environment.

```yaml
env:
  ANTHROPIC_API_KEY: ${ANTHROPIC_API_KEY}
```

## Authentication

Set `AINTEGRIX_API_KEY` environment variable. All `/api/*` and `/mcp` endpoints require `Authorization: Bearer <token>`. Health endpoints (`/health`, `/readiness`, `/metrics`) are public.

## Agent Modes

- **native**: Binary speaks ACP directly on stdio (kiro-cli, copilot, opencode)
- **adapter**: Wrapper binary that translates to ACP (claude-agent-acp, codex-acp)

## Model Selection

Each agent has a `default_model` and a list of available `models`. Clients can override the model when creating a session:

```json
{"agent": "kiro", "model": "claude-opus-4.6"}
```

Query available models: `GET /api/v1/agents/{name}/models`

## Auto-Clone Workspaces

When running **remotely** and `workspace_root` contains a repo path (e.g. `myorg/myproject`), AIntegriX auto-detects and clones it:

- If path exists on disk → use directly
- If path looks like a repo (`group/project`) → clone from your Git remote

**Local mode** (host = `127.0.0.1`): Only accepts local filesystem paths. No repo cloning. The MCP tool schema only exposes `workspace_root` without `branch` or `repo` params.

**Remote mode** (any other host): Accepts both local paths and repo paths. Schema includes `branch` param for clone control.

| Mode | workspace_root example | Behavior |
|------|----------------------|----------|
| Local | `/home/user/project` | Direct filesystem access |
| Remote | `myorg/myproject` | Auto-clone from your Git remote |
| Remote | `/opt/aintegrix` | Direct if exists on server |

## SSH Configuration (Server — Remote only)

The server needs SSH access to your Git server for repo cloning:

```
# ~/.ssh/config on your-server
Host git.example.com
    HostName YOUR_GIT_SERVER_IP
    User git
    IdentityFile ~/.ssh/id_rsa
    IdentitiesOnly yes
```

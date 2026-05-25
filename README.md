# AIntegriX

Centralized ACP (Agent Client Protocol) server for multi-agent coordination. Manages, routes, and orchestrates multiple AI coding agents through a single HTTP/MCP interface.

## Objectives

- Provide a single entry point to coordinate 5+ ACP-compatible coding agents
- Expose both REST API and native MCP server for universal integration
- Enable any MCP-compatible agent to delegate work to other agents
- Zero-UI, config-as-code, fast, lightweight

## Features

- JSON-RPC 2.0 ACP protocol over stdio (agent subprocess management)
- REST API with Bearer token authentication
- Native MCP server at `/mcp` (SSE transport)
- Real-time SSE streaming of agent responses
- Multi-agent orchestration (parallel, race, jury strategies)
- Agent chaining / pipelines (sequential multi-step workflows)
- Intelligent agent routing (YAML rules engine: keywords, file patterns, task type)
- Session fork (try same conversation with different agent)
- Git-aware sessions (auto-inject branch, commits, diff as context)
- Prompt rewriting per agent (configurable prefix/suffix)
- Context injection (auto-load steering files on session create)
- GitLab webhook triggers (MR → auto code review)
- Cost tracking (usage per agent/session/model)
- Live agent status (idle/busy with session counts)
- Declarative permission policy engine (YAML)
- Per-agent model selection (configurable defaults + dynamic override)
- Per-tenant rate limiting (token bucket)
- SQLite persistence (sessions, turns, agent state)
- Prometheus metrics at `/metrics`
- Graceful shutdown (SIGTERM/SIGINT)

## Agents Supported

| Agent | Command | Version | Default Model |
|-------|---------|---------|---------------|
| Kiro CLI | `kiro-cli acp` | 2.3.0 | claude-opus-4.6 |
| GitHub Copilot | `copilot --acp` | 1.0.48 | gpt-5.3-codex |
| OpenCode | `opencode acp` | 1.15.3 | xiaomi-mimo/mimo-v2.5-pro |
| Claude Code | `claude-agent-acp` | 0.37.0 | minimax-2.7 |
| Codex CLI | `codex-acp` | adapter | gpt-5.5-xhigh |

## Tech Stack

- Language: Rust (latest stable)
- Async runtime: tokio
- HTTP framework: axum
- Database: SQLite (sqlx, WAL mode)
- Config: YAML (serde_yaml)
- Protocol: JSON-RPC 2.0 over stdio

## Quick Start

```bash
# Build
cargo build --release

# Run with config
./target/release/aintegrix --config samples/aintegrix.yaml

# Or use the CLI
aintegrix-cli --url https://coord-acp.apulab.info status
```

## MCP Integration

Any MCP-compatible agent can use AIntegriX as a tool server:

**Remote (HTTPS):**
```json
{
  "mcpServers": {
    "aintegrix": {
      "url": "https://coord-acp.apulab.info/mcp",
      "type": "http",
      "headers": {
        "Authorization": "Bearer <token>"
      }
    }
  }
}
```

**Local (via mcp-proxy):**
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

Requires: `npm install -g mcp-proxy`

Available MCP tools: `acp_list_agents`, `acp_create_session`, `acp_prompt`, `acp_close_session`

### Auto-clone repos

Create a session with a fresh clone of any GitLab repo:

```json
{"name": "acp_create_session", "arguments": {"agent": "opencode", "repo": "ccvass/voxcix/admin", "branch": "develop"}}
```

The agent works on the latest code from the specified branch. Workspace is cleaned on session close.

### Important: Push before delegating

AIntegriX clones from the remote repository. The calling agent **must ensure code is pushed** before delegating work:

```
1. Agent works locally on code
2. Agent commits and pushes to remote branch
3. Agent calls acp_create_session(repo="...", branch="develop")
4. Remote agent analyzes the latest pushed code
```

If code is not pushed, the remote agent will see an outdated version.

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Liveness (no auth) |
| GET | `/api/v1/agents` | List agents |
| GET | `/api/v1/agents/{name}/models` | List models for agent |
| GET | `/api/v1/agents/status` | Live agent status (idle/busy/sessions) |
| GET | `/api/v1/usage` | Cost tracking (prompts per agent) |
| POST | `/api/v1/sessions` | Create session (supports `auto_route`, `git_context`) |
| GET | `/api/v1/sessions/{id}` | Get session info |
| POST | `/api/v1/sessions/{id}/prompt` | Send prompt (with prompt rewriting) |
| POST | `/api/v1/sessions/{id}/stream` | Send prompt with SSE streaming |
| POST | `/api/v1/sessions/{id}/fork` | Fork session to different agent |
| DELETE | `/api/v1/sessions/{id}` | Close session |
| POST | `/api/v1/orchestrate` | Multi-agent orchestration (parallel/race/jury) |
| POST | `/api/v1/pipelines` | Agent chaining (sequential steps) |
| POST | `/api/v1/stream` | Create session + stream in one call |
| POST | `/api/v1/webhooks/gitlab` | Receive GitLab webhooks (auto code review) |
| POST | `/mcp` | MCP JSON-RPC endpoint |

## Documentation

- [Development Standards](docs/STANDARDS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [API Reference](docs/API.md)
- [Configuration](docs/CONFIGURATION.md)
- [Installation](docs/INSTALL.md)

## Deployment

See `REMOTE.md` (not tracked — contains credentials).

## License

MIT

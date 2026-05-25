<p align="center">
  <img src="docs/logo.svg" alt="AIntegriX" width="400"/>
</p>

<p align="center">
  <strong>One API. Five AI agents. Infinite possibilities.</strong>
</p>

<p align="center">
  <a href="https://coord-acp.apulab.info/health">
    <img src="https://img.shields.io/badge/status-live-brightgreen" alt="Status"/>
  </a>
  <img src="https://img.shields.io/badge/agents-5-blue" alt="Agents"/>
  <img src="https://img.shields.io/badge/protocol-ACP-purple" alt="ACP"/>
  <img src="https://img.shields.io/badge/rust-stable-orange" alt="Rust"/>
</p>

---

## What is AIntegriX?

AIntegriX is a **centralized orchestrator** that lets any AI agent delegate work to other AI agents through a single MCP endpoint. Think of it as a **load balancer for AI coding agents**.

```
Your Agent (Kiro, Claude, etc.)
       │
       ▼ MCP
┌─────────────────────────────────┐
│         AIntegriX               │
│   Route • Orchestrate • Stream  │
└──┬──────┬──────┬──────┬──────┬──┘
   │      │      │      │      │
   ▼      ▼      ▼      ▼      ▼
 Kiro  Copilot OpenCode Claude Codex
```

**One prompt. Any agent. Real results.**

---

## Why AIntegriX?

| Problem | Solution |
|---------|----------|
| Each agent has different strengths | **Smart routing** picks the best agent for each task |
| Can't compare agent responses | **Orchestration** sends to N agents in parallel |
| No way to chain agent work | **Pipelines** feed output from one agent to the next |
| Agents can't read your latest code | **Auto-clone** from GitLab or direct local filesystem |
| Responses arrive all at once | **SSE streaming** shows chunks in real-time |
| Manual code review requests | **Webhooks** auto-trigger review on MR open |

---

## Workflow

```
┌─────────────────────────────────────────────────────────────┐
│                    YOUR DEVELOPMENT FLOW                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Code locally                                             │
│       │                                                      │
│  2. Ask AIntegriX:                                           │
│       "Review src/ for security issues"                      │
│       │                                                      │
│  3. AIntegriX routes to Claude (security expert)             │
│       │                                                      │
│  4. Claude reads your files, analyzes, responds              │
│       │                                                      │
│  5. You get the review in your agent's context               │
│                                                              │
│  ─── OR ───                                                  │
│                                                              │
│  Pipeline: OpenCode generates → Claude reviews → Kiro fixes  │
│                                                              │
│  ─── OR ───                                                  │
│                                                              │
│  Race: Send to 3 agents, first response wins                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Local (recommended for development)

```bash
# Install
git clone <your-repo-url>
cd aintegrix
cargo build --release
./configs/install.sh ~/.local/share/aintegrix

# Add to your agent's MCP config
{
  "mcpServers": {
    "aintegrix": {
      "command": "mcp-proxy",
      "args": ["-H", "Authorization", "Bearer aintegrix-local-key-2026",
               "--transport", "streamablehttp", "http://localhost:8050/mcp"],
      "env": {}
    }
  }
}
```

### Remote (for teams / CI)

```json
{
  "mcpServers": {
    "aintegrix": {
      "url": "https://coord-acp.apulab.info/mcp",
      "type": "http",
      "headers": {"Authorization": "Bearer <token>"}
    }
  }
}
```

---

## Agents

| Agent | Model | Best For |
|-------|-------|----------|
| 🧠 **Kiro** | claude-opus-4.6 | Rust, architecture, complex reasoning |
| 🤖 **Copilot** | gpt-5.3-codex | Frontend, React, quick edits |
| ⚡ **OpenCode** | mimo-v2.5-pro | Fast analysis, multi-file reads |
| 🔍 **Claude** | minimax-2.7 | Security review, deep analysis |
| 🛠️ **Codex** | gpt-5.5-xhigh | Python, refactoring, generation |

---

## Features

### Core
- **5 ACP agents** with real subprocess management
- **MCP server** — any agent can use AIntegriX as a tool
- **REST API** with Bearer auth
- **SSE streaming** of agent responses in real-time

### Orchestration
- **Parallel** — send to N agents, collect all responses
- **Race** — first response wins, cancel others
- **Jury** — N agents respond, a judge picks the best
- **Pipelines** — sequential multi-step workflows

### Intelligence
- **Auto-routing** — YAML rules pick the best agent by keywords/file patterns
- **Prompt rewriting** — per-agent prefix/suffix
- **Git-aware sessions** — inject branch, commits, diff as context
- **Context injection** — auto-load steering files

### Operations
- **Auto-clone repos** — fresh checkout from GitLab (remote mode)
- **Local filesystem** — direct access to your code (local mode)
- **Webhook triggers** — GitLab MR → auto code review
- **Session fork** — try same conversation with different agent
- **Cost tracking** — usage per agent/session/model
- **Live status** — agent idle/busy with session counts

---

## API

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/sessions` | Create session |
| POST | `/api/v1/sessions/{id}/prompt` | Send prompt |
| POST | `/api/v1/sessions/{id}/fork` | Fork to another agent |
| POST | `/api/v1/orchestrate` | Multi-agent (parallel/race/jury) |
| POST | `/api/v1/pipelines` | Sequential agent chaining |
| POST | `/api/v1/stream` | Create + stream SSE |
| POST | `/api/v1/webhooks/gitlab` | Receive webhook events |
| GET | `/api/v1/agents/status` | Live agent status |
| GET | `/api/v1/usage` | Cost tracking |
| POST | `/mcp` | MCP JSON-RPC endpoint |

---

## Documentation

- [Installation](docs/INSTALL.md)
- [Configuration](docs/CONFIGURATION.md)
- [API Reference](docs/API.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Development Standards](docs/STANDARDS.md)
- [Diagrams](docs/DIAGRAMS.md)

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Language | Rust (latest stable) |
| Runtime | tokio |
| HTTP | axum |
| Protocol | ACP (JSON-RPC 2.0 over stdio) |
| Database | SQLite (sqlx) |
| Config | YAML |

---

## License

Apache-2.0

---

<p align="center">
  <sub>Built with 🦀 Rust • Powered by ACP</sub>
</p>

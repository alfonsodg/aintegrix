# Architecture

## Overview

AIntegriX is a centralized ACP server that acts as a multiplexer/orchestrator for AI coding agents. It spawns agents as subprocesses, communicates via JSON-RPC 2.0 over stdio, and exposes their capabilities through REST API and MCP.

## Components

```
Clients (REST/MCP/WS)
        |
        v HTTPS (nginx reverse proxy)
+------------------+
|   AIntegriX      |  Port 8050
|                  |
|  +------------+  |
|  | HTTP Router |  |  axum + tower middleware
|  +-----+------+  |
|        |         |
|  +-----v------+  |
|  | Session Mgr|  |  dashmap, concurrency limits
|  +-----+------+  |
|        |         |
|  +-----v------+  |
|  | Agent Pool |  |  spawn, health check, auto-restart
|  +-----+------+  |
|        |         |
+--------+---------+
         | JSON-RPC 2.0 (stdin/stdout)
   +-----+-----+-----+-----+
   |     |     |     |     |
 kiro copilot opencode claude codex
```

## Data Flow

1. Client sends HTTP request (REST or MCP JSON-RPC)
2. Auth middleware validates Bearer token
3. Router dispatches to appropriate handler
4. Session manager creates/retrieves session
5. Agent pool spawns or reuses agent subprocess
6. AIntegriX sends ACP JSON-RPC to agent via stdin
7. Agent processes and streams session/update notifications via stdout
8. Updates forwarded to client via WebSocket/SSE
9. Final response returned to client
10. Session state persisted to SQLite

## Technology Stack

| Layer | Technology |
|-------|------------|
| Runtime | Rust + tokio |
| HTTP | axum + tower |
| Protocol | JSON-RPC 2.0 over stdio |
| Database | SQLite (sqlx, WAL mode) |
| Cache | dashmap (in-memory) |
| Config | YAML (serde_yaml) |
| Auth | Bearer token |
| Metrics | Prometheus text format |
| Logging | tracing (JSON structured) |

## External Integrations

- 5 ACP agents (Kiro, Copilot, OpenCode, Claude, Codex)
- MCP clients (any agent with MCP support)
- Nginx reverse proxy with Let's Encrypt SSL
- Prometheus scraping (optional)
- Webhook targets (HTTP POST with HMAC)

## Security Considerations

- Authentication: Bearer token via AINTEGRIX_API_KEY env var
- Authorization: Permission policy engine (auto_approve/require_approval/always_deny)
- Workspace sandboxing: Path traversal prevention on fs operations
- TLS: Let's Encrypt via certbot (nginx)
- No secrets in config files (env var interpolation)

## Scalability

- Single-node design (SQLite + in-memory cache)
- Horizontal: not needed for current use case (5 agents, <20 concurrent sessions)
- Bottleneck: agent subprocess count (configurable max_sessions per agent)
- Future: migrate SQLite to PostgreSQL + add Redis if multi-node needed

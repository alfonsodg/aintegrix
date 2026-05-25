# API Reference

Base URL: `https://coord-acp.apulab.info`

All `/api/*` and `/mcp` endpoints require: `Authorization: Bearer <token>`

## Health

### GET /health
Liveness check. No auth required.

**Response**: `200 OK` (empty body)

### GET /readiness
Readiness check. Returns 503 if no agents configured.

## Agents

### GET /api/v1/agents
List all registered agents.

**Response**:
```json
[
  {"name": "kiro", "command": "kiro-cli", "mode": "native", "max_sessions": 3},
  {"name": "claude", "command": "claude-agent-acp", "mode": "adapter", "max_sessions": 3}
]
```

### GET /api/v1/agents/{name}/models
List available models for an agent.

**Response**:
```json
{
  "agent": "kiro",
  "default_model": "claude-opus-4.6",
  "models": ["claude-opus-4.6", "claude-sonnet-4", "claude-haiku-3.5"]
}
```

## Sessions

### POST /api/v1/sessions
Create a new session.

**Request**:
```json
{"agent": "kiro", "workspace_root": "/tmp/project", "model": "claude-opus-4.6"}
```

**With auto-clone** (clones repo fresh into temp workspace):
```json
{"agent": "opencode", "repo": "ccvass/voxcix/admin", "branch": "develop"}
```

**With auto-routing**:
```json
{"auto_route": true, "prompt": "fix the React component", "repo": "ccvass/voxcix/admin", "branch": "develop"}
```

> **Note**: When using `repo`, ensure code is pushed to the remote branch first. AIntegriX clones from the remote — unpushed local changes will not be visible to the agent.

**Response** (201):
```json
{"id": "uuid", "agent": "kiro", "status": "active"}
```

### POST /api/v1/sessions/{id}/prompt
Send a prompt to a session.

**Request**:
```json
{"messages": [{"role": "user", "content": [{"type": "text", "text": "fix the bug"}]}]}
```

**Response**:
```json
{"stop_reason": "end_turn"}
```

### GET /api/v1/sessions/{id}/stream
WebSocket upgrade for real-time session updates.

**Messages received**:
```json
{"type": "session_update", "data": {"type": "agent_message_chunk", "text": "..."}}
{"type": "turn_complete", "data": {"stop_reason": "end_turn"}}
```

### DELETE /api/v1/sessions/{id}
Close a session.

## MCP

### POST /mcp
MCP JSON-RPC endpoint (Streamable HTTP transport).

**Initialize**:
```json
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
```

**List tools**:
```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
```

**Call tool**:
```json
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "acp_list_agents", "arguments": {}}}
```

### MCP Tools

| Tool | Description | Required Params |
|------|-------------|-----------------|
| `acp_list_agents` | List available agents | — |
| `acp_create_session` | Create session | `agent`, optional: `model`, `workspace_root` |
| `acp_prompt` | Send prompt | `session_id`, `message` |
| `acp_close_session` | Close session | `session_id` |

## Orchestration

### POST /api/v1/orchestrate
Send prompt to multiple agents.

**Request**:
```json
{
  "agents": ["kiro", "claude", "opencode"],
  "messages": [{"type": "text", "text": "implement binary search"}],
  "strategy": "parallel",
  "judge": "claude"
}
```

Strategies: `parallel` (all respond), `race` (first wins), `jury` (judge picks best).

## Pipelines

### POST /api/v1/pipelines
Sequential multi-step agent chaining.

**Request**:
```json
{
  "steps": [{"agent": "opencode"}, {"agent": "claude", "prompt_template": "Review: {{previous}}"}],
  "messages": [{"type": "text", "text": "implement the feature"}]
}
```

## Streaming

### POST /api/v1/stream
Create session and stream response via SSE.

### POST /api/v1/sessions/{id}/stream
Stream prompt response on existing session via SSE.

## Session Fork

### POST /api/v1/sessions/{id}/fork
Fork a session to a different agent.

**Request**: `{"target_agent": "claude"}`

## Agent Status

### GET /api/v1/agents/status
Returns current status of all agents (idle/busy, active sessions, max sessions).

## Usage Tracking

### GET /api/v1/usage
Query usage data. Optional params: `?agent=kiro&since=2026-05-01`

## Webhooks

### POST /api/v1/webhooks/gitlab
Receive GitLab webhook events. Validates `X-Gitlab-Token` header against `AINTEGRIX_WEBHOOK_SECRET` env var. MR open/reopen triggers auto code review.

## Metrics

### GET /metrics
Prometheus text format. No auth required.

```
aintegrix_http_requests_total 42
aintegrix_sessions_created_total 5
aintegrix_prompts_total 12
aintegrix_agent_restarts_total 0
```

## Errors

All errors follow:
```json
{"error": {"code": "agent_not_found", "message": "agent 'xyz' not configured"}}
```

HTTP status codes: 401 (unauthorized), 404 (not found), 429 (rate limited), 500 (internal).

# ACP Protocol Reference

## Specification

- Official site: https://agentclientprotocol.com
- GitHub: https://github.com/agentclientprotocol/agent-client-protocol
- TypeScript SDK: https://github.com/agentclientprotocol/typescript-sdk

## Key Methods

| Method | Direction | Description |
|--------|-----------|-------------|
| `initialize` | Client → Agent | Exchange capabilities |
| `session/new` | Client → Agent | Create session (params: `cwd`, `mcpServers`) |
| `session/prompt` | Client → Agent | Send prompt (params: `sessionId`, `prompt`) |
| `session/cancel` | Client → Agent | Cancel current operation |
| `session/request_permission` | Agent → Client | Request tool approval |
| `fs/read_text_file` | Agent → Client | Read file from client filesystem |
| `fs/write_text_file` | Agent → Client | Write file to client filesystem |
| `session/update` (notification) | Agent → Client | Streaming updates (AgentMessageChunk, ToolCall, TurnEnd) |

## Permission Response Format

```json
{"jsonrpc": "2.0", "id": 5, "result": {"outcome": {"outcome": "selected", "optionId": "allow-once"}}}
```

## Session/new Params

```json
{"cwd": "/path/to/workspace", "mcpServers": []}
```

## Agent Adapters

- Kiro: `kiro-cli acp` (native)
- Copilot: `copilot --acp` (native)
- OpenCode: `opencode acp` (native)
- Claude: `@agentclientprotocol/claude-agent-acp` (npm adapter)
- Codex: `@agentclientprotocol/codex-acp` (npm adapter)

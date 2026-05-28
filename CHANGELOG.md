# Changelog

## [v0.2.0] - 2026-05-26

### Added
- ACP capability negotiation (session/set_model, embeddedContext, mcpServers, fork, load/resume)
- All features capability-gated — only active if agent advertises support

## [v0.1.0] - 2026-05-25

### Added
- Multi-agent ACP orchestrator server (Kiro, Copilot, OpenCode, Claude, Codex)
- REST API: sessions, prompts, agents, models, fork, close
- MCP server: acp_list_agents, acp_create_session, acp_prompt, acp_close_session
- Multi-agent orchestration (parallel, race, jury strategies)
- Agent pipelines (sequential chaining)
- SSE streaming of agent responses
- Intelligent routing (YAML rules: keywords, file patterns, task type)
- Auto-clone repos (remote mode) / direct filesystem (local mode)
- Tool call handling: session/request_permission auto-approve
- Webhook triggers for auto code review
- Session fork, cost tracking, agent status
- Prompt rewriting, git-aware sessions, context injection
- Dynamic MCP tool schema (local vs remote detection)
- 75 Rust tests + 28 E2E bash + 16 E2E Python
- Apache-2.0 license, published to GitHub

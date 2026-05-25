# Development Standards

## Language

- Rust (latest stable via rust-toolchain.toml)
- Edition: 2024

## Tooling

- Build: `cargo build`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt`
- Test: `cargo test`
- Audit: `cargo audit`

## Code Style

- Max file size: 300 lines
- `#[allow(dead_code)]` only for scaffold modules pending integration
- All public functions documented
- Error handling via `thiserror` (AppError enum)
- Async by default for I/O operations

## Dependencies

- axum 0.8 (HTTP + WebSocket)
- tokio 1 (async runtime)
- sqlx 0.8 (SQLite, compile-time checked)
- serde + serde_yaml (config)
- serde_json (JSON-RPC)
- dashmap 6 (concurrent cache)
- tracing (structured logging)
- clap 4 (CLI)
- uuid 1 (session IDs)
- chrono 0.4 (timestamps)
- reqwest 0.12 (webhooks)
- hmac + sha2 (webhook signatures)

## Testing

- Unit tests: inline `#[cfg(test)]` modules
- Integration tests: `tests/` directory with mock ACP agent
- Mock agent: `src/bin/mock_agent.rs`
- CI: clippy + test + audit

## Git

- Branch from `develop`
- Conventional commits: `type(scope): subject (#issue)`
- Merge to develop directly (no MR for features)
- MR only for develop → main

## Config

- YAML format (`aintegrix.yaml`)
- Env var interpolation: `${VAR_NAME}`
- No hardcoded values

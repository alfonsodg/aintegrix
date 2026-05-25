# Development Standards

## Language & Version

- **Language**: [Python/TypeScript/Go/Rust/Java] (latest stable)
- **Package Manager**: [uv/npm/go mod/cargo/maven]

## Code Style

- Linter: [ruff/eslint/golangci-lint/clippy/checkstyle]
- Formatter: [ruff format/prettier/gofmt/rustfmt]
- Line length: 120
- Indentation: spaces (4 for Python/Java, 2 for TS/Go/Rust)

## Architecture

- Pattern: [Clean Architecture / Hexagonal / MVC]
- API style: [REST / GraphQL / gRPC]
- Database: [PostgreSQL / Redis / MongoDB]

## Testing

- Framework: [pytest/vitest/go test/cargo test]
- Coverage target: 80%
- Test location: `tests/`

## Git Workflow

- Branches: `feature/*` → `develop` → `main`
- Commits: `type(scope): description (#issue)`
- MR required for `main` (from `develop` only)

## Deployment

- Container: Docker (multi-stage)
- Registry: registry.labtau.com
- CI/CD: GitLab CI (auto-detected)

## Documentation

- `README.md` — project overview
- `docs/STANDARDS.md` — this file
- `CHANGELOG.md` — auto-generated from commits

# Configuration Guide

## Environment Variables

All configuration is via environment variables. See `samples/.env.example` for defaults.

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | Database connection string | `postgresql://user:pass@host:5432/db` |
| `SECRET_KEY` | Application secret | (generate random) |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `8000` |
| `LOG_LEVEL` | Logging verbosity | `INFO` |
| `WORKERS` | Number of workers | `4` |

## Configuration Files

| File | Purpose |
|------|---------|
| `.env` | Local environment (gitignored) |
| `samples/.env.example` | Template with all variables |

## Per-Environment Settings

### Development

- Debug mode enabled
- Hot reload active
- Local database

### Staging

- Debug disabled
- Connected to staging services
- Deployed via CI/CD

### Production

- Debug disabled
- All secrets from CI/CD variables (masked + protected)
- Health check required before traffic

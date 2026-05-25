# Installation Guide

## Prerequisites

- (Language runtime and version)
- (Package manager)
- Docker (for local services)

## Clone

```bash
git clone git@scovil.labtau.com:<group>/<project>.git
cd <project>
```

## Environment Setup

```bash
cp samples/.env.example .env
# Edit .env with your local values
```

## Install Dependencies

```bash
# (language-specific install command)
```

## Database Setup

```bash
# (migration command if applicable)
```

## Run Locally

```bash
# (start command)
```

## Verify

```bash
curl http://localhost:<port>/health
```

## Common Issues

| Problem | Solution |
|---------|----------|
| Port in use | Change port in .env |
| DB connection refused | Ensure Docker services are running |

# API Documentation

## Base URL

- Development: `http://localhost:<port>`
- Staging: `https://api-dev.<domain>`
- Production: `https://api.<domain>`

## Authentication

All endpoints require `Authorization: Bearer <token>` unless noted.

## Endpoints

### Health

```
GET /health
```

Response: `200 OK`

```json
{"status": "healthy", "version": "1.0.0"}
```

### Example Resource

```
GET /api/v1/resource
```

Query parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| page | int | No | Page number (default: 1) |
| limit | int | No | Items per page (default: 20) |

Response: `200 OK`

```json
{
  "data": [],
  "meta": {"page": 1, "limit": 20, "total": 0}
}
```

## Error Responses

| Code | Description |
|------|-------------|
| 400 | Bad Request — invalid parameters |
| 401 | Unauthorized — missing or invalid token |
| 403 | Forbidden — insufficient permissions |
| 404 | Not Found — resource does not exist |
| 500 | Internal Server Error |

Error body:

```json
{"error": "description", "code": "ERROR_CODE"}
```

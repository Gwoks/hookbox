# 🪝 WebhookCatch

*"Catch every webhook. Debug with confidence."*

Self-hosted webhook testing service for developers.

## Features

- ✅ Unique webhook endpoints (`/hook/<id>`)
- ✅ Capture method, headers, body, query params
- ✅ Web dashboard to view requests
- ✅ Mock response configuration
- ✅ Auto-delete expired data
- ✅ GitHub auto-deploy ready

## Quick Start

```bash
# Install dependencies
pip install -r requirements.txt

# Run server
python -m app.main

# Open in browser
http://localhost:5000
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Python 3.14 + FastAPI |
| Database | SQLite |
| Frontend | HTML + Tailwind CSS |
| Server | Uvicorn (ASGI) |

## API Endpoints

| Method | URL | Description |
|--------|-----|-------------|
| POST | `/api/endpoints` | Create endpoint |
| GET | `/api/endpoints` | List endpoints |
| GET | `/api/endpoints/<id>` | Get endpoint |
| DELETE | `/api/endpoints/<id>` | Delete endpoint |
| GET | `/api/endpoints/<id>/requests` | List requests |
| GET | `/api/requests/<id>` | Get request detail |
| PUT | `/api/endpoints/<id>/mock` | Set mock response |
| GET | `/api/endpoints/<id>/mock` | Get mock response |
| DELETE | `/api/endpoints/<id>/mock` | Delete mock |

## Webhook URL

Any request to `/hook/<endpoint_id>` will be captured and return the configured mock response (if set).

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GITHUB_WEBHOOK_SECRET` | Secret for GitHub webhook verification | (none) |

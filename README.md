# 📦 HookBox

*"Catch every webhook. Debug with confidence."*

Self-hosted webhook testing service with multi-user support.

## Features

- ✅ **Multi-user** - Email-based accounts, each user isolated
- ✅ **Capture webhooks** - Method, headers, body, query params
- ✅ **Web dashboard** - View and manage all your endpoints
- ✅ **Mock responses** - Configure custom responses for testing
- ✅ **Auto-reset** - Database clears daily at midnight
- ✅ **GitHub auto-deploy** - Webhook receiver for CI/CD

## Requirements

- Python 3.14+ (with pip)
- Docker & Docker Compose (optional)
- Linux server (for cronjobs)

## Installation

### Option 1: Docker (Recommended)

```bash
# 1. Clone the repository
git clone https://github.com/Gwoks/hookbox.git
cd hookbox

# 2. Run with Docker Compose
docker-compose up -d

# 3. Open in browser
http://localhost:5000
```

### Option 2: Manual

```bash
# 1. Clone the repository
git clone https://github.com/Gwoks/hookbox.git
cd hookbox

# 2. Install dependencies
pip install -r requirements.txt

# 3. Run the server
python -m app.main

# 4. Open in browser
http://localhost:5000
```

## First-time Setup

1. Visit `http://localhost:5000/register`
2. Enter your email to create an account
3. Create your first webhook endpoint
4. Share your unique webhook URL

## Webhook URL Format

```
http://your-server:5000/hook/{user_id}/{endpoint_id}
```

Example:
```
http://localhost:5000/hook/abc123def456/xyZ8jK2d
```

## API Documentation

Visit `/docs` for Swagger API documentation.

## API Authentication

All API endpoints (except `/api/register` and `/api/login`) require headers:
- `X-User-ID`: Your user ID (from registration)
- `X-Email`: Your email address

### Register
```bash
curl -X POST http://localhost:5000/api/register \
  -H "Content-Type: application/json" \
  -d '{"email": "you@example.com"}'
```

### Create Endpoint
```bash
curl -X POST http://localhost:5000/api/endpoints \
  -H "Content-Type: application/json" \
  -H "X-User-ID: your-user-id" \
  -H "X-Email: you@example.com"
```

### Send Webhook
```bash
curl -X POST http://localhost:5000/hook/user-id/endpoint-id \
  -H "Content-Type: application/json" \
  -d '{"event": "test"}'
```

## Configuration

Edit `config.py` to customize:

```python
HOST = "0.0.0.0"      # Server host
PORT = 5000           # Server port
ENDPOINT_ID_LENGTH = 8  # Random ID length
DEFAULT_EXPIRY_HOURS = 24  # Endpoint expiry
AUTO_DELETE_HOURS = 168    # Auto-cleanup (7 days)
```

## Production Deployment

### Systemd Service

Create `/etc/systemd/system/hookbox.service`:
```ini
[Unit]
Description=HookBox Webhook Service
After=network.target

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/home/ubuntu/hookbox
ExecStart=/home/linuxbrew/.linuxbrew/bin/python3.14 -m app.main
Restart=always

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable hookbox
sudo systemctl start hookbox
```

### Cronjob (Auto-reset at midnight)

```bash
# Add to crontab
crontab -e

# Add this line:
0 0 * * * /home/ubuntu/hookbox/reset_db.sh >> /home/ubuntu/hookbox/cron_reset.log 2>&1
```

### Docker Management

```bash
# View logs
docker-compose logs -f

# Restart
docker-compose restart

# Stop
docker-compose down

# Update and restart
git pull origin main && docker-compose up -d --force-recreate
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Python 3.14 + FastAPI |
| Database | SQLite |
| Frontend | HTML + Tailwind CSS |
| Server | Uvicorn (ASGI) |

## Project Structure

```
hookbox/
├── app/
│   ├── __init__.py
│   ├── main.py          # FastAPI app
│   ├── database.py      # SQLite setup
│   ├── models.py        # Pydantic models
│   └── routes/
│       ├── api.py       # JSON API
│       └── webhook.py   # GitHub webhook
├── templates/
│   ├── base.html
│   ├── index.html
│   ├── dashboard.html
│   ├── mock.html
│   ├── login.html
│   └── register.html
├── data/                # SQLite database
├── config.py
├── requirements.txt
├── reset_db.sh          # Daily reset script
└── README.md
```

## License

MIT

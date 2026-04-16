# HookBox Features & User Journey

## Overview
**HookBox** is a self-hosted webhook testing service that allows users to capture, inspect, and mock HTTP requests.

**URL:** http://43.156.182.81:5000
**GitHub:** https://github.com/Gwoks/hookbox

---

## Features

### 1. Email-Only Authentication
- **Flow:** User enters email → Auto-registered if new → Auto-logged in
- **No passwords required**
- **Session stored in localStorage**
- **File:** `templates/login.html`, `app/routes/api.py`

### 2. Multiple Endpoints Per Account
- One account can create unlimited endpoints
- Each endpoint gets unique webhook URL: `/hook/{user_id}/{endpoint_id}`
- **File:** `app/routes/api.py` (endpoints CRUD)

### 3. Webhook Capture
- Capture any HTTP method: GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD
- Store: headers, query params, body, content-type, timestamp
- **File:** `app/main.py` (receive_webhook route)

### 4. Real-Time Dashboard Updates (WebSocket)
- Dashboard auto-updates when new requests arrive
- Connection status indicator (Live/Disconnected)
- Auto-reconnect on disconnect
- **File:** `app/websocket.py`, `templates/dashboard.html`

### 5. Method-Specific Mock Responses
- Configure different responses per HTTP method
- Per-method: status code, headers, body, content-type, delay
- DEFAULT fallback for unmatched methods
- Priority: Specific method → DEFAULT
- **File:** `app/routes/api.py` (mock rules), `templates/mock.html`

### 6. Backup & Restore
- Export all data as JSON
- Import to restore
- **File:** `app/routes/backup.py`, `templates/backup.html`

### 7. Auto-Reset at Midnight
- Database cleared at midnight (configurable)
- Cron job: `reset_db.sh`
- **File:** `reset_db.sh`, `app/utils/cleanup.py`

### 8. GitHub Auto-Deploy
- Push to GitHub → Auto-pull and restart
- **File:** `app/routes/webhook.py`

### 9. Docker Support
- Dockerfile + docker-compose.yml
- **File:** `Dockerfile`, `docker-compose.yml`

---

## User Journey

### 1. First Visit
1. User visits `/login`
2. Enters email address
3. Auto-registered and redirected to `/` (endpoints list)
4. Sees "No endpoints yet" message

### 2. Create First Endpoint
1. Clicks "+ Create Endpoint" button
2. Optionally enters endpoint name
3. Redirected to `/d/{endpoint_id}` (dashboard)
4. Copy webhook URL

### 3. Send Test Request
1. User configures their app to send webhooks to copied URL
2. Sends a test request (e.g., POST with JSON body)
3. Dashboard shows "Live" indicator (WebSocket connected)
4. Request appears in list automatically (no refresh)

### 4. View Request Details
1. Clicks on any request row
2. Modal shows: headers, query params, body
3. Can pretty-print JSON body

### 5. Configure Mock Response
1. Clicks "Mock" button
2. Selects HTTP method (GET, POST, etc.)
3. Configures: status code, headers, body, delay
4. Enables the mock
5. Sends test request to see mock response

### 6. Create More Endpoints
1. Clicks "Endpoints" to go back to list
2. Creates new endpoint
3. Each endpoint has independent requests and mock configs

### 7. Backup Data
1. Visits `/backup`
2. Downloads JSON export
3. Can import later to restore

---

## Technical Details

### Database Schema
```sql
users: id, email, created_at, last_login
endpoints: id, user_id, name, created_at, last_hit, request_count, expires_at, is_active
requests: id, endpoint_id, method, path, headers, query_params, body, content_type, timestamp
mock_rules: id, endpoint_id, method, status_code, response_body, response_headers, content_type, enabled, delay_ms
```

### API Endpoints
- `POST /api/login` - Login/register with email
- `GET /api/endpoints` - List user's endpoints
- `POST /api/endpoints` - Create endpoint
- `GET /api/endpoints/{id}` - Get endpoint details
- `DELETE /api/endpoints/{id}` - Delete endpoint
- `GET /api/endpoints/{id}/requests` - List captured requests
- `GET /api/requests/{id}` - Get request details
- `PUT /api/endpoints/{id}/mock` - Set mock rule
- `GET /api/endpoints/{id}/mock` - Get mock rules

### WebSocket
- Route: `/ws/{endpoint_id}`
- Message types: `new_request`

### Environment Variables
- `DATABASE_URL` - SQLite database path
- `GITHUB_WEBHOOK_SECRET` - For GitHub auto-deploy

---

## Ports

| Service | Port | URL |
|---------|------|-----|
| HookBox | 5000 | http://43.156.182.81:5000 |
| Crypto Status | 5002 | http://43.156.182.81:5002 |

---

## Known Issues / TODOs
- [ ] WebSocket disconnect doesn't auto-reconnect on dashboard refresh needed
- [ ] Mock response delay not working in some cases
- [ ] Delete mock rule should be method-specific, not all

# HookBox Features & User Journey

## Overview
HookBox is a self-hosted webhook testing service that allows developers to capture, inspect, and mock HTTP webhooks.

**Live URL:** http://43.156.182.81:5000  
**GitHub:** https://github.com/Gwoks/hookbox  
**Tech Stack:** Python 3.14 + FastAPI + SQLite + Tailwind CSS

---

## Features

### 1. User Authentication
- **Email-based registration and login**
- Auto-registration on login if user doesn't exist
- Session stored in localStorage (token-based)
- User isolation: each user sees only their own data
- User ID is hashed from email (16 char hex)

**User Journey:**
1. Visit `/login` or `/register`
2. Enter email
3. Account created automatically if new
4. Redirected to homepage

**Files:** `app/routes/api.py` (register, login), `templates/login.html`, `templates/register.html`

### 2. Webhook Endpoints
- Create unique webhook endpoints
- Each endpoint gets a unique ID (8 chars)
- Webhook URL format: `/hook/{user_id}/{endpoint_id}`
- Capture any HTTP method (GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD)
- Store: method, headers, body, query params, timestamp

**User Journey:**
1. Go to homepage
2. Click "+ New Endpoint"
3. Enter optional name
4. Get unique webhook URL to share

**Files:** `app/routes/api.py` (endpoints CRUD), `app/main.py` (webhook receiver)

### 3. Dashboard with Real-time Updates
- View captured requests for an endpoint
- **WebSocket support** for live updates
- Auto-reconnect on disconnect
- Request count, mock status, creation time

**User Journey:**
1. Click "View" on an endpoint
2. Dashboard shows all captured requests
3. Requests appear instantly when webhooks arrive (no refresh needed)

**Files:** `app/main.py` (WebSocket), `app/websocket.py`, `templates/dashboard.html`

### 4. Mock Responses
- **Method-specific mock responses** (GET, POST, PUT, DELETE, etc.)
- DEFAULT fallback for any method
- Configure: status code, headers, body, delay
- Enable/disable per method

**Priority:** Specific method > DEFAULT

**User Journey:**
1. Click "Mock" on an endpoint
2. Select HTTP method (or DEFAULT)
3. Configure response
4. Save
5. Webhook returns configured response

**Files:** `app/routes/api.py` (mock rules), `app/main.py` (webhook receiver), `templates/mock.html`

### 5. Backup & Restore
- Export all data as JSON
- Download backup file
- Restore from backup file

**User Journey:**
1. Click "Backup" in nav
2. Click "Download Backup"
3. JSON file downloaded
4. To restore: select file and click "Restore"

**Files:** `app/routes/backup.py`, `templates/backup.html`

### 6. Auto-Reset at Midnight
- Database clears daily at midnight
- Clears all endpoints, requests, mock rules
- Service restarts fresh

**Cron:** `0 0 * * * /home/ubuntu/hookbox/reset_db.sh`

---

## User Journey (Complete)

### First Time User
1. Visit `/register`
2. Enter email → Account auto-created
3. Click "+ New Endpoint"
4. Copy webhook URL
5. Share URL with external service
6. View incoming requests in dashboard
7. Configure mock responses

### Returning User
1. Visit `/login`
2. Enter email → Auto-login
3. See existing endpoints
4. Continue working

### Testing Webhook
1. Create endpoint
2. Configure mock (optional)
3. Send test request: `curl -X POST http://server/hook/user_id/endpoint_id -d '{"test": true}'`
4. See request appear in dashboard instantly
5. Mock returns configured response

---

## API Endpoints

| Method | URL | Description |
|--------|-----|-------------|
| POST | `/api/register` | Register user |
| POST | `/api/login` | Login (auto-register) |
| POST | `/api/endpoints` | Create endpoint |
| GET | `/api/endpoints` | List user's endpoints |
| GET | `/api/endpoints/{id}` | Get endpoint details |
| DELETE | `/api/endpoints/{id}` | Delete endpoint |
| GET | `/api/endpoints/{id}/requests` | List captured requests |
| GET | `/api/requests/{id}` | Get request details |
| PUT | `/api/endpoints/{id}/mock` | Set mock rule |
| GET | `/api/endpoints/{id}/mock` | Get mock rules |
| DELETE | `/api/endpoints/{id}/mock` | Delete mock rules |
| GET | `/api/backup/export` | Export all data as JSON |
| POST | `/api/backup/restore` | Restore from JSON |
| GET | `/ws/{endpoint_id}` | WebSocket for live updates |

---

## Database Schema

### users
- id (TEXT PRIMARY KEY)
- email (TEXT UNIQUE)
- created_at
- last_login

### endpoints
- id (TEXT PRIMARY KEY)
- user_id (TEXT FK)
- name
- created_at
- last_hit
- request_count
- expires_at
- is_active

### requests
- id (INTEGER PRIMARY KEY)
- endpoint_id (TEXT FK)
- method
- path
- headers
- query_params
- body
- content_type
- timestamp

### mock_rules
- id (INTEGER PRIMARY KEY)
- endpoint_id (TEXT FK)
- method (DEFAULT, GET, POST, etc.)
- status_code
- response_body
- response_headers
- content_type
- enabled
- delay_ms

---

## Configuration

**File:** `config.py`
- HOST = "0.0.0.0"
- PORT = 5000
- ENDPOINT_ID_LENGTH = 8
- DEFAULT_EXPIRY_HOURS = 24
- AUTO_DELETE_HOURS = 168

---

## Docker Support

```bash
docker-compose up -d
```

**Files:** `Dockerfile`, `docker-compose.yml`

---

## Testing Checklist

1. **Registration/Login**
   - [ ] New user can register
   - [ ] Existing user can login
   - [ ] Login auto-creates user if not found

2. **Endpoints**
   - [ ] Create endpoint
   - [ ] List endpoints
   - [ ] Delete endpoint
   - [ ] Endpoint ID is unique

3. **Webhook Capture**
   - [ ] Send GET request → captured
   - [ ] Send POST request → captured with body
   - [ ] Headers stored correctly
   - [ ] Query params stored correctly

4. **Dashboard**
   - [ ] Shows requests list
   - [ ] WebSocket updates live
   - [ ] Click request shows detail

5. **Mock Responses**
   - [ ] Configure GET mock
   - [ ] Configure POST mock
   - [ ] DEFAULT fallback works
   - [ ] Method priority works
   - [ ] Delay works
   - [ ] Custom headers work

6. **Backup**
   - [ ] Export downloads JSON
   - [ ] Restore merges data

---

## Common Issues

1. **Research data stale** - Ensure cron runs `research_daily.py` not `research_6h.py`
2. **Timezone mismatch** - Set server timezone to Asia/Jakarta (+7)
3. **Port in use** - Kill existing process with `fuser -k 5000/tcp`

---

## GitHub Integration

**PAT:** 

**Repos:**
- OpenClaw: https://github.com/Gwoks/openclaw
- HookBox: https://github.com/Gwoks/hookbox

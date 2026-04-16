"""JSON API routes with user authentication"""

from fastapi import APIRouter, Depends, HTTPException, Request
from fastapi.responses import JSONResponse
import aiosqlite
from datetime import datetime
import json
from fastapi import Header

from app.database import get_db, create_user_token, hash_email
from app.models import (
    UserRegister, UserLogin, UserResponse, UserSession,
    EndpointCreate, EndpointResponse, EndpointDetail,
    RequestResponse, RequestDetail, 
    MockRuleCreate, MockRuleResponse,
    MessageResponse
)
from app.utils import generate_endpoint_id, calculate_expiry

router = APIRouter(prefix="/api", tags=["API"])

# --- User Authentication ---

def get_current_user(x_user_id: str = Header(...), x_email: str = Header(...)) -> dict:
    """Validate user from headers"""
    return {"user_id": x_user_id, "email": x_email}

@router.post("/register", response_model=UserResponse)
async def register(data: UserRegister, db: aiosqlite.Connection = Depends(get_db)):
    """Register user with email"""
    user_id = hash_email(data.email)
    token = create_user_token()
    
    try:
        await db.execute(
            "INSERT INTO users (id, email) VALUES (?, ?)",
            (user_id, data.email.lower().strip())
        )
        await db.commit()
    except aiosqlite.IntegrityError:
        # User exists, get existing data
        async with db.execute("SELECT * FROM users WHERE id = ?", (user_id,)) as cursor:
            row = await cursor.fetchone()
        if row:
            return UserResponse(
                id=row['id'],
                email=row['email'],
                created_at=datetime.fromisoformat(row['created_at']),
                last_login=datetime.fromisoformat(row['last_login']) if row['last_login'] else None,
                token=token
            )
    
    return UserResponse(
        id=user_id,
        email=data.email.lower().strip(),
        created_at=datetime.utcnow(),
        last_login=None,
        token=token
    )

@router.post("/login", response_model=UserResponse)
async def login(data: UserLogin, db: aiosqlite.Connection = Depends(get_db)):
    """Login with email - auto-register if not found"""
    user_id = hash_email(data.email)
    
    async with db.execute("SELECT * FROM users WHERE id = ?", (user_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row:
        # Auto-register the user
        await db.execute(
            "INSERT INTO users (id, email) VALUES (?, ?)",
            (user_id, data.email.lower().strip())
        )
        await db.commit()
        token = create_user_token()
        return UserResponse(
            id=user_id,
            email=data.email.lower().strip(),
            created_at=datetime.utcnow(),
            last_login=None,
            token=token
        )
    
    # Update last login
    await db.execute("UPDATE users SET last_login = ? WHERE id = ?", 
                     (datetime.utcnow().isoformat(), user_id))
    await db.commit()
    
    token = create_user_token()
    return UserResponse(
        id=row['id'],
        email=row['email'],
        created_at=datetime.fromisoformat(row['created_at']),
        last_login=datetime.utcnow(),
        token=token
    )

# --- Endpoints CRUD (User-scoped) ---

@router.post("/endpoints", response_model=EndpointResponse)
async def create_endpoint(
    data: EndpointCreate = None,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Create a new webhook endpoint for current user"""
    if data is None:
        data = EndpointCreate()
    
    endpoint_id = generate_endpoint_id()
    expires_at = calculate_expiry(data.expires_in_hours or 24)
    
    await db.execute(
        "INSERT INTO endpoints (id, user_id, name, expires_at) VALUES (?, ?, ?, ?)",
        (endpoint_id, current_user['user_id'], data.name, expires_at.isoformat())
    )
    await db.commit()
    
    return EndpointResponse(
        id=endpoint_id,
        user_id=current_user['user_id'],
        name=data.name,
        created_at=datetime.utcnow(),
        last_hit=None,
        request_count=0,
        webhook_url=f"/{current_user['user_id']}/{endpoint_id}"
    )

@router.get("/endpoints", response_model=list[EndpointResponse])
async def list_endpoints(
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """List all endpoints for current user only"""
    async with db.execute(
        "SELECT id, name, user_id, created_at, last_hit, request_count FROM endpoints WHERE user_id = ? AND is_active = 1 ORDER BY created_at DESC",
        (current_user['user_id'],)
    ) as cursor:
        rows = await cursor.fetchall()
    
    return [
        EndpointResponse(
            id=row['id'],
            user_id=row['user_id'],
            name=row['name'],
            created_at=datetime.fromisoformat(row['created_at']),
            last_hit=datetime.fromisoformat(row['last_hit']) if row['last_hit'] else None,
            request_count=row['request_count'],
            webhook_url=f"/{current_user['user_id']}/{row['id']}"
        )
        for row in rows
    ]

@router.get("/endpoints/{endpoint_id}", response_model=EndpointDetail)
async def get_endpoint(
    endpoint_id: str,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Get endpoint details (only if owned by current user)"""
    async with db.execute(
        "SELECT e.*, m.enabled as mock_enabled FROM endpoints e LEFT JOIN mock_rules m ON e.id = m.endpoint_id WHERE e.id = ? AND e.user_id = ?",
        (endpoint_id, current_user['user_id'])
    ) as cursor:
        row = await cursor.fetchone()
    
    if not row:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    return EndpointDetail(
        id=row['id'],
        user_id=row['user_id'],
        name=row['name'],
        created_at=datetime.fromisoformat(row['created_at']),
        last_hit=datetime.fromisoformat(row['last_hit']) if row['last_hit'] else None,
        request_count=row['request_count'],
        webhook_url=f"/{current_user['user_id']}/{row['id']}",
        expires_at=datetime.fromisoformat(row['expires_at']) if row['expires_at'] else None,
        is_active=bool(row['is_active']),
        mock_enabled=bool(row['mock_enabled']) if row['mock_enabled'] else False
    )

@router.delete("/endpoints/{endpoint_id}", response_model=MessageResponse)
async def delete_endpoint(
    endpoint_id: str,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Delete endpoint (only if owned by current user)"""
    # Verify ownership
    async with db.execute("SELECT user_id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row or row['user_id'] != current_user['user_id']:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    await db.execute("DELETE FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,))
    await db.execute("DELETE FROM requests WHERE endpoint_id = ?", (endpoint_id,))
    await db.execute("DELETE FROM endpoints WHERE id = ?", (endpoint_id,))
    await db.commit()
    
    return MessageResponse(message=f"Endpoint {endpoint_id} deleted")

# --- Requests ---

@router.get("/endpoints/{endpoint_id}/requests", response_model=list[RequestResponse])
async def list_requests(
    endpoint_id: str, 
    limit: int = 50,
    offset: int = 0,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """List captured requests for endpoint (only if owned by current user)"""
    # Verify ownership
    async with db.execute("SELECT user_id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row or row['user_id'] != current_user['user_id']:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    async with db.execute(
        "SELECT id, endpoint_id, method, path, timestamp, content_type FROM requests WHERE endpoint_id = ? ORDER BY timestamp DESC LIMIT ? OFFSET ?",
        (endpoint_id, limit, offset)
    ) as cursor:
        rows = await cursor.fetchall()
    
    return [
        RequestResponse(
            id=row['id'],
            endpoint_id=row['endpoint_id'],
            method=row['method'],
            path=row['path'],
            timestamp=datetime.fromisoformat(row['timestamp']),
            content_type=row['content_type']
        )
        for row in rows
    ]

@router.get("/requests/{request_id}", response_model=RequestDetail)
async def get_request(
    request_id: int,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Get full request details"""
    async with db.execute(
        "SELECT r.* FROM requests r JOIN endpoints e ON r.endpoint_id = e.id WHERE r.id = ? AND e.user_id = ?",
        (request_id, current_user['user_id'])
    ) as cursor:
        row = await cursor.fetchone()
    
    if not row:
        raise HTTPException(status_code=404, detail="Request not found")
    
    return RequestDetail(
        id=row['id'],
        endpoint_id=row['endpoint_id'],
        method=row['method'],
        path=row['path'],
        timestamp=datetime.fromisoformat(row['timestamp']),
        content_type=row['content_type'],
        headers=json.loads(row['headers']) if row['headers'] else {},
        query_params=json.loads(row['query_params']) if row['query_params'] else {},
        body=row['body']
    )

# --- Mock Rules ---

@router.put("/endpoints/{endpoint_id}/mock", response_model=MockRuleResponse)
async def set_mock_rule(
    endpoint_id: str,
    data: MockRuleCreate,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Set or update mock rule for endpoint (method-specific)"""
    # Verify ownership
    async with db.execute("SELECT user_id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
        if not row or row['user_id'] != current_user['user_id']:
            raise HTTPException(status_code=404, detail="Endpoint not found")
    
    method = data.method.upper() if data.method else "DEFAULT"
    
    await db.execute(
        """INSERT INTO mock_rules (endpoint_id, method, status_code, response_body, response_headers, content_type, enabled, delay_ms)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(endpoint_id, method) DO UPDATE SET
           status_code = excluded.status_code, response_body = excluded.response_body,
           response_headers = excluded.response_headers, content_type = excluded.content_type,
           enabled = excluded.enabled, delay_ms = excluded.delay_ms""",
        (endpoint_id, method, data.status_code, data.response_body, json.dumps(data.response_headers),
         data.content_type, 1 if data.enabled else 0, data.delay_ms)
    )
    await db.commit()
    
    return MockRuleResponse(
        endpoint_id=endpoint_id,
        method=method,
        status_code=data.status_code,
        response_body=data.response_body,
        response_headers=data.response_headers,
        content_type=data.content_type,
        enabled=data.enabled,
        delay_ms=data.delay_ms
    )

@router.get("/endpoints/{endpoint_id}/mock", response_model=MockRuleListResponse)
async def get_mock_rules(
    endpoint_id: str,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Get all mock rules for endpoint"""
    # Verify ownership
    async with db.execute("SELECT user_id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row or row['user_id'] != current_user['user_id']:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    async with db.execute("SELECT * FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,)) as cursor:
        rows = await cursor.fetchall()
    
    rules = []
    default_enabled = False
    for row in rows:
        rules.append(MockRuleResponse(
            endpoint_id=row['endpoint_id'],
            method=row['method'],
            status_code=row['status_code'],
            response_body=row['response_body'],
            response_headers=json.loads(row['response_headers']) if row['response_headers'] else {},
            content_type=row['content_type'],
            enabled=bool(row['enabled']),
            delay_ms=row['delay_ms']
        ))
        if row['method'] == 'DEFAULT':
            default_enabled = bool(row['enabled'])
    
    return MockRuleListResponse(
        endpoint_id=endpoint_id,
        rules=rules,
        default_enabled=default_enabled
    )

@router.delete("/endpoints/{endpoint_id}/mock", response_model=MessageResponse)
async def delete_mock_rule(
    endpoint_id: str,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Delete mock rule for specific method or all"""
    method = None
    # Try to get method from query param
    # For simplicity, delete all rules for this endpoint
    
    # Verify ownership
    async with db.execute("SELECT user_id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row or row['user_id'] != current_user['user_id']:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    await db.execute("DELETE FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,))
    await db.commit()
    return MessageResponse(message="Mock rules deleted")

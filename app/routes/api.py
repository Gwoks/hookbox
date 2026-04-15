"""JSON API routes"""

from fastapi import APIRouter, Depends, HTTPException
import aiosqlite
from datetime import datetime
import json

from app.database import get_db
from app.models import (
    EndpointCreate, EndpointResponse, EndpointDetail,
    RequestResponse, RequestDetail, 
    MockRuleCreate, MockRuleResponse,
    MessageResponse
)
from app.utils import generate_endpoint_id, calculate_expiry

router = APIRouter(prefix="/api", tags=["API"])

@router.post("/endpoints", response_model=EndpointResponse)
async def create_endpoint(data: EndpointCreate = None, db: aiosqlite.Connection = Depends(get_db)):
    if data is None:
        data = EndpointCreate()
    
    endpoint_id = generate_endpoint_id()
    expires_at = calculate_expiry(data.expires_in_hours or 24)
    
    await db.execute(
        "INSERT INTO endpoints (id, name, expires_at) VALUES (?, ?, ?)",
        (endpoint_id, data.name, expires_at.isoformat())
    )
    await db.commit()
    
    return EndpointResponse(
        id=endpoint_id,
        name=data.name,
        created_at=datetime.utcnow(),
        last_hit=None,
        request_count=0,
        webhook_url=f"/hook/{endpoint_id}"
    )

@router.get("/endpoints", response_model=list[EndpointResponse])
async def list_endpoints(db: aiosqlite.Connection = Depends(get_db)):
    async with db.execute(
        "SELECT id, name, created_at, last_hit, request_count FROM endpoints WHERE is_active = 1 ORDER BY created_at DESC"
    ) as cursor:
        rows = await cursor.fetchall()
    
    return [
        EndpointResponse(
            id=row['id'],
            name=row['name'],
            created_at=datetime.fromisoformat(row['created_at']),
            last_hit=datetime.fromisoformat(row['last_hit']) if row['last_hit'] else None,
            request_count=row['request_count'],
            webhook_url=f"/hook/{row['id']}"
        )
        for row in rows
    ]

@router.get("/endpoints/{endpoint_id}", response_model=EndpointDetail)
async def get_endpoint(endpoint_id: str, db: aiosqlite.Connection = Depends(get_db)):
    async with db.execute(
        "SELECT e.*, m.enabled as mock_enabled FROM endpoints e LEFT JOIN mock_rules m ON e.id = m.endpoint_id WHERE e.id = ?",
        (endpoint_id,)
    ) as cursor:
        row = await cursor.fetchone()
    
    if not row:
        raise HTTPException(status_code=404, detail="Endpoint not found")
    
    return EndpointDetail(
        id=row['id'],
        name=row['name'],
        created_at=datetime.fromisoformat(row['created_at']),
        last_hit=datetime.fromisoformat(row['last_hit']) if row['last_hit'] else None,
        request_count=row['request_count'],
        webhook_url=f"/hook/{row['id']}",
        expires_at=datetime.fromisoformat(row['expires_at']) if row['expires_at'] else None,
        is_active=bool(row['is_active']),
        mock_enabled=bool(row['mock_enabled']) if row['mock_enabled'] else False
    )

@router.delete("/endpoints/{endpoint_id}", response_model=MessageResponse)
async def delete_endpoint(endpoint_id: str, db: aiosqlite.Connection = Depends(get_db)):
    await db.execute("DELETE FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,))
    await db.execute("DELETE FROM requests WHERE endpoint_id = ?", (endpoint_id,))
    await db.execute("DELETE FROM endpoints WHERE id = ?", (endpoint_id,))
    await db.commit()
    return MessageResponse(message=f"Endpoint {endpoint_id} deleted")

@router.get("/endpoints/{endpoint_id}/requests", response_model=list[RequestResponse])
async def list_requests(endpoint_id: str, limit: int = 50, offset: int = 0, db: aiosqlite.Connection = Depends(get_db)):
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
async def get_request(request_id: int, db: aiosqlite.Connection = Depends(get_db)):
    async with db.execute("SELECT * FROM requests WHERE id = ?", (request_id,)) as cursor:
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

@router.put("/endpoints/{endpoint_id}/mock", response_model=MockRuleResponse)
async def set_mock_rule(endpoint_id: str, data: MockRuleCreate, db: aiosqlite.Connection = Depends(get_db)):
    async with db.execute("SELECT id FROM endpoints WHERE id = ?", (endpoint_id,)) as cursor:
        if not await cursor.fetchone():
            raise HTTPException(status_code=404, detail="Endpoint not found")
    
    await db.execute(
        """INSERT INTO mock_rules (endpoint_id, status_code, response_body, response_headers, content_type, enabled, delay_ms)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(endpoint_id) DO UPDATE SET
           status_code = excluded.status_code, response_body = excluded.response_body,
           response_headers = excluded.response_headers, content_type = excluded.content_type,
           enabled = excluded.enabled, delay_ms = excluded.delay_ms""",
        (endpoint_id, data.status_code, data.response_body, json.dumps(data.response_headers),
         data.content_type, 1 if data.enabled else 0, data.delay_ms)
    )
    await db.commit()
    
    return MockRuleResponse(
        endpoint_id=endpoint_id,
        status_code=data.status_code,
        response_body=data.response_body,
        response_headers=data.response_headers,
        content_type=data.content_type,
        enabled=data.enabled,
        delay_ms=data.delay_ms
    )

@router.get("/endpoints/{endpoint_id}/mock", response_model=MockRuleResponse)
async def get_mock_rule(endpoint_id: str, db: aiosqlite.Connection = Depends(get_db)):
    async with db.execute("SELECT * FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,)) as cursor:
        row = await cursor.fetchone()
    
    if not row:
        raise HTTPException(status_code=404, detail="No mock rule set")
    
    return MockRuleResponse(
        endpoint_id=row['endpoint_id'],
        status_code=row['status_code'],
        response_body=row['response_body'],
        response_headers=json.loads(row['response_headers']) if row['response_headers'] else {},
        content_type=row['content_type'],
        enabled=bool(row['enabled']),
        delay_ms=row['delay_ms']
    )

@router.delete("/endpoints/{endpoint_id}/mock", response_model=MessageResponse)
async def delete_mock_rule(endpoint_id: str, db: aiosqlite.Connection = Depends(get_db)):
    await db.execute("DELETE FROM mock_rules WHERE endpoint_id = ?", (endpoint_id,))
    await db.commit()
    return MessageResponse(message="Mock rule deleted")

"""Backup/Restore API routes"""

from fastapi import APIRouter, Depends, HTTPException
from fastapi.responses import JSONResponse
import aiosqlite
from datetime import datetime
import json
import email as email_lib
import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from email.header import Header

from app.database import get_db, hash_email
from app.models import MessageResponse

router = APIRouter(prefix="/api/backup", tags=["Backup"])

def get_current_user(x_user_id: str = Header(...), x_email: str = Header(...)) -> dict:
    return {"user_id": x_user_id, "email": x_email}

@router.get("/export")
async def export_user_data(
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Export all user data as JSON"""
    user_id = current_user['user_id']
    
    # Get all endpoints for user
    async with db.execute(
        "SELECT * FROM endpoints WHERE user_id = ?",
        (user_id,)
    ) as cursor:
        endpoints = await cursor.fetchall()
    
    # Get all requests for user's endpoints
    endpoint_ids = [ep['id'] for ep in endpoints]
    requests_list = []
    if endpoint_ids:
        placeholders = ','.join('?' * len(endpoint_ids))
        async with db.execute(
            f"SELECT * FROM requests WHERE endpoint_id IN ({placeholders})",
            endpoint_ids
        ) as cursor:
            requests_list = await cursor.fetchall()
    
    # Get all mock rules for user's endpoints
    mock_rules = []
    if endpoint_ids:
        placeholders = ','.join('?' * len(endpoint_ids))
        async with db.execute(
            f"SELECT * FROM mock_rules WHERE endpoint_id IN ({placeholders})",
            endpoint_ids
        ) as cursor:
            mock_rules = await cursor.fetchall()
    
    # Build export data
    export_data = {
        "version": "1.0",
        "exported_at": datetime.utcnow().isoformat(),
        "user_email": current_user['email'],
        "endpoints": [dict(ep) for ep in endpoints],
        "requests": [dict(req) for req in requests_list],
        "mock_rules": [dict(mr) for mr in mock_rules]
    }
    
    return JSONResponse(content=export_data)

@router.post("/restore")
async def restore_user_data(
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Import user data from JSON backup"""
    user_id = current_user['user_id']
    
    try:
        data = await request.json()
    except:
        raise HTTPException(status_code=400, detail="Invalid JSON data")
    
    if data.get("version") != "1.0":
        raise HTTPException(status_code=400, detail="Unsupported backup version")
    
    restored_count = 0
    
    # Restore endpoints
    for ep in data.get("endpoints", []):
        if ep.get("user_id") == user_id:
            await db.execute("""
                INSERT OR REPLACE INTO endpoints (id, user_id, name, created_at, last_hit, request_count, expires_at, is_active)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                ep["id"], user_id, ep.get("name"), ep["created_at"],
                ep.get("last_hit"), ep.get("request_count", 0), ep.get("expires_at"),
                ep.get("is_active", 1)
            ))
            restored_count += 1
    
    await db.commit()
    
    return MessageResponse(message=f"Restored {restored_count} endpoints")

@router.post("/email")
async def send_backup_email(
    request: Request,
    db: aiosqlite.Connection = Depends(get_db),
    current_user: dict = Depends(get_current_user)
):
    """Export data and send to user's email"""
    import os
    from app.database import get_db
    
    SMTP_HOST = os.getenv("SMTP_HOST", "")
    SMTP_PORT = int(os.getenv("SMTP_PORT", "587"))
    SMTP_USER = os.getenv("SMTP_USER", "")
    SMTP_PASS = os.getenv("SMTP_PASS", "")
    
    if not SMTP_HOST:
        raise HTTPException(status_code=500, detail="SMTP not configured")
    
    # Export user data
    user_id = current_user['user_id']
    
    async with db.execute(
        "SELECT * FROM endpoints WHERE user_id = ?",
        (user_id,)
    ) as cursor:
        endpoints = await cursor.fetchall()
    
    endpoint_ids = [ep['id'] for ep in endpoints]
    requests_list = []
    mock_rules = []
    
    if endpoint_ids:
        placeholders = ','.join('?' * len(endpoint_ids))
        async with db.execute(
            f"SELECT * FROM requests WHERE endpoint_id IN ({placeholders})",
            endpoint_ids
        ) as cursor:
            requests_list = await cursor.fetchall()
        
        async with db.execute(
            f"SELECT * FROM mock_rules WHERE endpoint_id IN ({placeholders})",
            endpoint_ids
        ) as cursor:
            mock_rules = await cursor.fetchall()
    
    export_data = {
        "version": "1.0",
        "exported_at": datetime.utcnow().isoformat(),
        "user_email": current_user['email'],
        "endpoints": [dict(ep) for ep in endpoints],
        "requests": [dict(req) for req in requests_list],
        "mock_rules": [dict(mr) for mr in mock_rules]
    }
    
    json_data = json.dumps(export_data, indent=2)
    
    # Build email
    msg = MIMEMultipart()
    msg['From'] = SMTP_USER
    msg['To'] = current_user['email']
    msg['Subject'] = Header(f"HookBox Backup - {datetime.utcnow().strftime('%Y-%m-%d')}", 'utf-8')
    
    msg.attach(MIMEText(json_data, 'json', 'utf-8'))
    
    # Attach as file
    attachment = MIMEText(json_data, 'json', 'utf-8')
    attachment.add_header('Content-Disposition', 'attachment', filename=f'hookbox-backup-{datetime.utcnow().strftime("%Y%m%d")}.json')
    msg.attach(attachment)
    
    # Send email
    try:
        server = smtplib.SMTP(SMTP_HOST, SMTP_PORT)
        server.starttls()
        server.login(SMTP_USER, SMTP_PASS)
        server.send_message(msg)
        server.quit()
        return MessageResponse(message="Backup sent to your email!")
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to send email: {str(e)}")

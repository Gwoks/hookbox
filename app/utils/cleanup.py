"""Background cleanup task for expired data"""

import asyncio
from datetime import datetime, timedelta
from app.database import DATABASE_PATH
import aiosqlite
from config import AUTO_DELETE_HOURS

async def cleanup_expired_data():
    cutoff = datetime.utcnow() - timedelta(hours=AUTO_DELETE_HOURS)
    cutoff_str = cutoff.isoformat()
    
    async with aiosqlite.connect(DATABASE_PATH) as db:
        result = await db.execute("DELETE FROM requests WHERE timestamp < ?", (cutoff_str,))
        deleted_requests = result.rowcount
        result = await db.execute("DELETE FROM endpoints WHERE expires_at < ? AND is_active = 1", (cutoff_str,))
        deleted_endpoints = result.rowcount
        await db.execute("DELETE FROM mock_rules WHERE endpoint_id NOT IN (SELECT id FROM endpoints)")
        await db.commit()
        if deleted_requests or deleted_endpoints:
            print(f"[Cleanup] Deleted {deleted_requests} requests, {deleted_endpoints} endpoints")
        return deleted_requests, deleted_endpoints

async def cleanup_loop():
    while True:
        try:
            await cleanup_expired_data()
        except Exception as e:
            print(f"[Cleanup] Error: {e}")
        await asyncio.sleep(3600)

def start_cleanup_task():
    task = asyncio.create_task(cleanup_loop())
    print("[Cleanup] Background cleanup task started")
    return task

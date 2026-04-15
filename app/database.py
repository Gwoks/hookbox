"""SQLite database setup and connection"""

import aiosqlite
from pathlib import Path
from config import DATABASE_PATH

DATABASE_PATH.parent.mkdir(parents=True, exist_ok=True)

async def get_db():
    """Get database connection"""
    db = await aiosqlite.connect(DATABASE_PATH)
    db.row_factory = aiosqlite.Row
    try:
        yield db
    finally:
        await db.close()

async def init_db():
    """Initialize database tables"""
    async with aiosqlite.connect(DATABASE_PATH) as db:
        await db.execute("""
            CREATE TABLE IF NOT EXISTS endpoints (
                id TEXT PRIMARY KEY,
                name TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                last_hit TIMESTAMP,
                request_count INTEGER DEFAULT 0,
                expires_at TIMESTAMP,
                is_active INTEGER DEFAULT 1
            )
        """)
        
        await db.execute("""
            CREATE TABLE IF NOT EXISTS requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint_id TEXT,
                method TEXT,
                path TEXT,
                headers TEXT,
                query_params TEXT,
                body TEXT,
                content_type TEXT,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (endpoint_id) REFERENCES endpoints(id)
            )
        """)
        
        await db.execute("""
            CREATE TABLE IF NOT EXISTS mock_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint_id TEXT UNIQUE,
                status_code INTEGER DEFAULT 200,
                response_body TEXT DEFAULT '',
                response_headers TEXT DEFAULT '{}',
                content_type TEXT DEFAULT 'application/json',
                enabled INTEGER DEFAULT 1,
                delay_ms INTEGER DEFAULT 0,
                FOREIGN KEY (endpoint_id) REFERENCES endpoints(id)
            )
        """)
        
        await db.execute("CREATE INDEX IF NOT EXISTS idx_requests_endpoint ON requests(endpoint_id)")
        await db.execute("CREATE INDEX IF NOT EXISTS idx_requests_timestamp ON requests(timestamp)")
        
        await db.commit()
        print("Database initialized successfully!")

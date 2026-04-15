"""FastAPI main application"""

from fastapi import FastAPI, Request, Depends
from fastapi.responses import JSONResponse, Response, HTMLResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
import json
import asyncio
from datetime import datetime

from app.database import init_db, get_db
from app.routes.api import router as api_router
from app.routes.webhook import router as webhook_router
from app.utils.cleanup import start_cleanup_task

app = FastAPI(title="WebhookCatch", description="Self-hosted webhook testing service", version="1.0.0")

app.mount("/static", StaticFiles(directory="static"), name="static")
templates = Jinja2Templates(directory="templates")
app.include_router(api_router)
app.include_router(webhook_router)

@app.on_event("startup")
async def startup():
    await init_db()
    start_cleanup_task()

@app.api_route("/hook/{endpoint_id}", methods=["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"])
async def receive_webhook(endpoint_id: str, request: Request, db = Depends(get_db)):
    async with db.execute(
        "SELECT * FROM mock_rules WHERE endpoint_id = ? AND enabled = 1",
        (endpoint_id,)
    ) as cursor:
        mock = await cursor.fetchone()
    
    headers = dict(request.headers)
    query_params = dict(request.query_params)
    body = await request.body()
    content_type = headers.get('content-type', '')
    body_text = body.decode('utf-8', errors='replace') if body else ''
    
    now = datetime.utcnow().isoformat()
    await db.execute(
        "INSERT INTO requests (endpoint_id, method, path, headers, query_params, body, content_type, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (endpoint_id, request.method, str(request.url.path), json.dumps(dict(headers)),
         json.dumps(dict(query_params)), body_text[:1_000_000], content_type, now)
    )
    await db.execute("UPDATE endpoints SET request_count = request_count + 1, last_hit = ? WHERE id = ?", (now, endpoint_id))
    await db.commit()
    
    if mock:
        delay_ms = mock['delay_ms'] or 0
        if delay_ms > 0:
            await asyncio.sleep(delay_ms / 1000)
        
        response_headers = json.loads(mock['response_headers']) if mock['response_headers'] else {}
        response_headers['Content-Type'] = mock['content_type']
        
        return Response(content=mock['response_body'], status_code=mock['status_code'], headers=response_headers)
    
    return JSONResponse(content={"status": "received", "endpoint_id": endpoint_id, "timestamp": now}, status_code=200)

@app.get("/", response_class=HTMLResponse)
async def home(request: Request):
    return templates.TemplateResponse("index.html", {"request": request})

@app.get("/d/{endpoint_id}", response_class=HTMLResponse)
async def dashboard(request: Request, endpoint_id: str):
    return templates.TemplateResponse("dashboard.html", {"request": request})

@app.get("/m/{endpoint_id}", response_class=HTMLResponse)
async def mock_config(request: Request, endpoint_id: str):
    return templates.TemplateResponse("mock.html", {"request": request})

if __name__ == "__main__":
    import uvicorn
    from config import HOST, PORT
    uvicorn.run(app, host=HOST, port=PORT)

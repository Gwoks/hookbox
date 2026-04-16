"""FastAPI main application"""

from fastapi import FastAPI, Request, Depends, Header, WebSocket
from fastapi.responses import JSONResponse, Response, HTMLResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
import json
import asyncio
from datetime import datetime
from pathlib import Path

from app.database import init_db, get_db
from app.routes.api import router as api_router
from app.routes.backup import router as backup_router
from app.websocket import manager

app = FastAPI(title="HookBox", description="Self-hosted webhook testing service", version="1.0.0")

BASE_DIR = Path(__file__).parent.parent
app.mount("/static", StaticFiles(directory=str(BASE_DIR / "static")), name="static")
templates = Jinja2Templates(directory=str(BASE_DIR / "templates"))
app.include_router(api_router)
app.include_router(backup_router)

@app.on_event("startup")
async def startup():
    await init_db()

@app.websocket("/ws/{endpoint_id}")
async def websocket_endpoint(websocket, endpoint_id: str):
    await manager.connect(websocket, endpoint_id)
    try:
        while True:
            # Keep connection alive, wait for close
            data = await websocket.receive_text()
    except Exception:
        pass
    finally:
        manager.disconnect(websocket, endpoint_id)

@app.api_route("/hook/{user_id}/{endpoint_id}", methods=["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"])
async def receive_webhook(user_id: str, endpoint_id: str, request: Request, db = Depends(get_db)):
    # Try method-specific mock first, then fall back to DEFAULT
    mock = None
    async with db.execute(
        "SELECT * FROM mock_rules WHERE endpoint_id = ? AND method = ? AND enabled = 1",
        (endpoint_id, request.method)
    ) as cursor:
        mock = await cursor.fetchone()
    
    # Fall back to DEFAULT if no method-specific rule
    if not mock:
        async with db.execute(
            "SELECT * FROM mock_rules WHERE endpoint_id = ? AND method = 'DEFAULT' AND enabled = 1",
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
    
    # Broadcast to WebSocket clients
    await manager.broadcast_new_request(endpoint_id, {
        "method": request.method,
        "path": str(request.url.path),
        "content_type": content_type,
        "timestamp": now
    })
    
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

@app.get("/login", response_class=HTMLResponse)
async def login_page(request: Request):
    return templates.TemplateResponse("login.html", {"request": request})

@app.get("/register", response_class=HTMLResponse)
async def register_page(request: Request):
    return templates.TemplateResponse("register.html", {"request": request})

@app.get("/backup", response_class=HTMLResponse)
async def backup_page(request: Request):
    return templates.TemplateResponse("backup.html", {"request": request})

if __name__ == "__main__":
    import uvicorn
    from config import HOST, PORT
    uvicorn.run(app, host=HOST, port=PORT)

@app.get("/status", response_class=HTMLResponse)
async def crypto_status(request: Request):
    """Crypto Trading Bot Status Page"""
    import json
    import os
    from datetime import datetime
    
    # Paths
    WORKSPACE = "/home/ubuntu/.openclaw/workspace/openclaw"
    holdings_path = f"{WORKSPACE}/holdings.json"
    research_path = f"{WORKSPACE}/stocks/research_6h.json"
    signals_path = f"{WORKSPACE}/stocks/signals.json"
    heartbeat_path = f"{WORKSPACE}/heartbeat.log"
    
    # Load data
    holdings = {}
    try:
        if os.path.exists(holdings_path):
            with open(holdings_path) as f:
                holdings = json.load(f)
    except: pass
    
    research = {}
    try:
        if os.path.exists(research_path):
            with open(research_path) as f:
                research = json.load(f)
    except: pass
    
    signals = {}
    try:
        if os.path.exists(signals_path):
            with open(signals_path) as f:
                signals = json.load(f)
    except: pass
    
    heartbeat_log = ""
    try:
        if os.path.exists(heartbeat_path):
            with open(heartbeat_path) as f:
                heartbeat_log = f.read()[-2000:]
    except: pass
    
    # Get FGI
    fgi_value = research.get("market", {}).get("fear_greed", {}).get("value", "N/A")
    fgi_class = research.get("market", {}).get("fear_greed", {}).get("classification", "Unknown")
    
    positions = holdings.get("positions", [])
    total_value = sum(p.get("current_value_usdt", 0) for p in positions)
    
    html = f"""
<!DOCTYPE html>
<html>
<head>
    <title>Crypto Trading Bot Status</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0d1117; color: #e6edf3; padding: 20px; line-height: 1.6; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        h1 {{ color: #58a6ff; margin-bottom: 20px; }}
        h2 {{ color: #8b949e; font-size: 14px; text-transform: uppercase; margin: 25px 0 10px; border-bottom: 1px solid #30363d; padding-bottom: 5px; }}
        .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 15px; margin-bottom: 15px; }}
        .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 10px; }}
        .stat {{ background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 12px; text-align: center; }}
        .stat .label {{ color: #8b949e; font-size: 11px; text-transform: uppercase; }}
        .stat .value {{ font-size: 18px; font-weight: bold; margin-top: 5px; }}
        .green {{ color: #3fb950; }}
        .red {{ color: #f85149; }}
        table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
        th, td {{ padding: 8px 12px; text-align: left; border-bottom: 1px solid #30363d; }}
        th {{ color: #8b949e; font-size: 11px; text-transform: uppercase; background: #161b22; }}
        pre {{ background: #0d1117; padding: 10px; border-radius: 4px; overflow-x: auto; font-size: 11px; max-height: 200px; }}
        .nav {{ display: flex; gap: 15px; margin-bottom: 20px; }}
        .nav a {{ color: #58a6ff; text-decoration: none; padding: 8px 16px; background: #161b22; border-radius: 6px; }}
        .nav a:hover {{ background: #21262d; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="nav">
            <a href="/">← HookBox</a>
            <a href="/status">Crypto Status</a>
        </div>
        
        <h1>🤙 Crypto Trading Bot Status</h1>
        
        <h2>📊 Portfolio</h2>
        <div class="grid">
            <div class="stat">
                <div class="label">Fear & Greed</div>
                <div class="value">{fgi_value} ({fgi_class})</div>
            </div>
            <div class="stat">
                <div class="label">Mode</div>
                <div class="value">{holdings.get('baseline', {}).get('mode', 'N/A')}</div>
            </div>
            <div class="stat">
                <div class="label">Total Value</div>
                <div class="value">${total_value:.2f}</div>
            </div>
            <div class="stat">
                <div class="label">Positions</div>
                <div class="value">{len(positions)}</div>
            </div>
        </div>
        
        <h2>💰 Positions</h2>
        <div class="card">
            <table>
                <thead><tr><th>Symbol</th><th>Qty</th><th>Buy Price</th><th>Current</th><th>Value</th><th>P&L</th></tr></thead>
                <tbody>
"""
    
    for pos in positions:
        pnl = pos.get('pnl_usdt', 0)
        pnl_pct = ((pos.get('current_price', 0) / pos.get('buy_price', 1)) - 1) * 100
        pnl_class = 'green' if pnl >= 0 else 'red'
        html += f"""<tr>
                        <td><strong>{pos.get('symbol', 'N/A')}</strong></td>
                        <td>{pos.get('buy_quantity', 0):.4f}</td>
                        <td>${pos.get('buy_price', 0):.4f}</td>
                        <td>${pos.get('current_price', 0):.4f}</td>
                        <td>${pos.get('current_value_usdt', 0):.2f}</td>
                        <td class="{pnl_class}">{pnl_pct:+.2f}%</td>
                    </tr>"""
    
    html += f"""</tbody></table></div>
        
        <h2>📡 Research</h2>
        <div class="card">
            <p>Sources: {', '.join(research.get('sources_used', ['None']))}</p>
            <p>Tickers processed: {research.get('total_tickers', 0)}</p>
        </div>
        
        <h2>📋 Recent Heartbeat</h2>
        <pre>{heartbeat_log or 'No heartbeat data'}</pre>
        
        <div style="text-align: center; color: #8b949e; margin-top: 30px; padding-top: 20px; border-top: 1px solid #30363d;">
            Last updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}
        </div>
    </div>
</body>
</html>"""
    return HTMLResponse(content=html)

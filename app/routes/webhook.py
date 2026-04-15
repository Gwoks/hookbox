"""Webhook receivers (GitHub, etc.)"""

import os
import hmac
import hashlib
import asyncio
from fastapi import APIRouter, Request, HTTPException

router = APIRouter()

GITHUB_WEBHOOK_SECRET = os.getenv("GITHUB_WEBHOOK_SECRET", "")

def verify_github_signature(payload_bytes: bytes, signature: str, secret: str) -> bool:
    if not secret:
        return True
    if not signature:
        return False
    expected = hmac.new(secret.encode(), payload_bytes, hashlib.sha256).hexdigest()
    actual = signature.replace("sha256=", "")
    return hmac.compare_digest(expected, actual)

async def restart_service():
    print("[AutoDeploy] Restarting service...")
    os.system("pkill -f 'python.*app.main' || true")
    await asyncio.sleep(2)
    os.chdir("/home/ubuntu/hookbox")
    os.system("nohup python3.14 -m app.main > /home/ubuntu/hookbox/webhookcatch.log 2>&1 &")
    print("[AutoDeploy] Service restarted!")

@router.api_route("/webhook/github", methods=["GET", "POST"])
async def github_webhook(request: Request):
    signature = request.headers.get("x-hub-signature-256", "")
    payload_bytes = await request.body()
    
    if GITHUB_WEBHOOK_SECRET:
        if not verify_github_signature(payload_bytes, signature, GITHUB_WEBHOOK_SECRET):
            raise HTTPException(status_code=403, detail="Invalid signature")
    
    event = request.headers.get("x-github-event", "")
    print(f"[GitHub Webhook] Received: {event}")
    
    if event == "push":
        print("[AutoDeploy] Pulling latest code...")
        os.chdir("/home/ubuntu/hookbox")
        result = os.system("git pull origin main")
        if result == 0:
            print("[AutoDeploy] Code pulled successfully")
            await restart_service()
            return {"status": "success", "message": "Code pulled and service restarted"}
        else:
            return {"status": "error", "message": "Git pull failed"}
    elif event == "ping":
        return {"status": "success", "message": "Pong!"}
    return {"status": "ignored", "message": f"Event {event} not handled"}

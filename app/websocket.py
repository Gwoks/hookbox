"""WebSocket connection manager"""

from fastapi import WebSocket
from typing import Dict, List
import json

class ConnectionManager:
    def __init__(self):
        # endpoint_id -> list of websockets
        self.active_connections: Dict[str, List[WebSocket]] = {}

    async def connect(self, websocket: WebSocket, endpoint_id: str):
        await websocket.accept()
        if endpoint_id not in self.active_connections:
            self.active_connections[endpoint_id] = []
        self.active_connections[endpoint_id].append(websocket)

    def disconnect(self, websocket: WebSocket, endpoint_id: str):
        if endpoint_id in self.active_connections:
            if websocket in self.active_connections[endpoint_id]:
                self.active_connections[endpoint_id].remove(websocket)
            if not self.active_connections[endpoint_id]:
                del self.active_connections[endpoint_id]

    async def broadcast_to_endpoint(self, endpoint_id: str, message: dict):
        """Broadcast message to all connections for this endpoint"""
        if endpoint_id in self.active_connections:
            disconnected = []
            for connection in self.active_connections[endpoint_id]:
                try:
                    await connection.send_json(message)
                except Exception:
                    disconnected.append(connection)
            # Clean up disconnected
            for conn in disconnected:
                self.disconnect(conn, endpoint_id)

    async def broadcast_new_request(self, endpoint_id: str, request_data: dict):
        """Broadcast new request event"""
        await self.broadcast_to_endpoint(endpoint_id, {
            "type": "new_request",
            "data": request_data
        })

manager = ConnectionManager()

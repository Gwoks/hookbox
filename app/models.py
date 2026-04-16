"""Pydantic models for request/response validation"""

from pydantic import BaseModel, Field, EmailStr
from typing import Optional, Dict, Any, List
from datetime import datetime

# --- User Models ---

class UserRegister(BaseModel):
    email: EmailStr

class UserLogin(BaseModel):
    email: EmailStr

class UserResponse(BaseModel):
    id: str
    email: str
    created_at: datetime
    last_login: Optional[datetime]
    token: str

class UserSession(BaseModel):
    user_id: str
    email: str
    token: str

# --- Endpoint Models ---

class EndpointCreate(BaseModel):
    name: Optional[str] = Field(None, max_length=100)
    expires_in_hours: Optional[int] = Field(24, ge=1, le=720)

class MockRuleCreate(BaseModel):
    method: str = Field("DEFAULT", max_length=20)
    status_code: int = Field(200, ge=100, le=599)
    response_body: str = Field("", max_length=1_000_000)
    response_headers: Dict[str, str] = {}
    content_type: str = "application/json"
    enabled: bool = True
    delay_ms: int = Field(0, ge=0, le=30000)

class EndpointResponse(BaseModel):
    id: str
    name: Optional[str]
    created_at: datetime
    last_hit: Optional[datetime]
    request_count: int
    webhook_url: str
    user_id: str

class EndpointDetail(EndpointResponse):
    expires_at: Optional[datetime]
    is_active: bool
    mock_enabled: bool

class RequestResponse(BaseModel):
    id: int
    endpoint_id: str
    method: str
    path: str
    timestamp: datetime
    content_type: Optional[str]

class RequestDetail(RequestResponse):
    headers: Dict[str, str]
    query_params: Dict[str, Any]
    body: Optional[str]

class MockRuleResponse(BaseModel):
    endpoint_id: str
    method: str
    status_code: int
    response_body: str
    response_headers: Dict[str, str]
    content_type: str
    enabled: bool
    delay_ms: int

class MockRuleListResponse(BaseModel):
    endpoint_id: str
    rules: List[MockRuleResponse]
    default_enabled: bool

class MessageResponse(BaseModel):
    message: str
    success: bool = True

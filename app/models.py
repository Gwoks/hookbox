"""Pydantic v2 models — the FROZEN §5.3 interface contract (authoritative).

Field names and types are lifted verbatim from architecture.md §5.3 / prd.md §5.3.
FE and BE implement against exactly these shapes. Validation clamps (latency/rate/
chaos) and the MITM scheme allow-list (AC-S6) are enforced here at the boundary.
"""

from __future__ import annotations

from datetime import datetime
from typing import Literal, Optional
from urllib.parse import urlsplit

from pydantic import BaseModel, EmailStr, Field, field_validator


# ---- Session / owner ----
class SessionCreate(BaseModel):
    email: EmailStr


class EndpointSummary(BaseModel):
    token: str
    name: Optional[str] = None
    mock_url: str                 # https://<token>.<MOCK_DOMAIN>
    path_url: str                 # /e/<token>
    created_at: datetime
    last_hit: Optional[datetime] = None
    request_count: int


class SessionResponse(BaseModel):
    owner_id: str                 # hash_email, non-secret
    owner_secret: str             # bearer token (only returned here)
    endpoints: list[EndpointSummary]
    primary: EndpointSummary


# ---- Endpoint config ----
def _validate_target_url(v: Optional[str]) -> Optional[str]:
    """http(s) scheme allow-list (AC-S6). Empty/None clears the target."""
    if v is None:
        return None
    v = v.strip()
    if v == "":
        return None
    scheme = urlsplit(v).scheme.lower()
    if scheme not in ("http", "https"):
        raise ValueError("target_url must use the http or https scheme")
    if not urlsplit(v).netloc:
        raise ValueError("target_url must include a host")
    return v


class EndpointCreate(BaseModel):
    name: Optional[str] = Field(None, max_length=100)


class EndpointConfigPatch(BaseModel):       # all optional → partial update
    name: Optional[str] = Field(None, max_length=100)
    auto_crud: Optional[bool] = None
    target_url: Optional[str] = None        # MITM upstream; "" or null clears
    default_mode: Optional[Literal["mock_404", "echo"]] = None
    latency_ms: Optional[int] = Field(None, ge=0, le=10000)
    rate_limit_per_min: Optional[int] = Field(None, ge=0, le=100000)   # 0 = unlimited
    chaos_pct: Optional[int] = Field(None, ge=0, le=100)
    cors_enabled: Optional[bool] = None

    @field_validator("target_url")
    @classmethod
    def _check_target(cls, v: Optional[str]) -> Optional[str]:
        return _validate_target_url(v)


class EndpointDetail(BaseModel):
    token: str
    name: Optional[str] = None
    mock_url: str
    path_url: str
    auto_crud: bool
    target_url: Optional[str] = None
    default_mode: Literal["mock_404", "echo"]
    latency_ms: int
    rate_limit_per_min: int
    chaos_pct: int
    cors_enabled: bool
    tunnel_active: bool
    created_at: datetime
    last_hit: Optional[datetime] = None
    request_count: int


# ---- Mock rule (rich) ----
class BodyCondition(BaseModel):
    path: str                                # jsonpath-lite e.g. "user.role"
    op: Literal["eq", "neq", "contains", "exists"] = "eq"
    value: Optional[str] = None


class StateRequirement(BaseModel):
    key: str
    op: Literal["eq", "neq", "exists", "absent"] = "eq"
    value: Optional[str] = None


class MatchCriteria(BaseModel):
    method: str = "ANY"                      # "ANY" | GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS
    path: str = "/*"                         # exact, ":param" segments, or trailing "/*"
    headers: dict[str, str] = {}             # header name -> required value (case-insensitive)
    query: dict[str, str] = {}               # query key -> required value
    body_conditions: list[BodyCondition] = []
    state_requirements: list[StateRequirement] = []


class StateWrite(BaseModel):
    key: str
    value: str                               # may contain template tags


class ResponseSpec(BaseModel):
    status_code: int = Field(200, ge=100, le=599)
    headers: dict[str, str] = {}
    body_template: str = Field("", max_length=256_000)   # AC-S11 DoS bound
    content_type: str = "application/json"


class WebhookAction(BaseModel):
    url: str
    body_template: str = ""


class MockRuleCreate(BaseModel):
    name: Optional[str] = Field(None, max_length=120)
    priority: int = Field(100, ge=0, le=100000)   # lower = evaluated first
    enabled: bool = True
    match: MatchCriteria = MatchCriteria()
    response: ResponseSpec = ResponseSpec()
    state_writes: list[StateWrite] = []
    latency_ms: Optional[int] = Field(None, ge=0, le=10000)
    rate_limit_per_min: Optional[int] = Field(None, ge=0, le=100000)
    webhook_action: Optional[WebhookAction] = None    # accepted-and-stored; no-op v1 (OQ-9)


class MockRulePatch(BaseModel):              # all-optional mirror for PATCH
    name: Optional[str] = None
    priority: Optional[int] = Field(None, ge=0, le=100000)
    enabled: Optional[bool] = None
    match: Optional[MatchCriteria] = None
    response: Optional[ResponseSpec] = None
    state_writes: Optional[list[StateWrite]] = None
    latency_ms: Optional[int] = Field(None, ge=0, le=10000)
    rate_limit_per_min: Optional[int] = Field(None, ge=0, le=100000)
    webhook_action: Optional[WebhookAction] = None


class MockRule(MockRuleCreate):
    id: int
    token: str
    created_at: datetime


# ---- Traces ----
class RequestSummary(BaseModel):
    id: int
    token: str
    method: str
    path: str                                # the mock path (e.g. /users/5)
    status_code: int                         # served status
    served_by: Literal["rule", "crud", "mitm", "tunnel", "default", "cors", "chaos", "ratelimit"]
    matched_rule_id: Optional[int] = None
    duration_ms: int                         # total wall-clock (incl. applied latency)
    overhead_ms: int                         # our overhead = duration - applied latency - upstream
    timestamp: datetime


class TraceEvent(BaseModel):                 # the "State & Tracing Logs" tab payload, ordered
    step: str
    detail: str


class RequestDetail(RequestSummary):
    request_headers: dict[str, str] = {}
    query_params: dict[str, str] = {}
    request_body: Optional[str] = None
    response_headers: dict[str, str] = {}
    response_body: Optional[str] = None
    trace: list[TraceEvent] = []
    state_snapshot: dict[str, str] = {}      # state at time of request


# ---- Generic ----
class Message(BaseModel):
    message: str
    success: bool = True


# ---- WebSocket / SSE event payloads (§5.4) ----
class WsHello(BaseModel):
    token: str
    server_time: datetime


class WsStateChanged(BaseModel):
    token: str
    key: str
    value: str


class WsEndpointUpdated(BaseModel):
    token: str
    fields: list[str] = []

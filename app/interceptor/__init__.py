"""HookBox interceptor engine (P1 mock surface).

The deterministic, <10ms-budget pipeline that resolves a mock request:
matcher -> templating -> state -> CRUD -> tunnel -> MITM -> conditions -> CORS.
Each concern is its own module; ``engine.handle_mock`` orchestrates the frozen
resolution order (§5.5).
"""

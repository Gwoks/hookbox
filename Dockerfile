# HookBox — Beeceptor-class API mocking & interception platform (AC-42/43/44).
# Locked stack: FastAPI on uvicorn[standard] (async), aiosqlite (WAL), Redis,
# httpx, server-rendered Jinja2 + HTMX + Alpine.js + Tailwind. No Node build.
FROM python:3.12-slim

# Faster, quieter, deterministic Python in a container.
ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=1 \
    APP_PORT=8000 \
    DATABASE_PATH=/app/data/hookbox.db

WORKDIR /app

# Install dependencies first (better layer caching).
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy the application.
COPY . .

# Run as a non-root user (security: never run the web tier as root) and own the
# data dir (the SQLite WAL file lives here on the hookbox_data named volume).
RUN useradd --create-home --uid 10001 hookbox \
    && mkdir -p /app/data \
    && chown -R hookbox:hookbox /app
USER hookbox

EXPOSE 8000

# Liveness/readiness via the /healthz endpoint (§5.2 #19). python is always
# present in the slim image (curl is not), so probe with urllib — no extra apt.
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
    CMD python -c "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8000/healthz', timeout=4).status==200 else 1)"

# Async ASGI server (uvloop + httptools via uvicorn[standard]).
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]

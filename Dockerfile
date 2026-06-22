# HookBox — single Rust/Axum binary over WAL SQLite, serving the API, the P1
# mock interceptor, and the Vite/React SPA from dist/. No Redis, no Postgres.
#
# Multi-stage: (1) build the SPA with pnpm -> dist/, (2) build the Rust binaries
# --release, (3) copy both into a slim runtime image.

# --- stage 1: frontend (Vite -> dist/) ---------------------------------------
FROM node:22-slim AS frontend
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm build

# --- stage 2: backend (cargo --release) --------------------------------------
FROM rust:1-slim AS backend
WORKDIR /build
COPY backend ./backend
RUN cd backend && cargo build --release --bins

# --- stage 3: runtime --------------------------------------------------------
FROM debian:bookworm-slim AS runtime
ENV APP_PORT=8080 \
    APP_BIND_HOST=0.0.0.0 \
    DATABASE_PATH=/app/data/app.db \
    STATIC_DIR=/app/dist
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 hookbox \
    && mkdir -p /app/data \
    && chown -R hookbox:hookbox /app

COPY --from=backend /build/backend/target/release/hookbox /usr/local/bin/hookbox
COPY --from=backend /build/backend/target/release/seed /usr/local/bin/seed
COPY --from=backend /build/backend/target/release/tunnel /usr/local/bin/tunnel
COPY --from=frontend /app/dist /app/dist

USER hookbox
EXPOSE 8080

# Liveness via the /healthz endpoint (§5.2 #19).
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
    CMD ["/bin/sh", "-c", "wget -q -O- http://127.0.0.1:8080/healthz >/dev/null 2>&1 || exit 1"]

# Seed on first run (idempotent), then serve the single binary.
CMD ["/bin/sh", "-c", "seed || true; exec hookbox"]

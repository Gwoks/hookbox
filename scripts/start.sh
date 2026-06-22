#!/usr/bin/env bash
# HookBox — build the React SPA + Rust backend, then run the single binary.
# The binary serves the P2 /api management plane, the P1 mock interceptor, and
# the P3 SPA (from dist/) + live feed + tunnel WS over one WAL SQLite file, and
# runs the retention sweep in-process. No Redis, no Postgres, no Node at runtime.
set -euo pipefail

cd "$(dirname "$0")/.."

# Load .env if present (export every assignment).
if [ -f .env ]; then
  set -a; . ./.env; set +a
fi

echo "==> Building frontend (Vite -> dist/)"
pnpm install --frozen-lockfile 2>/dev/null || pnpm install
pnpm build

echo "==> Building backend (cargo --release)"
( cd backend && cargo build --release )

# Migrate-on-startup happens inside the binary; seed demo data on first run when
# the DB file doesn't exist yet (idempotent — the seed bin is a no-op otherwise).
DATABASE_PATH="${DATABASE_PATH:-data/app.db}"
mkdir -p "$(dirname "$DATABASE_PATH")"
if [ "${SEED:-0}" = "1" ] || [ ! -f "$DATABASE_PATH" ]; then
  echo "==> Seeding demo data ($DATABASE_PATH)"
  ./backend/target/release/seed || true
fi

echo "==> Starting hookbox on :${APP_PORT:-8080}"
exec ./backend/target/release/hookbox

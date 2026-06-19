#!/usr/bin/env bash
# Reset the HookBox durable store for a fresh start (dev convenience, arch §7).
#
# Removes the SQLite database (config, rules, request history) so `init_db`
# recreates the §5.8 schema on next start. It does NOT touch Redis (live state /
# Auto-CRUD collections) — use `docker compose down -v` to also wipe Redis.
#
# Usage:
#   ./reset_db.sh                          # wipe ./data/hookbox.db (default)
#   DATABASE_PATH=/path/db.sqlite ./reset_db.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_PATH="${DATABASE_PATH:-$SCRIPT_DIR/data/hookbox.db}"

echo "HookBox DB reset — target: $DB_PATH"

# Remove the DB plus the WAL/SHM sidecar files created by WAL journaling.
for f in "$DB_PATH" "$DB_PATH-wal" "$DB_PATH-shm"; do
    if [ -f "$f" ]; then
        rm -f "$f"
        echo "  removed $f"
    fi
done

echo "Done. The schema is recreated automatically on next app start."

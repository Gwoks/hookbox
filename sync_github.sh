#!/bin/bash
# Sync hookbox repo to GitHub

LOCK_FILE="/tmp/openclaw_locks/cron_sync_hookbox.lock"

mkdir -p /tmp/openclaw_locks

exec 200>"$LOCK_FILE"
flock -n 200 || { echo "Sync: Another instance running"; exit 1; }

cd /home/ubuntu/hookbox

# Pull latest first (in case of external changes)
git pull origin main --quiet 2>/dev/null

# Add all changes
git add -A

# Commit only if there are changes
if git diff --staged --quiet; then
    echo "[$(date)] No changes to sync"
else
    git commit -m "Auto-sync $(date '+%Y-%m-%d %H:%M')"
    git push origin main
    echo "[$(date)] Synced to GitHub"
fi

rm -f "$LOCK_FILE"

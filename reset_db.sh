#!/bin/bash
# Reset WebhookCatch database - removes all endpoints, requests, keeps mock rules structure

LOCK_FILE="/tmp/openclaw_locks/cron_reset_db.lock"
DATA_DIR="/home/ubuntu/hookbox/data"
DB_PATH="$DATA_DIR/webhookcatch.db"

# Ensure lock directory exists
mkdir -p /tmp/openclaw_locks

# Acquire lock
exec 200>"$LOCK_FILE"
flock -n 200 || { echo "Reset DB: Another instance running"; exit 1; }

echo "[$(date)] Resetting WebhookCatch database..."

# Stop the service first
pkill -f 'python.*app.main' || true
sleep 2

# Remove database
rm -f "$DB_PATH"
echo "[$(date)] Database removed"

# Restart service (will recreate DB on startup)
cd /home/ubuntu/hookbox
nohup /home/linuxbrew/.linuxbrew/bin/python3.14 -m app.main > /home/ubuntu/hookbox/webhookcatch.log 2>&1 &
sleep 3

# Verify it's running
if curl -s http://127.0.0.1:5000/ > /dev/null 2>&1; then
    echo "[$(date)] Database reset complete, service restarted"
else
    echo "[$(date)] WARNING: Service may not be running properly"
fi

# Release lock
rm -f "$LOCK_FILE"

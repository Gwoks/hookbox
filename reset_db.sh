#!/bin/bash
# Reset HookBox database - removes all data for fresh start

LOCK_FILE="/tmp/openclaw_locks/cron_reset_hookbox.lock"
DATA_DIR="/home/ubuntu/hookbox/data"
DB_PATH="$DATA_DIR/webhookcatch.db"
LOG_FILE="/home/ubuntu/hookbox/reset.log"

mkdir -p /tmp/openclaw_locks

exec 200>"$LOCK_FILE"
flock -n 200 || { echo "[$(date)] Reset hookbox: Another instance running"; exit 1; }

echo "[$(date)] Starting HookBox database reset..." >> "$LOG_FILE"

# Stop the service
pkill -f 'python.*app.main' || true
echo "[$(date)] Service stopped" >> "$LOG_FILE"
sleep 2

# Delete database
rm -f "$DB_PATH"
echo "[$(date)] Database deleted: $DB_PATH" >> "$LOG_FILE"

# Clear logs
rm -f /tmp/hookbox.log
echo "[$(date)] Log file cleared" >> "$LOG_FILE"

# Restart service
cd /home/ubuntu/hookbox
nohup /home/linuxbrew/.linuxbrew/bin/python3.14 -m app.main > /tmp/hookbox.log 2>&1 &
sleep 3

# Verify it's running
if curl -s http://127.0.0.1:5000/ > /dev/null 2>&1; then
    echo "[$(date)] Service restarted successfully" >> "$LOG_FILE"
else
    echo "[$(date)] WARNING: Service may not be running properly" >> "$LOG_FILE"
fi

rm -f "$LOCK_FILE"
echo "[$(date)] Reset complete" >> "$LOG_FILE"

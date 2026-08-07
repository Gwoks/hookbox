#!/bin/bash
# =============================================================================
# Deploy script for hookbox.lgtm.my.id — manual fallback for the
# "Deploy to VPS" GitHub Actions workflow (.github/workflows/deploy.yml).
# Mirrors that workflow's steps 1:1 so a manual run can't drift from CI: both
# render+install the same systemd unit and nginx config, reload nginx, and
# verify the public health endpoint before declaring success.
# =============================================================================
set -e

APP_DIR="$(pwd)"
SERVICE_NAME="hookbox"
DOMAIN="hookbox.lgtm.my.id"
APP_PORT="8080"
MOCK_DOMAIN="localhost"
PUBLIC_BASE_URL="https://$DOMAIN"

echo "==> Pulling latest code"
git fetch origin main
git reset --hard origin/main

echo "==> Installing dependencies and building frontend"
pnpm install --frozen-lockfile 2>/dev/null || pnpm install
pnpm build

echo "==> Building Rust backend"
. "$HOME/.cargo/env"
cd backend
cargo build --release
cd ..

echo "==> Fixing permissions"
sudo chmod 755 /home/ubuntu

echo "==> Rendering and installing systemd service"
mkdir -p "$APP_DIR/data"
chown -R ubuntu:ubuntu "$APP_DIR/data"
sed "s|{{APP_DIR}}|$APP_DIR|g; s|{{APP_PORT}}|$APP_PORT|g; s|{{MOCK_DOMAIN}}|$MOCK_DOMAIN|g; s|{{PUBLIC_BASE_URL}}|$PUBLIC_BASE_URL|g; s|{{DOMAIN}}|$DOMAIN|g" \
  "$APP_DIR/deploy/hookbox.service" | sudo tee /etc/systemd/system/$SERVICE_NAME.service > /dev/null

echo "==> Rendering and installing nginx config"
sed "s|{{APP_DIR}}|$APP_DIR|g; s|{{APP_PORT}}|$APP_PORT|g; s|{{DOMAIN}}|$DOMAIN|g" \
  "$APP_DIR/deploy/nginx.conf" | sudo tee /etc/nginx/sites-available/$SERVICE_NAME > /dev/null
sudo ln -sf /etc/nginx/sites-available/$SERVICE_NAME /etc/nginx/sites-enabled/$SERVICE_NAME
sudo rm -f /etc/nginx/sites-enabled/default

echo "==> Reloading nginx and restarting hookbox"
sudo nginx -t
sudo systemctl reload nginx
sudo systemctl daemon-reload
sudo systemctl enable --now "$SERVICE_NAME"
sudo systemctl restart "$SERVICE_NAME"

echo "==> Verifying health"
sleep 2
curl -fsS "https://$DOMAIN/healthz" > /dev/null

echo "hookbox deployed successfully"

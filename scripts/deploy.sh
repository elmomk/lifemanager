#!/usr/bin/env bash
set -euo pipefail

echo "==> Building Tailwind CSS..."
npx @tailwindcss/cli -i ./input.css -o ./assets/main.css --minify

echo "==> Building Dioxus app (release)..."
dx build --release --platform web

echo "==> Building Docker image..."
docker compose build app

echo "==> Deploying..."
docker compose up -d

echo "==> Waiting for startup..."
sleep 2

echo "==> Checking health..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" https://lifemanager.tail6c1af7.ts.net/ 2>/dev/null || echo "000")
if [ "$STATUS" = "200" ]; then
    echo "==> Deploy successful! (HTTP $STATUS)"
else
    echo "==> WARNING: HTTP $STATUS — checking logs..."
    docker compose logs app --tail 10
fi

---
name: deploy
description: Build and deploy the Life Manager app via Docker Compose
allowed-tools: Bash, Read, Glob
---

Deploy the Life Manager application.

## Steps

1. Build the Docker image:
   ```
   docker compose build app
   ```

2. Restart the services:
   ```
   docker compose up -d
   ```

3. Verify deployment:
   ```
   docker compose logs app --tail 20
   docker compose exec tailscale tailscale serve status
   curl -s -o /dev/null -w "%{http_code}" https://lifemanager.tail6c1af7.ts.net/
   ```

4. Report the result: whether all server functions registered, Tailscale serve is active, and the app returns 200.

If any step fails, show the error logs and suggest a fix.

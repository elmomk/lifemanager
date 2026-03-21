---
name: deploy
description: Build and deploy the Life Manager app via Docker Compose
allowed-tools: Bash, Read, Glob
---

Deploy the Life Manager application.

## Steps

1. Run the deploy script:
   ```
   ./scripts/deploy.sh
   ```

2. If the script succeeds, verify with:
   ```
   docker compose logs app --tail 20
   curl -s -o /dev/null -w "%{http_code}" https://lifemanager.tail6c1af7.ts.net/
   ```

3. Report the result: whether all server functions registered and the app returns 200.

If any step fails, show the error logs and suggest a fix.

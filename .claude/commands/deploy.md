Deploy Life Manager: build locally, package in Docker, deploy.

Run: `./scripts/deploy.sh`

If the deploy script fails at any step, diagnose the error and fix it.
After a successful deploy, verify with: `docker compose logs app --tail 10`

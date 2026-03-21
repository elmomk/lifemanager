# Life Manager PWA

## Project Overview
Mobile-first PWA with five modules: To-Dos, Groceries, Shopee Pick-ups, Watchlist, and Cycle Tracker.

**Stack:** Rust + Dioxus 0.7 (fullstack, Wasm), Tailwind CSS v4, SQLite (server).

## Architecture
- `src/models/` — Data structs shared between client and server
- `src/pages/` — Dioxus page components (one per module)
- `src/components/` — Reusable UI components (swipe, tab bar, icons, quick-add chips, error banner, checklist page, OCR)
- `src/api/` — Server functions (`#[server]`) for CRUD operations
- `src/server/` — Server-only code (SQLite DB, auth) — gated behind `#[cfg(not(target_arch = "wasm32"))]`
- `src/route.rs` — Dioxus Router config
- `assets/` — Static assets (CSS, manifest, icons, SW)
- `scripts/` — Shell scripts for common tasks

## Build & Dev Commands
- **Dev server:** `./scripts/dev.sh` (Tailwind watch + Dioxus dev server on port 8080)
- **Production build:** `./scripts/build.sh` (Tailwind + Dioxus release build)
- **Type check:** `./scripts/check.sh` (cargo check)
- **Deploy:** `./scripts/deploy.sh` (build + Docker + deploy + health check)
- **Screenshots:** `./scripts/screenshot.sh` (Playwright mobile screenshots of all pages)

## Deployment
- Docker Compose with Tailscale sidecar container
- Dockerfile copies locally-built binary (no Rust build in Docker — uses `debian:trixie-slim`)
- App at `https://lifemanager.tail6c1af7.ts.net/`
- SQLite DB in Docker volume at `/app/data/life_manager.db`
- `DATABASE_PATH` env var configures DB location (defaults to `life_manager.db`)
- Tesseract OCR with `chi_tra+eng` for Shopee screenshot parsing

## Code Conventions
- Dioxus 0.7 RSX syntax (no `cx` parameter, uses `rsx!{}` macro with `Element` return)
- Server functions use `#[server(headers: axum::http::HeaderMap)]` — `headers` is injected by macro
- Auth: `auth::user_from_headers(&headers)` returns `Result<String>` (shared "default" user, but requires Tailscale header in prod)
- Attribution: `auth::display_name_from_headers(&headers)` returns actual Tailscale login for tracking who completed items
- Models derive `Clone, Debug, PartialEq, Serialize, Deserialize`
- Tailwind CSS v4 (no tailwind.config.js — uses `@import` and `@theme` in `input.css`)
- Cyberpunk design language: neon accents (cyan/green/magenta/orange/purple), dark backgrounds, JetBrains Mono font, glow effects, scanline overlay
- Custom theme colors: `cyber-black`, `cyber-dark`, `cyber-card`, `cyber-border`, `neon-cyan`, `neon-green`, `neon-magenta`, `neon-orange`, `neon-purple`, `neon-pink`, `neon-yellow`
- Data loading: `use_signal` + `use_effect` with explicit `reload()` closure (direct re-fetch after mutations)
- Error feedback: `ErrorBanner` component with dismissible `error_msg` signal on every page
- Dynamic quick-add chips: stored in `default_items` table, seeded from hardcoded defaults on first use
- Swipe right = complete (second swipe on completed = add to defaults), swipe left = delete

## Important Notes
- **Never use `base_path`** in Dioxus.toml — it breaks fullstack server function calls (Dioxus 0.7 bug)
- PWA files (`sw.js`, `sw-register.js`, `manifest.json`, `icons/`) must be copied into `public/` via Dockerfile since Dioxus doesn't include them in build output
- `target/`, `node_modules/`, `.env` are not committed
- Server-only deps gated with `cfg(not(target_arch = "wasm32"))`
- Dioxus fullstack means both `web` and `server` features are default-enabled
- JS-to-Rust communication in Dioxus 0.7: use `dioxus.send()` in JS + `eval.recv::<T>().await` in Rust (not Promise return)

## Skills & Commands
- `/deploy` — Build and deploy via Docker Compose
- `/build` — Build for production
- `/dev` — Start dev environment
- `/check` — Run type checking
- `/tailwind` — Compile Tailwind CSS
- `/db-migrate` — Add tables/columns to SQLite schema
- `/add-module` — Scaffold a new module (model + API + page + route)

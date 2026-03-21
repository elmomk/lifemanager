# Life Manager PWA

## Project Overview
Mobile-first PWA with five modules: To-Dos, Groceries, Shopee Pick-ups, Watchlist, and Cycle Tracker.

**Stack:** Rust + Dioxus 0.7 (fullstack, Wasm), Tailwind CSS v4, SQLite (server).

## Architecture
- `src/models/` — Data structs shared between client and server
- `src/pages/` — Dioxus page components (one per module)
- `src/components/` — Reusable UI components (swipe, tab bar, icons, quick-add chips)
- `src/api/` — Server functions (`#[server]`) for CRUD operations
- `src/server/` — Server-only code (SQLite DB, auth) — gated behind `#[cfg(not(target_arch = "wasm32"))]`
- `src/route.rs` — Dioxus Router config
- `assets/` — Static assets (CSS, manifest, icons, SW)

## Build & Dev Commands
- **Dev server:** `dx serve` (runs on port 8080)
- **Tailwind watch:** `npm run tailwind` (compiles `input.css` → `assets/main.css`)
- **Production build:** `dx build --release --platform web`
- **Type check:** `cargo check`
- **Deploy:** `docker compose build app && docker compose up -d`

## Deployment
- Docker Compose with Tailscale sidecar container
- App at `https://lifemanager.tail6c1af7.ts.net/`
- SQLite DB in Docker volume at `/app/data/life_manager.db`
- `DATABASE_PATH` env var configures DB location (defaults to `life_manager.db`)

## Code Conventions
- Dioxus 0.7 RSX syntax (no `cx` parameter, uses `rsx!{}` macro with `Element` return)
- Server functions use `#[server(headers: axum::http::HeaderMap)]` — `headers` is injected by macro
- Models derive `Clone, Debug, PartialEq, Serialize, Deserialize`
- Tailwind CSS v4 (no tailwind.config.js — uses `@import` and `@theme` in `input.css`)
- Glassmorphism design language: `backdrop-blur`, `rounded-2xl`/`rounded-3xl`, soft shadows
- System-aware dark/light theming via `dark:` variant classes

## Important Notes
- **Never use `base_path`** in Dioxus.toml — it breaks fullstack server function calls (Dioxus 0.7 bug)
- `target/`, `node_modules/`, `.env` are not committed
- Server-only deps gated with `cfg(not(target_arch = "wasm32"))`
- Dioxus fullstack means both `web` and `server` features are default-enabled

## Skills & Agents
- `/deploy` — Build and deploy via Docker Compose
- `/tailwind` — Compile Tailwind CSS
- `/db-migrate` — Add tables/columns to SQLite schema
- `/add-module` — Scaffold a new module (model + API + page + route)
- `dioxus-dev` agent — Dioxus 0.7 fullstack specialist
- `docker-ops` agent — Docker/Tailscale operations

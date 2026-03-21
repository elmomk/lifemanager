---
name: add-module
description: Scaffold a new Life Manager module (model, API, page, route)
allowed-tools: Read, Write, Edit, Bash, Grep, Glob
---

Scaffold a new module for the Life Manager app. Pass the module name as an argument: `/add-module <name>`

## Architecture

Each module consists of 4 files:
1. **Model** (`src/models/<name>.rs`) — Data struct with `Clone, Debug, PartialEq, Serialize, Deserialize`
2. **API** (`src/api/<name>.rs`) — Server functions: `list_<name>`, `add_<name>`, `toggle_<name>` (if applicable), `delete_<name>`
3. **Page** (`src/pages/<name>.rs`) — Dioxus component using `SwipeItem` for list items
4. **DB table** in `src/server/db.rs` — `CREATE TABLE IF NOT EXISTS`

## Steps

1. Read existing modules for patterns:
   - `src/models/checklist_item.rs` for model pattern
   - `src/api/checklist.rs` for server function pattern (uses `#[server(headers: axum::http::HeaderMap)]`)
   - `src/pages/todos.rs` for page component pattern
   - `src/server/db.rs` for schema pattern

2. Create all 4 files following the exact same patterns

3. Register the module:
   - Add `pub mod <name>;` to `src/models/mod.rs` and `pub use <name>::*;`
   - Add `pub mod <name>;` to `src/api/mod.rs`
   - Add `pub mod <name>;` to `src/pages/mod.rs`
   - Add route variant to `src/route.rs`
   - Add tab icon to `src/components/tab_bar.rs`

4. Add table to `src/server/db.rs` `execute_batch`

5. Run `cargo check` to verify

## Key patterns
- Server functions: `#[server(headers: axum::http::HeaderMap)]` with `auth::user_from_headers(&headers)`
- Page data loading: `use_signal` + `use_effect` with `refresh` counter
- Swipe right = toggle done, swipe left = delete
- UUIDs and timestamps generated server-side

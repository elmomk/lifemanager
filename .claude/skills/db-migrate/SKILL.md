---
name: db-migrate
description: Add a new table or column to the SQLite database schema
allowed-tools: Read, Edit, Bash, Grep
---

Add a database migration to `src/server/db.rs`.

## Context
- All tables live in `src/server/db.rs` in the `init()` function's `execute_batch` call
- Every table must have `user_id TEXT NOT NULL` for multi-user scoping
- Use `CREATE TABLE IF NOT EXISTS` (idempotent migrations)
- Schema: `id TEXT PRIMARY KEY, user_id TEXT NOT NULL, ...fields..., created_at REAL NOT NULL`

## Steps
1. Read `src/server/db.rs` to see existing schema
2. Add the new `CREATE TABLE IF NOT EXISTS` statement to the `execute_batch` call
3. If this is for an existing module, also update the corresponding model in `src/models/` and API in `src/api/`
4. Run `cargo check` to verify compilation

Pass the table name and fields as arguments: `/db-migrate tablename field1:type field2:type`

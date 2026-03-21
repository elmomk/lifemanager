Add a new page/module to Life Manager.

Arguments: $ARGUMENTS (the module name and description)

Steps:
1. Read the existing page structure by examining `src/pages/mod.rs` and one existing page like `src/pages/todos.rs` for patterns
2. Read `src/route.rs` for routing patterns
3. Read `src/models/mod.rs` and an existing model for data model patterns
4. Read `src/api/mod.rs` and an existing API module for server function patterns
5. Create the new model in `src/models/`
6. Create the new API module in `src/api/`
7. Create the new page component in `src/pages/`
8. Register the model, API, page, and route in their respective `mod.rs` files and `route.rs`
9. Add the SQLite table migration in `src/server/db.rs`
10. Run `cargo check` to verify everything compiles

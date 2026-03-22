use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::OnceLock;

pub type DbPool = Pool<SqliteConnectionManager>;

static POOL: OnceLock<DbPool> = OnceLock::new();

pub fn init() {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "life_manager.db".to_string());
    let manager = SqliteConnectionManager::file(&db_path)
        .with_init(|conn| {
            conn.pragma_update(None, "busy_timeout", 5000)?;
            Ok(())
        });
    let pool = Pool::new(manager).expect("Failed to create DB pool");

    let conn = pool.get().expect("Failed to get DB connection");

    // WAL mode: crash-safe, better concurrent read/write performance
    conn.pragma_update(None, "journal_mode", "WAL")
        .expect("Failed to set WAL mode");
    // NORMAL sync is safe with WAL (full durability except on OS crash + power loss)
    conn.pragma_update(None, "synchronous", "NORMAL")
        .expect("Failed to set synchronous mode");
    // Wait up to 5s for locks instead of failing immediately
    conn.pragma_update(None, "busy_timeout", 5000)
        .expect("Failed to set busy_timeout");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS checklist_items (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            text TEXT NOT NULL,
            date TEXT,
            done INTEGER NOT NULL DEFAULT 0,
            category TEXT NOT NULL,
            created_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS shopee_packages (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            title TEXT NOT NULL,
            store TEXT,
            code TEXT,
            picked_up INTEGER NOT NULL DEFAULT 0,
            created_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS watch_items (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            text TEXT NOT NULL,
            media_type TEXT NOT NULL,
            done INTEGER NOT NULL DEFAULT 0,
            created_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS cycles (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            symptoms TEXT NOT NULL DEFAULT '[]'
        );
        CREATE TABLE IF NOT EXISTS default_items (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            category TEXT NOT NULL,
            text TEXT NOT NULL,
            created_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS migrations (
            name TEXT PRIMARY KEY
        );
        CREATE INDEX IF NOT EXISTS idx_checklist_user ON checklist_items(user_id, category, done);
        CREATE INDEX IF NOT EXISTS idx_shopee_user ON shopee_packages(user_id, picked_up);
        CREATE INDEX IF NOT EXISTS idx_watch_user ON watch_items(user_id, done);
        CREATE INDEX IF NOT EXISTS idx_cycles_user ON cycles(user_id);
        CREATE INDEX IF NOT EXISTS idx_defaults_user ON default_items(user_id, category);"
    )
    .expect("Failed to run migrations");

    // Add completed_by column (safe to run multiple times — "duplicate column" errors are expected)
    for sql in [
        "ALTER TABLE checklist_items ADD COLUMN completed_by TEXT",
        "ALTER TABLE shopee_packages ADD COLUMN completed_by TEXT",
        "ALTER TABLE watch_items ADD COLUMN completed_by TEXT",
    ] {
        if let Err(e) = conn.execute_batch(sql) {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                eprintln!("WARNING: migration failed: {msg}");
            }
        }
    }

    // Add google_event_id column for Calendar sync
    for sql in [
        "ALTER TABLE checklist_items ADD COLUMN google_event_id TEXT",
    ] {
        if let Err(e) = conn.execute_batch(sql) {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                eprintln!("WARNING: migration failed: {msg}");
            }
        }
    }

    // One-time migration: consolidate all users to 'default'
    run_once(&conn, "consolidate_users",
        "UPDATE checklist_items SET user_id = 'default' WHERE user_id != 'default';
         UPDATE shopee_packages SET user_id = 'default' WHERE user_id != 'default';
         UPDATE watch_items SET user_id = 'default' WHERE user_id != 'default';
         UPDATE cycles SET user_id = 'default' WHERE user_id != 'default';"
    );

    POOL.set(pool).expect("DB pool already initialized");
}

fn run_once(conn: &rusqlite::Connection, name: &str, sql: &str) {
    let already_run: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM migrations WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !already_run {
        let _ = conn.execute_batch(sql);
        let _ = conn.execute(
            "INSERT INTO migrations (name) VALUES (?1)",
            rusqlite::params![name],
        );
    }
}

pub fn pool() -> &'static DbPool {
    POOL.get().expect("DB pool not initialized")
}

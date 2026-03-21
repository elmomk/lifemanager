use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::OnceLock;

pub type DbPool = Pool<SqliteConnectionManager>;

static POOL: OnceLock<DbPool> = OnceLock::new();

pub fn init() {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "life_manager.db".to_string());
    let manager = SqliteConnectionManager::file(db_path);
    let pool = Pool::new(manager).expect("Failed to create DB pool");

    let conn = pool.get().expect("Failed to get DB connection");
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
        );"
    )
    .expect("Failed to run migrations");

    POOL.set(pool).expect("DB pool already initialized");
}

pub fn pool() -> &'static DbPool {
    POOL.get().expect("DB pool not initialized")
}

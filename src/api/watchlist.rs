use dioxus::prelude::*;

use crate::models::{MediaType, WatchItem};

#[server(headers: axum::http::HeaderMap)]
pub async fn list_watchlist() -> Result<Vec<WatchItem>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, text, media_type, done, created_at
             FROM watch_items
             WHERE user_id = ?1
             ORDER BY done ASC, created_at DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id], |row| {
            let mt_str: String = row.get(2)?;
            Ok(WatchItem {
                id: row.get(0)?,
                text: row.get(1)?,
                media_type: match mt_str.as_str() {
                    "Series" => MediaType::Series,
                    "Anime" => MediaType::Anime,
                    "Cartoon" => MediaType::Cartoon,
                    _ => MediaType::Movie,
                },
                done: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_watchlist(text: String, media_type: MediaType) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let mt_str = media_type.label();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO watch_items (id, user_id, text, media_type, done, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5)",
        rusqlite::params![id, user_id, text, mt_str, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn toggle_watchlist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE watch_items SET done = 1 - done WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_watchlist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM watch_items WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

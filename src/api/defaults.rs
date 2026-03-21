use dioxus::prelude::*;

use crate::models::ItemCategory;

#[server(headers: axum::http::HeaderMap)]
pub async fn list_defaults(category: ItemCategory) -> Result<Vec<String>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let cat_str = category.to_string();
    let mut stmt = conn
        .prepare(
            "SELECT text FROM default_items
             WHERE user_id = ?1 AND category = ?2
             ORDER BY created_at ASC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id, cat_str], |row| row.get(0))
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_default(text: String, category: ItemCategory) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::short(&text, "chip text")?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let cat_str = category.to_string();

    // Don't add duplicates
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM default_items WHERE user_id = ?1 AND category = ?2 AND text = ?3",
            rusqlite::params![user_id, cat_str, text],
            |row| row.get(0),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if exists {
        return Ok(());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO default_items (id, user_id, category, text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, user_id, cat_str, text, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_default(text: String, category: ItemCategory) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let cat_str = category.to_string();

    conn.execute(
        "DELETE FROM default_items WHERE user_id = ?1 AND category = ?2 AND text = ?3",
        rusqlite::params![user_id, cat_str, text],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

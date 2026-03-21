use dioxus::prelude::*;

use crate::models::{ChecklistItem, ItemCategory};

#[server(headers: axum::http::HeaderMap)]
pub async fn list_checklist(category: ItemCategory) -> Result<Vec<ChecklistItem>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let cat_str = match category {
        ItemCategory::Todo => "Todo",
        ItemCategory::Grocery => "Grocery",
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, text, date, done, category, created_at
             FROM checklist_items
             WHERE user_id = ?1 AND category = ?2
             ORDER BY done ASC, created_at DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id, cat_str], |row| {
            let cat_s: String = row.get(4)?;
            Ok(ChecklistItem {
                id: row.get(0)?,
                text: row.get(1)?,
                date: row
                    .get::<_, Option<String>>(2)?
                    .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                done: row.get(3)?,
                category: if cat_s == "Grocery" {
                    ItemCategory::Grocery
                } else {
                    ItemCategory::Todo
                },
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_checklist(
    text: String,
    category: ItemCategory,
    date: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let cat_str = match category {
        ItemCategory::Todo => "Todo",
        ItemCategory::Grocery => "Grocery",
    };
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO checklist_items (id, user_id, text, date, done, category, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
        rusqlite::params![id, user_id, text, date, cat_str, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn toggle_checklist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE checklist_items SET done = 1 - done WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_checklist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM checklist_items WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

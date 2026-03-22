use dioxus::prelude::*;

use crate::models::{ChecklistItem, ItemCategory};

#[server(headers: axum::http::HeaderMap)]
pub async fn list_checklist(category: ItemCategory) -> Result<Vec<ChecklistItem>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let cat_str = category.to_string();

    let mut stmt = conn
        .prepare(
            "SELECT id, text, date, done, category, created_at, completed_by
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
                category: cat_s.parse().unwrap_or(ItemCategory::Todo),
                created_at: row.get(5)?,
                completed_by: row.get(6)?,
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
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::text(&text, "text")?;
    if let Some(ref d) = date {
        validate::date(d)?;
    }
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let cat_str = category.to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO checklist_items (id, user_id, text, date, done, category, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
        rusqlite::params![id, user_id, text, date, cat_str, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fire-and-forget Google Calendar sync
    {
        if let Some(ref d) = date {
            let id2 = id.clone();
            let text2 = text.clone();
            let d2 = d.clone();
            tokio::spawn(async move {
                crate::server::google::sync_item(&id2, &text2, Some(&d2), false, None).await;
            });
        }
    }

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn toggle_checklist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let display_name = auth::display_name_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE checklist_items SET done = 1 - done,
         completed_by = CASE WHEN done = 0 THEN ?3 ELSE NULL END
         WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id, display_name],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fire-and-forget Google Calendar sync
    {
        let id2 = id.clone();
        tokio::spawn(async move {
            let conn = crate::server::db::pool().get().ok();
            if let Some(conn) = conn {
                let item: Option<(String, Option<String>, bool, Option<String>)> = conn
                    .query_row(
                        "SELECT text, date, done, google_event_id FROM checklist_items WHERE id = ?1",
                        rusqlite::params![id2],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .ok();
                if let Some((text, date, done, event_id)) = item {
                    crate::server::google::sync_item(
                        &id2,
                        &text,
                        date.as_deref(),
                        done,
                        event_id.as_deref(),
                    ).await;
                }
            }
        });
    }

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_checklist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Read google_event_id before deleting
    let event_id: Option<String> = conn
        .query_row(
            "SELECT google_event_id FROM checklist_items WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    conn.execute(
        "DELETE FROM checklist_items WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fire-and-forget: delete Calendar event
    if let Some(eid) = event_id {
        let eid2 = eid.clone();
        tokio::spawn(async move {
            crate::server::google::sync_item("", "", None, true, Some(&eid2)).await;
        });
    }

    Ok(())
}

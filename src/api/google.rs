use dioxus::prelude::*;

#[server(headers: axum::http::HeaderMap)]
pub async fn google_calendar_status() -> Result<bool, ServerFnError> {
    use crate::server::auth;

    auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    Ok(crate::server::google::is_configured())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn google_full_sync() -> Result<String, ServerFnError> {
    use crate::server::{auth, db, google};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;

    if !google::is_configured() {
        return Err(ServerFnError::new("Google Calendar not configured"));
    }

    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Get all undone checklist items with dates (Todos and Groceries)
    let mut stmt = conn
        .prepare(
            "SELECT id, text, date, google_event_id FROM checklist_items
             WHERE user_id = ?1 AND done = 0 AND date IS NOT NULL"
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items: Vec<(String, String, String, Option<String>)> = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let total = items.len();
    let mut synced = 0;
    let mut errors = 0;

    for (id, text, date, event_id) in items {
        google::sync_item(&id, &text, Some(&date), false, event_id.as_deref()).await;
        // Check if it got an event_id now
        let conn2 = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;
        let has_event: bool = conn2
            .query_row(
                "SELECT google_event_id IS NOT NULL FROM checklist_items WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if has_event { synced += 1; } else { errors += 1; }
    }

    // Also clean up: delete events for completed checklist items that still have event IDs
    let mut stmt2 = conn
        .prepare(
            "SELECT id, google_event_id FROM checklist_items
             WHERE user_id = ?1 AND done = 1 AND google_event_id IS NOT NULL"
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let done_items: Vec<(String, String)> = stmt2
        .query_map(rusqlite::params![user_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for (id, event_id) in done_items {
        google::sync_item(&id, "", None, true, Some(&event_id)).await;
    }

    // Also sync Shopee packages with due dates
    let mut stmt3 = conn
        .prepare(
            "SELECT id, title, due_date, google_event_id FROM shopee_packages
             WHERE user_id = ?1 AND picked_up = 0 AND due_date IS NOT NULL AND date_is_estimate = 0"
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let shopee_items: Vec<(String, String, String, Option<String>)> = stmt3
        .query_map(rusqlite::params![user_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let shopee_total = shopee_items.len();
    let mut shopee_synced = 0;

    for (id, title, date, event_id) in shopee_items {
        google::sync_item(&id, &title, Some(&date), false, event_id.as_deref()).await;
        let conn2 = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;
        let has_event: bool = conn2
            .query_row(
                "SELECT google_event_id IS NOT NULL FROM shopee_packages WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if has_event { shopee_synced += 1; }
    }

    // Clean up picked-up shopee packages with events
    let mut stmt4 = conn
        .prepare(
            "SELECT id, google_event_id FROM shopee_packages
             WHERE user_id = ?1 AND picked_up = 1 AND google_event_id IS NOT NULL"
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let picked_up_items: Vec<(String, String)> = stmt4
        .query_map(rusqlite::params![user_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for (id, event_id) in picked_up_items {
        google::sync_item(&id, "", None, true, Some(&event_id)).await;
    }

    Ok(format!("Synced {synced}/{total} items, {shopee_synced}/{shopee_total} packages ({errors} errors)"))
}

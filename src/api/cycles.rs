use dioxus::prelude::*;

use crate::models::Cycle;

#[server(headers: axum::http::HeaderMap)]
pub async fn list_cycles() -> Result<Vec<Cycle>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, start_date, end_date, symptoms
             FROM cycles
             WHERE user_id = ?1
             ORDER BY start_date DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id], |row| {
            let symptoms_json: String = row.get(3)?;
            Ok(Cycle {
                id: row.get(0)?,
                start_date: row
                    .get::<_, String>(1)?
                    .parse()
                    .unwrap_or(chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
                end_date: row
                    .get::<_, Option<String>>(2)?
                    .and_then(|d| d.parse().ok()),
                symptoms: serde_json::from_str(&symptoms_json).unwrap_or_default(),
            })
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_cycle(
    start_date: String,
    end_date: Option<String>,
    symptoms: Vec<String>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let symptoms_json =
        serde_json::to_string(&symptoms).map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "INSERT INTO cycles (id, user_id, start_date, end_date, symptoms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, user_id, start_date, end_date, symptoms_json],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_cycle(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM cycles WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

use dioxus::prelude::*;

use crate::models::Cycle;

#[server(headers: axum::http::HeaderMap)]
pub async fn list_cycles() -> Result<Vec<Cycle>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
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
            let start_str: String = row.get(1)?;
            let symptoms_json: String = row.get(3)?;
            Ok((row.get::<_, String>(0)?, start_str, row.get::<_, Option<String>>(2)?, symptoms_json))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut cycles = Vec::new();
    for (id, start_str, end_str, symptoms_json) in items {
        let start_date = start_str.parse::<chrono::NaiveDate>()
            .map_err(|_| ServerFnError::new("Invalid date in database".to_string()))?;
        let end_date = match end_str {
            Some(d) => Some(d.parse::<chrono::NaiveDate>()
                .map_err(|_| ServerFnError::new("Invalid date in database".to_string()))?),
            None => None,
        };
        let symptoms: Vec<String> = serde_json::from_str(&symptoms_json).unwrap_or_default();
        cycles.push(Cycle { id, start_date, end_date, symptoms });
    }

    Ok(cycles)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_cycle(
    start_date: String,
    end_date: Option<String>,
    symptoms: Vec<String>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::date(&start_date)?;
    if let Some(ref d) = end_date {
        validate::date(d)?;
    }
    for s in &symptoms {
        validate::short(s, "symptom")?;
    }
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

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM cycles WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

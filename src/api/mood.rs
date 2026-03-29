use dioxus::prelude::*;

use crate::models::{MoodEntry, PhaseInsight, CycleSettings, Cycle, compute_insights};

#[server(headers: axum::http::HeaderMap)]
pub async fn log_mood(
    date: String,
    mood: i32,
    energy: i32,
    libido: i32,
    notes: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::date(&date)?;

    if !(1..=5).contains(&mood) || !(1..=5).contains(&energy) || !(1..=5).contains(&libido) {
        return Err(ServerFnError::new("Values must be 1-5"));
    }
    if let Some(ref n) = notes {
        validate::short(n, "notes")?;
    }

    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    let display_name = auth::display_name_from_headers(&headers);

    conn.execute(
        "INSERT INTO mood_logs (id, user_id, date, mood, energy, libido, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(user_id, date) DO UPDATE SET
             mood = excluded.mood,
             energy = excluded.energy,
             libido = excluded.libido,
             notes = excluded.notes,
             created_at = excluded.created_at",
        rusqlite::params![id, user_id, date, mood, energy, libido, notes, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Notify other users if mood/energy is low
    if mood <= 2 && energy <= 2 {
        crate::server::notify::create_notification(
            &display_name, "is not feeling great", "cycle",
            "low mood & energy today",
        );
    } else if mood <= 2 {
        crate::server::notify::create_notification(
            &display_name, "checked in", "cycle",
            "mood is low today",
        );
    } else if energy <= 2 {
        crate::server::notify::create_notification(
            &display_name, "checked in", "cycle",
            "energy is low today",
        );
    }

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_mood_for_date(date: String) -> Result<Option<MoodEntry>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let result = conn.query_row(
        "SELECT id, date, mood, energy, libido, notes FROM mood_logs
         WHERE user_id = ?1 AND date = ?2",
        rusqlite::params![user_id, date],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        },
    );

    match result {
        Ok((id, date_str, mood, energy, libido, notes)) => {
            let date = date_str.parse::<chrono::NaiveDate>()
                .map_err(|_| ServerFnError::new("Invalid date in database"))?;
            Ok(Some(MoodEntry { id, date, mood, energy, libido, notes }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ServerFnError::new(e.to_string())),
    }
}

#[server(headers: axum::http::HeaderMap)]
pub async fn list_mood_logs() -> Result<Vec<MoodEntry>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, date, mood, energy, libido, notes FROM mood_logs
             WHERE user_id = ?1
             ORDER BY date DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let rows = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut entries = Vec::new();
    for (id, date_str, mood, energy, libido, notes) in rows {
        let date = date_str.parse::<chrono::NaiveDate>()
            .map_err(|_| ServerFnError::new("Invalid date in database"))?;
        entries.push(MoodEntry { id, date, mood, energy, libido, notes });
    }
    Ok(entries)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_mood_insights() -> Result<Vec<PhaseInsight>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Load settings
    let settings = conn.query_row(
        "SELECT average_cycle_length, average_period_duration, on_birth_control
         FROM cycle_settings WHERE user_id = ?1",
        rusqlite::params![user_id],
        |row| {
            Ok(CycleSettings {
                average_cycle_length: row.get(0)?,
                average_period_duration: row.get(1)?,
                on_birth_control: row.get(2)?,
            })
        },
    ).unwrap_or_default();

    // Load cycles
    let mut stmt = conn
        .prepare("SELECT id, start_date, end_date, symptoms FROM cycles WHERE user_id = ?1 ORDER BY start_date DESC")
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let cycles: Vec<Cycle> = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .filter_map(|r| r.ok())
        .filter_map(|(id, start_str, end_str, symptoms_json)| {
            let start_date = start_str.parse::<chrono::NaiveDate>().ok()?;
            let end_date = end_str.and_then(|d| d.parse::<chrono::NaiveDate>().ok());
            let symptoms: Vec<String> = serde_json::from_str(&symptoms_json).unwrap_or_default();
            Some(Cycle { id, start_date, end_date, symptoms })
        })
        .collect();

    // Load mood logs
    let mut stmt = conn
        .prepare("SELECT id, date, mood, energy, libido, notes FROM mood_logs WHERE user_id = ?1 ORDER BY date DESC")
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let mood_logs: Vec<MoodEntry> = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .filter_map(|r| r.ok())
        .filter_map(|(id, date_str, mood, energy, libido, notes)| {
            let date = date_str.parse::<chrono::NaiveDate>().ok()?;
            Some(MoodEntry { id, date, mood, energy, libido, notes })
        })
        .collect();

    Ok(compute_insights(&mood_logs, &cycles, &settings))
}

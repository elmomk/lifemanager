use std::sync::Mutex;

static TOKEN_CACHE: Mutex<Option<(String, u64)>> = Mutex::new(None);

#[derive(serde::Deserialize)]
struct SaKey {
    client_email: String,
    private_key: String,
}

fn load_sa_key() -> Result<SaKey, String> {
    let path = std::env::var("GOOGLE_SA_KEY_FILE")
        .map_err(|_| "GOOGLE_SA_KEY_FILE not set".to_string())?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SA key file: {e}"))?;
    serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse SA key file: {e}"))
}

fn calendar_id() -> String {
    std::env::var("GOOGLE_CALENDAR_ID").unwrap_or_else(|_| "primary".to_string())
}

pub async fn get_token() -> Result<String, String> {
    // Check cache
    {
        let cache = TOKEN_CACHE.lock().unwrap();
        if let Some((ref token, exp)) = *cache {
            let now = chrono::Utc::now().timestamp() as u64;
            if now < exp {
                return Ok(token.clone());
            }
        }
    }

    let sa = load_sa_key()?;
    let now = chrono::Utc::now().timestamp() as u64;

    let claims = serde_json::json!({
        "iss": sa.client_email,
        "scope": "https://www.googleapis.com/auth/calendar.events",
        "aud": "https://oauth2.googleapis.com/token",
        "iat": now,
        "exp": now + 3600,
    });

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
        .map_err(|e| format!("Invalid SA private key: {e}"))?;
    let jwt = jsonwebtoken::encode(&header, &claims, &key)
        .map_err(|e| format!("JWT encode failed: {e}"))?;

    let client = reqwest::Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()
        .await
        .map_err(|e| format!("Token request failed: {e}"))?;

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Token response parse error: {e}"))?;

    let token = json["access_token"]
        .as_str()
        .ok_or_else(|| format!("No access_token in response: {json}"))?
        .to_string();

    // Cache for 50 minutes
    {
        let mut cache = TOKEN_CACHE.lock().unwrap();
        *cache = Some((token.clone(), now + 3000));
    }

    Ok(token)
}

pub fn is_configured() -> bool {
    std::env::var("GOOGLE_SA_KEY_FILE").is_ok()
}

fn encode_calendar_id(id: &str) -> String {
    url::form_urlencoded::byte_serialize(id.as_bytes()).collect()
}

pub async fn create_event(
    title: &str,
    date: &str,
    item_id: &str,
) -> Result<String, String> {
    let token = get_token().await?;
    let cal_id = encode_calendar_id(&calendar_id());

    let end_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date: {e}"))?
        .succ_opt()
        .ok_or("Date overflow")?
        .format("%Y-%m-%d")
        .to_string();

    let body = serde_json::json!({
        "summary": title,
        "start": { "date": date },
        "end": { "date": end_date },
        "extendedProperties": {
            "private": {
                "life_manager_id": item_id
            }
        }
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{cal_id}/events"
        ))
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Create event failed: {e}"))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Create event error: {text}"));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse create response: {e}"))?;
    json["id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No event id in response".to_string())
}

pub async fn update_event(
    event_id: &str,
    title: &str,
    date: &str,
) -> Result<(), String> {
    let token = get_token().await?;
    let cal_id = encode_calendar_id(&calendar_id());

    let end_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date: {e}"))?
        .succ_opt()
        .ok_or("Date overflow")?
        .format("%Y-%m-%d")
        .to_string();

    let body = serde_json::json!({
        "summary": title,
        "start": { "date": date },
        "end": { "date": end_date },
    });

    let client = reqwest::Client::new();
    let resp = client
        .patch(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{cal_id}/events/{event_id}"
        ))
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Update event failed: {e}"))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Update event error: {text}"));
    }

    Ok(())
}

pub async fn delete_event(event_id: &str) -> Result<(), String> {
    let token = get_token().await?;
    let cal_id = encode_calendar_id(&calendar_id());

    let client = reqwest::Client::new();
    let resp = client
        .delete(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{cal_id}/events/{event_id}"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Delete event failed: {e}"))?;

    // 404 or 410 = already deleted, that's fine
    if !resp.status().is_success() && resp.status().as_u16() != 404 && resp.status().as_u16() != 410 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Delete event error: {text}"));
    }

    Ok(())
}

/// Find an existing event by life_manager_id extended property
pub async fn find_event_by_item_id(item_id: &str) -> Result<Option<String>, String> {
    let token = get_token().await?;
    let cal_id = encode_calendar_id(&calendar_id());
    let filter: String = url::form_urlencoded::byte_serialize(
        format!("life_manager_id={item_id}").as_bytes()
    ).collect();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{cal_id}/events?privateExtendedProperty={filter}&maxResults=1"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Find event failed: {e}"))?;

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Parse find response: {e}"))?;

    Ok(json["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|item| item["id"].as_str())
        .map(|s| s.to_string()))
}

/// Sync a single checklist item to Google Calendar.
/// Called fire-and-forget via tokio::spawn after mutations.
pub async fn sync_item(item_id: &str, title: &str, date: Option<&str>, done: bool, google_event_id: Option<&str>) {
    if !is_configured() {
        return;
    }

    let result: Result<(), String> = async {
        if done {
            // Completed → delete event if exists
            if let Some(eid) = google_event_id {
                delete_event(eid).await?;
            } else if !item_id.is_empty() {
                if let Some(eid) = find_event_by_item_id(item_id).await? {
                    delete_event(&eid).await?;
                }
            }
            // Clear google_event_id in DB (try both tables)
            let conn = super::db::pool().get().map_err(|e| e.to_string())?;
            let _ = conn.execute(
                "UPDATE checklist_items SET google_event_id = NULL WHERE id = ?1",
                rusqlite::params![item_id],
            );
            let _ = conn.execute(
                "UPDATE shopee_packages SET google_event_id = NULL WHERE id = ?1",
                rusqlite::params![item_id],
            );
        } else if let Some(date) = date {
            if let Some(eid) = google_event_id {
                // Update existing event
                update_event(eid, title, date).await?;
            } else {
                // Create new event
                let event_id = create_event(title, date, item_id).await?;
                // Store event ID in DB (try both tables)
                let conn = super::db::pool().get().map_err(|e| e.to_string())?;
                let affected = conn.execute(
                    "UPDATE checklist_items SET google_event_id = ?2 WHERE id = ?1",
                    rusqlite::params![item_id, event_id],
                ).map_err(|e| e.to_string())?;
                if affected == 0 {
                    conn.execute(
                        "UPDATE shopee_packages SET google_event_id = ?2 WHERE id = ?1",
                        rusqlite::params![item_id, event_id],
                    ).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }.await;

    if let Err(e) = result {
        tracing::warn!("Google Calendar sync failed for item {item_id}: {e}");
    }
}

use super::db;

/// Fire-and-forget notification creation. Never blocks the caller on failure.
pub fn create_notification(actor: &str, action: &str, module: &str, item_text: &str) {
    let conn = match db::pool().get() {
        Ok(c) => c,
        Err(_) => return,
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;
    let text: String = item_text.chars().take(100).collect();

    let _ = conn.execute(
        "INSERT INTO notifications (id, actor, action, module, item_text, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, actor, action, module, text, now],
    );

    // Clean up entries older than 7 days
    let cutoff = now - 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
    let _ = conn.execute(
        "DELETE FROM notifications WHERE created_at < ?1",
        rusqlite::params![cutoff],
    );

    // Collect push subscription data synchronously (rusqlite is not Send)
    let subs = load_push_subscriptions(actor);

    // Drop conn before spawning
    drop(conn);

    if subs.is_empty() {
        return;
    }

    let actor_owned = actor.to_string();
    let action_owned = action.to_string();
    let module_owned = module.to_string();
    let text_owned = text.clone();
    tokio::spawn(async move {
        send_push(subs, &actor_owned, &action_owned, &module_owned, &text_owned).await;
    });
}

struct PushSub {
    endpoint: String,
    p256dh: String,
    auth: String,
    id: String,
}

/// Load push subscriptions from DB (synchronous — safe to call from non-async context).
fn load_push_subscriptions(actor: &str) -> Vec<PushSub> {
    let conn = match db::pool().get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut stmt = match conn.prepare(
        "SELECT ps.endpoint, ps.p256dh, ps.auth, ps.id
         FROM push_subscriptions ps
         JOIN notification_settings ns ON ns.user_name = ps.user_name
         WHERE ns.enabled = 1 AND ps.user_name != ?1",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map(rusqlite::params![actor], |row| {
        Ok(PushSub {
            endpoint: row.get(0)?,
            p256dh: row.get(1)?,
            auth: row.get(2)?,
            id: row.get(3)?,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Send web push notifications (async, no DB references — safe to spawn).
async fn send_push(subs: Vec<PushSub>, actor: &str, action: &str, module: &str, item_text: &str) {
    use web_push::*;

    let vapid_private = match std::env::var("VAPID_PRIVATE_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return,
    };

    let partial_builder = match VapidSignatureBuilder::from_base64_no_sub(&vapid_private) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("VAPID key error: {e}");
            return;
        }
    };

    let body = format!("{actor} {action} {item_text}");
    let payload = serde_json::json!({
        "title": format!("Life Manager — {}", capitalize(module)),
        "body": body,
        "module": module,
    });
    let payload_str = payload.to_string();

    let client = IsahcWebPushClient::new().ok();
    let Some(client) = client else { return };

    for sub in &subs {
        let sub_info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);

        let sig = match partial_builder.clone().add_sub_info(&sub_info).build() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut builder = WebPushMessageBuilder::new(&sub_info);
        builder.set_vapid_signature(sig);
        let _ = builder.set_payload(ContentEncoding::Aes128Gcm, payload_str.as_bytes());

        match builder.build() {
            Ok(msg) => {
                if let Err(e) = client.send(msg).await {
                    tracing::debug!("Push send failed for {}: {e}", sub.id);
                    if matches!(e, WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_)) {
                        if let Ok(conn) = db::pool().get() {
                            let _ = conn.execute(
                                "DELETE FROM push_subscriptions WHERE id = ?1",
                                rusqlite::params![sub.id],
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Push build failed: {e}");
            }
        }
    }
}

/// Send push with explicit title/body (no actor formatting).
async fn send_push_raw(subs: Vec<PushSub>, title: &str, body: &str, module: &str) {
    use web_push::*;

    let vapid_private = match std::env::var("VAPID_PRIVATE_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return,
    };

    let partial_builder = match VapidSignatureBuilder::from_base64_no_sub(&vapid_private) {
        Ok(b) => b,
        Err(_) => return,
    };

    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "module": module,
    });
    let payload_str = payload.to_string();

    let client = IsahcWebPushClient::new().ok();
    let Some(client) = client else { return };

    for sub in &subs {
        let sub_info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);
        let sig = match partial_builder.clone().add_sub_info(&sub_info).build() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut builder = WebPushMessageBuilder::new(&sub_info);
        builder.set_vapid_signature(sig);
        let _ = builder.set_payload(ContentEncoding::Aes128Gcm, payload_str.as_bytes());

        if let Ok(msg) = builder.build() {
            if let Err(e) = client.send(msg).await {
                tracing::debug!("Targeted push failed for {}: {e}", sub.id);
                if matches!(e, WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_)) {
                    if let Ok(conn) = db::pool().get() {
                        let _ = conn.execute(
                            "DELETE FROM push_subscriptions WHERE id = ?1",
                            rusqlite::params![sub.id],
                        );
                    }
                }
            }
        }
    }
}

/// Check cycle phase and notify all other users if in a sensitive phase.
/// Deduplicates by checking if a phase alert was already created today.
pub fn check_cycle_phase_alert() {
    let conn = match db::pool().get() {
        Ok(c) => c,
        Err(_) => return,
    };

    let today = chrono::Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    // Check if already sent a phase alert today
    let already_sent: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM notifications WHERE actor = '_system:cycle' AND module = 'cycle' AND item_text LIKE ?1",
            rusqlite::params![format!("%[{}]%", today_str)],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if already_sent {
        return;
    }

    // Load cycle settings (shared data, user_id = 'default')
    let settings = conn
        .query_row(
            "SELECT average_cycle_length, average_period_duration, on_birth_control
             FROM cycle_settings WHERE user_id = 'default'",
            [],
            |row| {
                Ok(crate::models::CycleSettings {
                    average_cycle_length: row.get(0)?,
                    average_period_duration: row.get(1)?,
                    on_birth_control: row.get(2)?,
                })
            },
        )
        .unwrap_or_default();

    let last_start: Option<String> = conn
        .query_row(
            "SELECT start_date FROM cycles WHERE user_id = 'default' ORDER BY start_date DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let Some(start_str) = last_start else { return };
    let Ok(last_period_start) = start_str.parse::<chrono::NaiveDate>() else { return };
    let Some(phase_info) = crate::models::current_phase(last_period_start, today, &settings) else {
        return;
    };

    use crate::models::CyclePhase;
    let message = match phase_info.phase {
        CyclePhase::LateLuteal => Some(format!(
            "Late Luteal phase (Sensitive) — mood swings likely. {} days until next period. [{}]",
            phase_info.days_in_phase_remaining, today_str
        )),
        CyclePhase::Menstruation => Some(format!(
            "Period started (Rest & Reset) — may need extra care. {} days remaining. [{}]",
            phase_info.days_in_phase_remaining, today_str
        )),
        _ => None,
    };

    if let Some(msg) = message {
        // Use "_system:cycle" as actor — this means it shows up for all other users
        // (the existing notification system filters out actor == current_user)
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis() as f64;
        let _ = conn.execute(
            "INSERT INTO notifications (id, actor, action, module, item_text, created_at)
             VALUES (?1, '_system:cycle', 'alert', 'cycle', ?2, ?3)",
            rusqlite::params![id, msg, now],
        );

        // Send push to all opted-in users
        let subs = load_push_subscriptions("_system:cycle");
        drop(conn);

        if !subs.is_empty() {
            let push_msg = msg.split(" [").next().unwrap_or(&msg).to_string();
            tokio::spawn(async move {
                send_push_raw(subs, "Cycle Tracker", &push_msg, "cycle").await;
            });
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

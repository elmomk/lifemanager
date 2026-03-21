use dioxus::prelude::*;

use crate::models::ShopeePackage;

#[server]
pub async fn ocr_shopee_code(image_base64: String) -> Result<String, ServerFnError> {
    use base64::Engine;
    use std::io::Write;

    // Decode base64 image data (strip data URL prefix if present)
    let raw_b64 = if let Some(pos) = image_base64.find(",") {
        &image_base64[pos + 1..]
    } else {
        &image_base64
    };

    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_b64)
        .map_err(|e| ServerFnError::new(format!("Base64 decode error: {e}")))?;

    // Write to temp file
    let mut tmp = tempfile::NamedTempFile::new()
        .map_err(|e| ServerFnError::new(format!("Temp file error: {e}")))?;
    tmp.write_all(&image_bytes)
        .map_err(|e| ServerFnError::new(format!("Write error: {e}")))?;

    let tmp_path = tmp.path().to_string_lossy().to_string();

    // Run tesseract OCR
    let output = std::process::Command::new("tesseract")
        .arg(&tmp_path)
        .arg("stdout")
        .arg("--psm")
        .arg("6")
        .output()
        .map_err(|e| ServerFnError::new(format!("Tesseract error: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerFnError::new(format!("Tesseract failed: {stderr}")));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();

    // Extract pickup code: look for alphanumeric sequences (typically 6-20 chars)
    let code = text
        .split_whitespace()
        .find(|word| {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            trimmed.len() >= 6 && trimmed.chars().all(|c| c.is_alphanumeric())
        })
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .ok_or_else(|| ServerFnError::new("No pickup code found in image".to_string()))?;

    Ok(code)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn list_shopee() -> Result<Vec<ShopeePackage>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, title, store, code, picked_up, created_at
             FROM shopee_packages
             WHERE user_id = ?1
             ORDER BY picked_up ASC, created_at DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok(ShopeePackage {
                id: row.get(0)?,
                title: row.get(1)?,
                store: row.get(2)?,
                code: row.get(3)?,
                picked_up: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_shopee(
    title: String,
    store: Option<String>,
    code: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO shopee_packages (id, user_id, title, store, code, picked_up, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        rusqlite::params![id, user_id, title, store, code, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn toggle_shopee(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE shopee_packages SET picked_up = 1 - picked_up WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_shopee(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM shopee_packages WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

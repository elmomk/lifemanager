use dioxus::prelude::*;

use crate::models::{OcrResult, ShopeePackage};

#[server(headers: axum::http::HeaderMap)]
pub async fn ocr_shopee(image_base64: String) -> Result<Vec<OcrResult>, ServerFnError> {
    use base64::Engine;
    use std::io::Write;
    use crate::server::auth;

    auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;

    let raw_b64 = if let Some(pos) = image_base64.find(",") {
        &image_base64[pos + 1..]
    } else {
        &image_base64
    };

    const MAX_BASE64_SIZE: usize = 10 * 1024 * 1024 * 4 / 3;
    if raw_b64.len() > MAX_BASE64_SIZE {
        return Err(ServerFnError::new("Image too large (max 10MB)".to_string()));
    }

    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_b64)
        .map_err(|e| ServerFnError::new(format!("Base64 decode error: {e}")))?;

    let mut tmp = tempfile::NamedTempFile::new()
        .map_err(|e| ServerFnError::new(format!("Temp file error: {e}")))?;
    tmp.write_all(&image_bytes)
        .map_err(|e| ServerFnError::new(format!("Write error: {e}")))?;

    let tmp_path = tmp.path().to_string_lossy().to_string();

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::process::Command::new("tesseract")
            .arg(&tmp_path)
            .arg("stdout")
            .arg("-l")
            .arg("chi_tra+eng")
            .arg("--psm")
            .arg("3")
            .output(),
    )
    .await
    .map_err(|_| ServerFnError::new("OCR timed out (30s limit)".to_string()))?
    .map_err(|e| ServerFnError::new(format!("Tesseract error: {e}")))?;

    if !output.status.success() {
        tracing::error!("Tesseract failed: {}", String::from_utf8_lossy(&output.stderr));
        return Err(ServerFnError::new("OCR processing failed".to_string()));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();

    let results = extract_packages(&text);

    if results.is_empty() {
        return Err(ServerFnError::new("Could not extract any packages from image".to_string()));
    }

    Ok(results)
}

/// Extract multiple packages from OCR text.
/// Splits text into sections by looking for repeated pickup-code or store patterns.
#[cfg(not(target_arch = "wasm32"))]
fn extract_packages(text: &str) -> Vec<OcrResult> {
    use crate::models::OcrResult;

    // Find all pickup codes with their positions
    let codes = extract_all_codes(text);
    let stores = extract_all_stores(text);
    let titles = extract_all_titles(text);

    if codes.is_empty() && titles.is_empty() {
        // Single package fallback: try to extract whatever we can
        let code = codes.into_iter().next().map(|(_, c)| c);
        let store = stores.into_iter().next().map(|(_, s)| s);
        let title = titles.into_iter().next().map(|(_, t)| t);
        if code.is_some() || store.is_some() || title.is_some() {
            return vec![OcrResult { title, store, code }];
        }
        return vec![];
    }

    // If we have multiple codes, each code is a package
    if codes.len() > 1 {
        return codes.iter().map(|(pos, code)| {
            // Find the nearest store and title that appear BEFORE this code
            let store = stores.iter()
                .filter(|(sp, _)| *sp < *pos)
                .last()
                .map(|(_, s)| s.clone());
            let title = titles.iter()
                .filter(|(tp, _)| *tp < *pos)
                .last()
                .map(|(_, t)| t.clone());
            OcrResult {
                title,
                store,
                code: Some(code.clone()),
            }
        }).collect();
    }

    // Single code or no code — try to match by titles
    if titles.len() > 1 && codes.len() <= 1 {
        // Multiple products but one code — likely one package with multiple items
        // Just return as single package
        let code = codes.into_iter().next().map(|(_, c)| c);
        let store = stores.into_iter().next().map(|(_, s)| s);
        let title_strs: Vec<String> = titles.iter().map(|(_, t)| t.clone()).collect();
        return vec![OcrResult {
            title: Some(title_strs.join(" + ")),
            store,
            code,
        }];
    }

    // Single package
    let code = codes.into_iter().next().map(|(_, c)| c);
    let store = stores.into_iter().next().map(|(_, s)| s);
    let title = titles.into_iter().next().map(|(_, t)| t);
    if code.is_some() || store.is_some() || title.is_some() {
        vec![OcrResult { title, store, code }]
    } else {
        vec![]
    }
}

/// Returns all pickup codes with their byte position in the text.
#[cfg(not(target_arch = "wasm32"))]
fn extract_all_codes(text: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let code_patterns = &["取件驗證碼", "验证码", "驗證碼", "取件码"];

    for pattern in code_patterns {
        let mut search_from = 0;
        while let Some(pos) = text[search_from..].find(pattern) {
            let abs_pos = search_from + pos;
            let after = &text[abs_pos + pattern.len()..];
            let after = after.trim_start_matches(|c: char| {
                c == '：' || c == ':' || c == ' ' || c == '\t' || c == ',' || c == '，'
            });
            let code: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if code.len() >= 4 {
                results.push((abs_pos, code));
            }
            search_from = abs_pos + pattern.len();
        }
    }

    // Deduplicate by code value
    results.dedup_by(|a, b| a.1 == b.1);

    // If no pattern-based codes found, look for standalone 6-10 digit numbers
    if results.is_empty() {
        let mut pos = 0;
        for segment in text.split(|c: char| !c.is_ascii_digit()) {
            if segment.len() >= 6 && segment.len() <= 10 {
                results.push((pos, segment.to_string()));
            }
            pos += segment.len() + 1;
        }
    }

    results
}

/// Returns all store/location mentions with their byte position.
#[cfg(not(target_arch = "wasm32"))]
fn extract_all_stores(text: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();

    for (line_start, line) in line_positions(text) {
        // Pattern: 至 ... 取件
        if let Some(start) = line.find('至') {
            let after = &line[start + '至'.len_utf8()..];
            if let Some(end) = after.find("取件") {
                let store = after[..end].trim()
                    .trim_start_matches("蝦皮店到店")
                    .trim_start_matches("蝦皮")
                    .trim();
                if !store.is_empty() {
                    results.push((line_start + start, store.to_string()));
                    continue;
                }
            }
        }

        // Pattern: 店到店 LOCATION
        if let Some(pos) = line.find("店到店") {
            let after = &line[pos + "店到店".len()..];
            let store = after.trim().trim_end_matches(|c: char| {
                c == '。' || c == '.' || c == ',' || c == '，'
            }).trim();
            if !store.is_empty() && store.len() > 2 {
                results.push((line_start + pos, store.to_string()));
            }
        }
    }

    results
}

/// Returns all product titles with their byte position.
#[cfg(not(target_arch = "wasm32"))]
fn extract_all_titles(text: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let skip_patterns = &[
        "待收貨", "待付款", "待出貨", "訂單", "退貨", "退款", "追蹤",
        "取件", "驗證", "請於", "猜你", "購買", "蝦皮", "已售出",
    ];

    for (line_start, line) in line_positions(text) {
        let line = line.trim();
        if line.is_empty() { continue; }

        // Product names in 【】brackets
        if let Some(start) = line.find('【') {
            let title = &line[start..];
            let title = title.trim_end_matches(|c: char| c == '。' || c == '.' || c == '\n');
            if title.len() > 2 {
                results.push((line_start + start, title.to_string()));
                continue;
            }
        }

        // Product-like lines
        if line.chars().count() < 8 { continue; }
        if skip_patterns.iter().any(|p| line.contains(p)) { continue; }
        if line.chars().all(|c| c.is_ascii_digit() || c == '$' || c == ',' || c == '.' || c == ' ') { continue; }

        // Might be a product title — only add if we haven't already found a bracket title near this position
        if results.iter().all(|(pos, _)| (*pos as isize - line_start as isize).unsigned_abs() > 100) {
            results.push((line_start, line.to_string()));
        }
    }

    results
}

/// Helper: iterate lines with their byte offset in the original text.
#[cfg(not(target_arch = "wasm32"))]
fn line_positions(text: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut pos = 0;
    for line in text.lines() {
        result.push((pos, line));
        pos += line.len() + 1; // +1 for \n
    }
    result
}

#[server(headers: axum::http::HeaderMap)]
pub async fn list_shopee() -> Result<Vec<ShopeePackage>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, title, store, code, picked_up, created_at, completed_by
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
                completed_by: row.get(6)?,
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
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::text(&title, "title")?;
    if let Some(ref s) = store { validate::short(s, "store")?; }
    if let Some(ref c) = code { validate::short(c, "code")?; }
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

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let display_name = auth::display_name_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE shopee_packages SET picked_up = 1 - picked_up,
         completed_by = CASE WHEN picked_up = 0 THEN ?3 ELSE NULL END
         WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id, display_name],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_shopee(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM shopee_packages WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

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
/// Strategy: Split by store header lines, then extract fields within each section.
#[cfg(not(target_arch = "wasm32"))]
fn extract_packages(text: &str) -> Vec<OcrResult> {
    use crate::models::OcrResult;

    let lines: Vec<(usize, &str)> = line_positions(text);

    // Find store header indices — lines that look like store brand names
    // These are the delimiters between packages
    let section_starts = find_store_headers(&lines);

    if section_starts.is_empty() {
        // Fallback: try to extract a single package from entire text
        return extract_single_package(text).into_iter().collect();
    }

    let mut results = Vec::new();

    for (i, &start_idx) in section_starts.iter().enumerate() {
        let end_idx = section_starts.get(i + 1).copied().unwrap_or(lines.len());
        let section_lines: Vec<&str> = lines[start_idx..end_idx].iter().map(|(_, l)| *l).collect();
        let section_text = section_lines.join("\n");

        let title = extract_title_from_section(&section_lines);
        let store = extract_store_from_section(&section_lines);
        let code = extract_code_from_section(&section_text);
        let _delivery_status = extract_delivery_status(&section_lines);

        if title.is_some() || store.is_some() || code.is_some() {
            results.push(OcrResult {
                title,
                store,
                code, // None for packages not yet arrived
            });
        }
    }

    // Dedup: if two adjacent results have identical titles, merge them
    results.dedup_by(|a, b| {
        a.title.is_some() && a.title == b.title
    });

    if results.is_empty() {
        // Last resort fallback
        return extract_single_package(text).into_iter().collect();
    }

    results
}

/// Detect store header lines. These are brand/store name lines that start each package section.
/// Patterns: lines with CJK + brand-like text, often followed by product lines.
/// Heuristic: lines that DON'T match known non-header patterns and appear before product titles.
#[cfg(not(target_arch = "wasm32"))]
fn find_store_headers(lines: &[(usize, &str)]) -> Vec<usize> {
    let mut headers = Vec::new();
    let non_header_patterns = &[
        "待收貨", "待付款", "待出貨", "訂單", "退貨", "退款", "追蹤",
        "取件", "驗證", "請於", "猜你", "購買", "已售出", "查看更多",
        "預計於", "配達", "包裹", "出貨", "物流", "運送中",
        "【", "】", "取件驗證碼", "店到店",
    ];
    let store_indicators = &[
        "旗艦", "官方", "專賣", "OUTLET", "outlet", "Shop", "shop", "SHOP",
        "Store", "store", "STORE", "旗艦店", "官方店",
    ];

    for (idx, (_, line)) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() < 3 { continue; }

        // Skip lines that are clearly not headers
        if non_header_patterns.iter().any(|p| trimmed.contains(p)) { continue; }

        // Skip pure numeric/price lines
        if trimmed.chars().all(|c| c.is_ascii_digit() || c == '$' || c == ',' || c == '.' || c == ' ' || c == 'x' || c == 'X') { continue; }

        // A store header often contains brand-like indicators
        let has_indicator = store_indicators.iter().any(|p| trimmed.contains(p));

        // Or it's a short-medium line (store names aren't super long) with mixed CJK/ASCII
        let char_count = trimmed.chars().count();
        let has_cjk = trimmed.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}');
        let has_ascii = trimmed.chars().any(|c| c.is_ascii_alphabetic());
        let is_reasonable_length = char_count >= 3 && char_count <= 40;

        // Check if next few lines contain a product title (【】) — confirms this is a section header
        let next_has_product = lines[idx+1..std::cmp::min(idx+5, lines.len())]
            .iter()
            .any(|(_, l)| l.contains('【'));

        if has_indicator && is_reasonable_length {
            headers.push(idx);
        } else if next_has_product && is_reasonable_length && (has_cjk || has_ascii) {
            // If the next few lines have a product title, this line is likely a store header
            headers.push(idx);
        }
    }

    headers
}

/// Extract a product title from a section's lines.
#[cfg(not(target_arch = "wasm32"))]
fn extract_title_from_section(lines: &[&str]) -> Option<String> {
    // First try: 【】bracket titles
    for line in lines {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find('【') {
            let title = &trimmed[start..];
            let title = title.trim_end_matches(|c: char| c == '。' || c == '.' || c == '\n');
            if title.len() > 2 {
                return Some(title.to_string());
            }
        }
    }

    // Fallback: look for product-like lines (longer text, not status/UI text)
    let skip = &[
        "待收貨", "待付款", "待出貨", "訂單", "退貨", "退款", "追蹤",
        "取件", "驗證", "請於", "猜你", "購買", "蝦皮", "已售出",
        "預計於", "配達", "查看更多", "物流", "運送中", "出貨",
    ];
    for line in lines.iter().skip(1) { // skip first line (store header)
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.chars().count() < 8 { continue; }
        if skip.iter().any(|p| trimmed.contains(p)) { continue; }
        if trimmed.chars().all(|c| c.is_ascii_digit() || c == '$' || c == ',' || c == '.' || c == ' ') { continue; }
        return Some(trimmed.to_string());
    }

    None
}

/// Extract store/location from section lines.
#[cfg(not(target_arch = "wasm32"))]
fn extract_store_from_section(lines: &[&str]) -> Option<String> {
    for line in lines {
        // Pattern: 至 ... 取件
        if let Some(start) = line.find('至') {
            let after = &line[start + '至'.len_utf8()..];
            if let Some(end) = after.find("取件") {
                let store = after[..end].trim()
                    .trim_start_matches("蝦皮店到店")
                    .trim_start_matches("蝦皮")
                    .trim();
                if !store.is_empty() {
                    return Some(store.to_string());
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
                return Some(store.to_string());
            }
        }
    }
    None
}

/// Extract pickup code from section text.
#[cfg(not(target_arch = "wasm32"))]
fn extract_code_from_section(text: &str) -> Option<String> {
    let code_patterns = &["取件驗證碼", "验证码", "驗證碼", "取件码"];

    for pattern in code_patterns {
        if let Some(pos) = text.find(pattern) {
            let after = &text[pos + pattern.len()..];
            let after = after.trim_start_matches(|c: char| {
                c == '：' || c == ':' || c == ' ' || c == '\t' || c == ',' || c == '，'
            });
            let code: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if code.len() >= 4 {
                return Some(code);
            }
        }
    }

    None
}

/// Detect delivery status lines like "預計於 DATE 配達" (package not yet arrived).
#[cfg(not(target_arch = "wasm32"))]
fn extract_delivery_status(lines: &[&str]) -> Option<String> {
    for line in lines {
        if line.contains("預計於") || line.contains("預計") && line.contains("配達") {
            return Some(line.trim().to_string());
        }
        if line.contains("運送中") || line.contains("出貨中") {
            return Some(line.trim().to_string());
        }
    }
    None
}

/// Fallback: try to extract a single package from the entire text.
#[cfg(not(target_arch = "wasm32"))]
fn extract_single_package(text: &str) -> Option<OcrResult> {
    use crate::models::OcrResult;

    let lines: Vec<&str> = text.lines().collect();
    let title = extract_title_from_section(&lines);
    let store = extract_store_from_section(&lines);
    let code = extract_code_from_section(text);

    if title.is_some() || store.is_some() || code.is_some() {
        Some(OcrResult { title, store, code })
    } else {
        None
    }
}

/// Helper: iterate lines with their byte offset in the original text.
#[cfg(not(target_arch = "wasm32"))]
fn line_positions(text: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut pos = 0;
    for line in text.lines() {
        result.push((pos, line));
        pos += line.len() + 1;
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

#[server(headers: axum::http::HeaderMap)]
pub async fn update_shopee_code(id: String, code: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::short(&code, "code")?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE shopee_packages SET code = ?3 WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id, code],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn find_matching_packages(titles: Vec<String>) -> Result<Vec<ShopeePackage>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Find active (not picked up) packages that match any of the given titles
    let mut results = Vec::new();
    for title in &titles {
        let mut stmt = conn
            .prepare(
                "SELECT id, title, store, code, picked_up, created_at, completed_by
                 FROM shopee_packages
                 WHERE user_id = ?1 AND picked_up = 0 AND title LIKE '%' || ?2 || '%'"
            )
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let items = stmt
            .query_map(rusqlite::params![user_id, title], |row| {
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

        results.extend(items);
    }

    // Dedup by id
    results.sort_by(|a, b| a.id.cmp(&b.id));
    results.dedup_by(|a, b| a.id == b.id);

    Ok(results)
}

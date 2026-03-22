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

    let raw_text = String::from_utf8_lossy(&output.stdout).to_string();

    // Normalize OCR output: Tesseract often adds spaces between CJK characters
    // and uses fullwidth punctuation. Collapse spaces between CJK chars and
    // normalize common OCR artifacts.
    let text = normalize_ocr_text(&raw_text);
    tracing::debug!("OCR normalized text:\n{text}");

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
        let (due_date, date_is_estimate) = extract_due_date(&section_lines);

        if title.is_some() || store.is_some() || code.is_some() {
            results.push(OcrResult {
                title,
                store,
                code,
                due_date,
                date_is_estimate,
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
/// In Shopee's "待收貨" list, each package starts with a line like:
///   "QMAT OUTLET 運動/瑜珈墊 巧拼地墊 按... 待收貨"
///   "DENPA GINGA 電波銀河 待收貨"
/// The key signal: `待收貨` (or `待出貨`) at the end of a line that also has a store name.
#[cfg(not(target_arch = "wasm32"))]
fn find_store_headers(lines: &[(usize, &str)]) -> Vec<usize> {
    let mut headers = Vec::new();

    // Status markers that appear on store header lines in the Shopee order list
    // Note: OCR may produce "待收吉" instead of "待收貨"
    let header_markers = &["待收貨", "待出貨", "待收吉"];

    // Store name indicators (brand-like patterns)
    let store_indicators = &[
        "旗艦", "官方", "專賣", "OUTLET", "outlet", "Shop", "shop", "SHOP",
        "Store", "store", "STORE", "旗艦店", "官方店", "GINGA", "DENPA",
    ];

    for (idx, (_, line)) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() < 3 { continue; }

        // Primary strategy: line contains 待收貨/待出貨 — this IS a store header line
        let has_marker = header_markers.iter().any(|p| trimmed.contains(p));
        if has_marker {
            // Make sure it's not JUST the tab label (e.g. a standalone "待收貨" tab)
            // Header lines have store name text alongside the marker
            let char_count = trimmed.chars().count();
            if char_count > 4 {
                headers.push(idx);
                continue;
            }
        }

        // Fallback: lines with store indicators followed by a product title within 5 lines
        let has_indicator = store_indicators.iter().any(|p| trimmed.contains(p));
        if has_indicator {
            let char_count = trimmed.chars().count();
            if char_count >= 3 && char_count <= 50 {
                // Confirm: next few lines should have a product title (【】)
                let next_has_product = lines[idx+1..std::cmp::min(idx+5, lines.len())]
                    .iter()
                    .any(|(_, l)| l.contains('【'));
                if next_has_product {
                    headers.push(idx);
                }
            }
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
        "完成訂單", "追蹤訂單", "檢視其他", "較長備貨", "超取", "店到店",
        "訂單金額", "包裹抵達", "處理中",
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
/// The pickup info may span multiple lines, e.g.:
///   "請於 2026-03-28 前 , 至蝦皮店到店南港重"
///   "陽 - 智取店取件。取件驗證碼 ﹔ 782399。"
/// So we join the section text and search in the combined string.
#[cfg(not(target_arch = "wasm32"))]
fn extract_store_from_section(lines: &[&str]) -> Option<String> {
    // Join all lines to handle multi-line pickup info
    let combined = lines.join(" ");

    // Pattern: 至 ... 取件 (pickup location)
    if let Some(start) = combined.find('至') {
        let after = &combined[start + '至'.len_utf8()..];
        if let Some(end) = after.find("取件") {
            let store = after[..end].trim()
                .trim_start_matches("蝦皮店到店")
                .trim_start_matches("蝦皮")
                .trim();
            if !store.is_empty() && store.chars().count() > 1 {
                let store = store.trim_end_matches(|c: char| {
                    c == '。' || c == '.' || c == ',' || c == '，' || c == ' '
                });
                return Some(store.to_string());
            }
        }
    }

    // Pattern: 店到店 LOCATION (on a single line)
    for line in lines {
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
                || c == '﹔' || c == ';' || c == '；'
            });
            let code: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if code.len() >= 4 {
                return Some(code);
            }
        }
    }

    None
}

/// Extract due date from section lines. Returns (date, is_estimate).
/// - "請於 DATE 前" → pickup deadline (is_estimate = false)
/// - "預計於 DATE 配達" → delivery estimate (is_estimate = true)
#[cfg(not(target_arch = "wasm32"))]
fn extract_due_date(lines: &[&str]) -> (Option<String>, bool) {
    let current_year = chrono::Utc::now().format("%Y").to_string();

    for line in lines {
        // Pattern: 請於 DATE 前 (pickup deadline — firm date)
        if let Some(pos) = line.find("請於") {
            let after = &line[pos + "請於".len()..];
            if let Some(date) = parse_chinese_date(after, &current_year) {
                return (Some(date), false);
            }
        }

        // Pattern: 預計於 DATE - DATE 配達 (delivery estimate)
        if let Some(pos) = line.find("預計於") {
            let after = &line[pos + "預計於".len()..];
            let dates = extract_dates_from_segment(after, &current_year);
            if let Some(last) = dates.last() {
                return (Some(last.clone()), true);
            }
        }

        // Pattern: 預計 DATE (estimate, but not 預計於)
        if line.contains("預計") && !line.contains("預計於") {
            if let Some(pos) = line.find("預計") {
                let after = &line[pos + "預計".len()..];
                if let Some(date) = parse_chinese_date(after, &current_year) {
                    return (Some(date), true);
                }
            }
        }
    }

    (None, false)
}

/// Parse a date from text that might be in formats like:
/// "3/25", "03/25", "3月25日", "2026/03/25", "2026-03-25"
#[cfg(not(target_arch = "wasm32"))]
fn parse_chinese_date(text: &str, current_year: &str) -> Option<String> {
    let text = text.trim();

    // Try M月D日 format
    if let Some(month_pos) = text.find('月') {
        let month_str: String = text[..month_pos]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if let Some(day_pos) = text[month_pos..].find('日') {
            let between = &text[month_pos + '月'.len_utf8()..month_pos + day_pos];
            let day_str: String = between.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let (Ok(m), Ok(d)) = (month_str.parse::<u32>(), day_str.parse::<u32>()) {
                if m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                    return Some(format!("{current_year}-{m:02}-{d:02}"));
                }
            }
        }
    }

    // Try YYYY/MM/DD or MM/DD (slash or dash separated)
    let slash_parts: Vec<&str> = text
        .split(|c: char| c == '/' || c == '-')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .collect();

    if slash_parts.len() >= 3 {
        // YYYY/MM/DD
        if let (Ok(y), Ok(m), Ok(d)) = (
            slash_parts[0].parse::<i32>(),
            slash_parts[1].parse::<u32>(),
            slash_parts[2].parse::<u32>(),
        ) {
            if y >= 2020 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                return Some(format!("{y}-{m:02}-{d:02}"));
            }
        }
    } else if slash_parts.len() == 2 {
        // MM/DD
        if let (Ok(m), Ok(d)) = (slash_parts[0].parse::<u32>(), slash_parts[1].parse::<u32>()) {
            if m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                return Some(format!("{current_year}-{m:02}-{d:02}"));
            }
        }
    }

    None
}

/// Extract all parseable dates from a text segment (used for date ranges).
/// After normalization, date ranges look like: "2026-03-20 - 2026-03-22"
#[cfg(not(target_arch = "wasm32"))]
fn extract_dates_from_segment(text: &str, current_year: &str) -> Vec<String> {
    let mut dates = Vec::new();

    // Try to find all YYYY-MM-DD patterns using regex-like manual scan
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Look for 4-digit year
        if chars[i].is_ascii_digit() {
            let start = i;
            let num: String = chars[i..].iter().take_while(|c| c.is_ascii_digit()).collect();
            if num.len() == 4 {
                // Could be YYYY-MM-DD
                let rest = &text[text.char_indices().nth(start).unwrap().0..];
                if let Some(date) = parse_chinese_date(rest, current_year) {
                    dates.push(date);
                    i += 10; // skip past the date
                    continue;
                }
            }
        }
        i += 1;
    }

    // Fallback: split by common delimiters
    if dates.is_empty() {
        for part in text.split(|c: char| c == '~' || c == '至' || c == '到') {
            if let Some(date) = parse_chinese_date(part, current_year) {
                dates.push(date);
            }
        }
    }

    if dates.is_empty() {
        if let Some(date) = parse_chinese_date(text, current_year) {
            dates.push(date);
        }
    }

    dates
}

/// Fallback: try to extract a single package from the entire text.
#[cfg(not(target_arch = "wasm32"))]
fn extract_single_package(text: &str) -> Option<OcrResult> {
    use crate::models::OcrResult;

    let lines: Vec<&str> = text.lines().collect();
    let title = extract_title_from_section(&lines);
    let store = extract_store_from_section(&lines);
    let code = extract_code_from_section(text);
    let (due_date, date_is_estimate) = extract_due_date(&lines);

    if title.is_some() || store.is_some() || code.is_some() {
        Some(OcrResult { title, store, code, due_date, date_is_estimate })
    } else {
        None
    }
}

/// Normalize OCR text: collapse spaces between CJK characters and normalize punctuation.
/// Tesseract often outputs "待 收 貨" instead of "待收貨" and uses fullwidth punctuation.
#[cfg(not(target_arch = "wasm32"))]
fn normalize_ocr_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    // First pass: normalize fullwidth punctuation to ASCII equivalents
    for ch in text.chars() {
        match ch {
            '﹣' | '－' => result.push('-'),
            '﹕' | '：' => result.push(':'),
            '﹐' | '，' => result.push(','),
            '﹒' => result.push('.'),
            '﹩' => result.push('$'),
            '﹔' | '；' => result.push(';'),
            '\u{FF3B}' => result.push('['), // ［
            '\u{FF3D}' => result.push(']'), // ］
            _ => result.push(ch),
        }
    }

    // Second pass: collapse spaces between CJK characters
    // A "CJK char" includes CJK Unified Ideographs and common CJK punctuation
    let chars: Vec<char> = result.chars().collect();
    let mut collapsed = String::with_capacity(result.len());
    let mut i = 0;
    while i < chars.len() {
        collapsed.push(chars[i]);
        // If current char is CJK and next is space(s) followed by CJK, skip the spaces
        if is_cjk_or_punct(chars[i]) && i + 1 < chars.len() && chars[i + 1] == ' ' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            if j < chars.len() && is_cjk_or_punct(chars[j]) {
                // Skip spaces between CJK chars
                i = j;
                continue;
            }
        }
        i += 1;
    }

    // Also normalize [ to 【 and ] to 】 when they look like product title brackets
    collapsed = collapsed.replace('[', "【").replace(']', "】");

    collapsed
}

#[cfg(not(target_arch = "wasm32"))]
fn is_cjk_or_punct(c: char) -> bool {
    // CJK Unified Ideographs
    (c >= '\u{4e00}' && c <= '\u{9fff}')
    // CJK punctuation
    || c == '【' || c == '】' || c == '。' || c == '，' || c == '：'
    || c == '、' || c == '（' || c == '）' || c == '「' || c == '」'
    // Common fullwidth
    || c == '﹣' || c == '﹕'
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
            "SELECT id, title, store, code, due_date, date_is_estimate, picked_up, created_at, completed_by
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
                due_date: row.get(4)?,
                date_is_estimate: row.get::<_, i32>(5).unwrap_or(0) != 0,
                picked_up: row.get(6)?,
                created_at: row.get(7)?,
                completed_by: row.get(8)?,
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
    due_date: Option<String>,
    date_is_estimate: bool,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::text(&title, "title")?;
    if let Some(ref s) = store { validate::short(s, "store")?; }
    if let Some(ref c) = code { validate::short(c, "code")?; }
    if let Some(ref d) = due_date { validate::date(d)?; }
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO shopee_packages (id, user_id, title, store, code, due_date, date_is_estimate, picked_up, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
        rusqlite::params![id, user_id, title, store, code, due_date, date_is_estimate as i32, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fire-and-forget Google Calendar sync
    if let Some(ref d) = due_date {
        let id2 = id.clone();
        let title2 = title.clone();
        let d2 = d.clone();
        tokio::spawn(async move {
            crate::server::google::sync_item(&id2, &title2, Some(&d2), false, None).await;
        });
    }

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

    // Fire-and-forget Google Calendar sync
    {
        let id2 = id.clone();
        tokio::spawn(async move {
            let conn = crate::server::db::pool().get().ok();
            if let Some(conn) = conn {
                let item: Option<(String, Option<String>, bool, Option<String>)> = conn
                    .query_row(
                        "SELECT title, due_date, picked_up, google_event_id FROM shopee_packages WHERE id = ?1",
                        rusqlite::params![id2],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .ok();
                if let Some((title, due_date, picked_up, event_id)) = item {
                    crate::server::google::sync_item(
                        &id2,
                        &title,
                        due_date.as_deref(),
                        picked_up,
                        event_id.as_deref(),
                    ).await;
                }
            }
        });
    }

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_shopee(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Read google_event_id before deleting
    let event_id: Option<String> = conn
        .query_row(
            "SELECT google_event_id FROM shopee_packages WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![id, user_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    conn.execute(
        "DELETE FROM shopee_packages WHERE id = ?1 AND user_id = ?2",
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
                "SELECT id, title, store, code, due_date, date_is_estimate, picked_up, created_at, completed_by
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
                    due_date: row.get(4)?,
                    date_is_estimate: row.get::<_, i32>(5).unwrap_or(0) != 0,
                    picked_up: row.get(6)?,
                    created_at: row.get(7)?,
                    completed_by: row.get(8)?,
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

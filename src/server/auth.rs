pub fn user_from_headers(headers: &axum::http::HeaderMap) -> Result<String, String> {
    let require_auth = std::env::var("REQUIRE_AUTH").unwrap_or_default() == "true";
    if require_auth {
        if headers.get("Tailscale-User-Login").is_none() {
            return Err("Unauthorized: missing Tailscale-User-Login header".to_string());
        }
    }
    Ok("default".to_string())
}

/// Returns the actual Tailscale display name for attribution (who did what).
/// Truncates to 50 chars to prevent abuse.
pub fn display_name_from_headers(headers: &axum::http::HeaderMap) -> String {
    let name = headers
        .get("Tailscale-User-Login")
        .and_then(|v| v.to_str().ok())
        .map(|login| login.split('@').next().unwrap_or(login).to_string())
        .unwrap_or_else(|| "local".to_string());
    name.chars().take(50).collect()
}

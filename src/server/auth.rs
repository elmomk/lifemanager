pub fn user_from_headers(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("Tailscale-User-Login")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("local")
        .to_string()
}

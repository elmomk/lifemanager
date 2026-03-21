use dioxus::prelude::ServerFnError;

const MAX_TEXT: usize = 500;
const MAX_SHORT: usize = 100;

pub fn text(s: &str, field: &str) -> Result<(), ServerFnError> {
    if s.len() > MAX_TEXT {
        return Err(ServerFnError::new(format!("{field} too long (max {MAX_TEXT} chars)")));
    }
    Ok(())
}

pub fn short(s: &str, field: &str) -> Result<(), ServerFnError> {
    if s.len() > MAX_SHORT {
        return Err(ServerFnError::new(format!("{field} too long (max {MAX_SHORT} chars)")));
    }
    Ok(())
}

pub fn date(s: &str) -> Result<(), ServerFnError> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ServerFnError::new(format!("Invalid date format: {s}")))?;
    Ok(())
}

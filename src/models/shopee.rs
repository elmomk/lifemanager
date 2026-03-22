use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcrResult {
    pub title: Option<String>,
    pub store: Option<String>,
    pub code: Option<String>,
    pub due_date: Option<String>, // YYYY-MM-DD format
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopeePackage {
    pub id: String,
    pub title: String,
    pub store: Option<String>,
    pub code: Option<String>,
    pub due_date: Option<String>,
    pub picked_up: bool,
    pub created_at: f64,
    pub completed_by: Option<String>,
}

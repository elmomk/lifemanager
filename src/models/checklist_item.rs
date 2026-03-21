use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ItemCategory {
    Todo,
    Grocery,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub date: Option<NaiveDate>,
    pub done: bool,
    pub category: ItemCategory,
    pub created_at: f64,
}

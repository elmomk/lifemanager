use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ItemCategory {
    Todo,
    Grocery,
}

impl fmt::Display for ItemCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemCategory::Todo => write!(f, "Todo"),
            ItemCategory::Grocery => write!(f, "Grocery"),
        }
    }
}

impl FromStr for ItemCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Todo" => Ok(ItemCategory::Todo),
            "Grocery" => Ok(ItemCategory::Grocery),
            _ => Err(format!("Unknown category: {s}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub date: Option<NaiveDate>,
    pub done: bool,
    pub category: ItemCategory,
    pub created_at: f64,
    pub completed_by: Option<String>,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Series,
    Anime,
    Cartoon,
}

impl MediaType {
    pub fn all() -> &'static [MediaType] {
        &[
            MediaType::Movie,
            MediaType::Series,
            MediaType::Anime,
            MediaType::Cartoon,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            MediaType::Movie => "Movie",
            MediaType::Series => "Series",
            MediaType::Anime => "Anime",
            MediaType::Cartoon => "Cartoon",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WatchItem {
    pub id: String,
    pub text: String,
    pub media_type: MediaType,
    pub done: bool,
    pub created_at: f64,
    pub completed_by: Option<String>,
}

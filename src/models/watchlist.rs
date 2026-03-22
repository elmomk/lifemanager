use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Series,
    Anime,
}

impl MediaType {
    pub fn all() -> &'static [MediaType] {
        &[MediaType::Movie, MediaType::Series, MediaType::Anime]
    }

    pub fn label(&self) -> &'static str {
        match self {
            MediaType::Movie => "Movie",
            MediaType::Series => "Series",
            MediaType::Anime => "Anime",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Series" => MediaType::Series,
            "Anime" => MediaType::Anime,
            _ => MediaType::Movie,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WatchStatus {
    Unwatched,
    InProgress,
    Completed,
}

impl Default for WatchStatus {
    fn default() -> Self {
        WatchStatus::Unwatched
    }
}

impl fmt::Display for WatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchStatus::Unwatched => write!(f, "unwatched"),
            WatchStatus::InProgress => write!(f, "in_progress"),
            WatchStatus::Completed => write!(f, "completed"),
        }
    }
}

impl WatchStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => WatchStatus::InProgress,
            "completed" => WatchStatus::Completed,
            _ => WatchStatus::Unwatched,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WatchItem {
    pub id: String,
    pub text: String,
    pub media_type: MediaType,
    #[serde(default)]
    pub status: WatchStatus,
    pub done: bool,
    pub total_seasons: Option<i32>,
    pub total_episodes: Option<i32>,
    pub poster_url: Option<String>,
    pub tmdb_id: Option<i32>,
    pub jikan_id: Option<i32>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub trailer_url: Option<String>,
    /// JSON map: season_number -> episode_count (e.g. {"1":13,"2":22})
    #[serde(default)]
    pub season_data: Option<String>,
    pub created_at: f64,
    pub completed_by: Option<String>,
    // Computed from watch_progress
    #[serde(default)]
    pub current_season: Option<i32>,
    #[serde(default)]
    pub current_episode: Option<i32>,
    #[serde(default)]
    pub episodes_watched: Option<i32>,
}

// Phase 2: External API types

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaSearchResult {
    pub external_id: String,
    pub title: String,
    pub poster_url: Option<String>,
    pub year: Option<String>,
    pub total_seasons: Option<i32>,
    pub total_episodes: Option<i32>,
    pub media_type: MediaType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamingProvider {
    pub name: String,
    pub logo_url: String,
    pub provider_type: String, // "flatrate", "rent", "buy"
    #[serde(default)]
    pub link: Option<String>,  // JustWatch deep link or streaming URL
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaRecommendation {
    pub external_id: String,
    pub title: String,
    pub poster_url: Option<String>,
    pub year: Option<String>,
    pub already_in_list: bool,
}

// Explore card detail (fetched on flip)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExploreDetail {
    pub overview: Option<String>,
    pub trailer_url: Option<String>,
    pub providers: Vec<StreamingProvider>,
    pub total_seasons: Option<i32>,
    pub total_episodes: Option<i32>,
    #[serde(default)]
    pub recommendations: Vec<MediaSearchResult>,
}

// Watch Settings

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WatchSettings {
    pub streaming_providers: Vec<String>,
    pub filter_by_provider: bool,
}

impl Default for WatchSettings {
    fn default() -> Self {
        WatchSettings {
            streaming_providers: vec![],
            filter_by_provider: false,
        }
    }
}

// Phase 3: Franchise/Canon types

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FranchiseRelation {
    Sequel,
    Prequel,
    Spinoff,
}

impl fmt::Display for FranchiseRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FranchiseRelation::Sequel => write!(f, "sequel"),
            FranchiseRelation::Prequel => write!(f, "prequel"),
            FranchiseRelation::Spinoff => write!(f, "spinoff"),
        }
    }
}

impl FranchiseRelation {
    pub fn from_str(s: &str) -> Self {
        match s {
            "prequel" => FranchiseRelation::Prequel,
            "spinoff" => FranchiseRelation::Spinoff,
            _ => FranchiseRelation::Sequel,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FranchiseRelation::Sequel => "Sequel",
            FranchiseRelation::Prequel => "Prequel",
            FranchiseRelation::Spinoff => "Spin-off",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FranchiseLink {
    pub id: String,
    pub from_item_id: String,
    pub to_item_id: String,
    pub to_item_title: String,
    pub to_item_status: WatchStatus,
    pub relation: FranchiseRelation,
    pub sort_order: i32,
}

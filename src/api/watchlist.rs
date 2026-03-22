use dioxus::prelude::*;

use crate::models::{
    ExploreDetail, FranchiseLink, FranchiseRelation, MediaRecommendation, MediaSearchResult,
    MediaType, StreamingProvider, WatchItem, WatchSettings, WatchStatus,
};

#[cfg(not(target_arch = "wasm32"))]
fn load_existing_external_ids(
    conn: &rusqlite::Connection,
    user_id: &str,
) -> Result<std::collections::HashSet<String>, ServerFnError> {
    let mut stmt = conn
        .prepare("SELECT tmdb_id, jikan_id FROM watch_items WHERE user_id = ?1")
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params![user_id], |row| {
            Ok((row.get::<_, Option<i32>>(0)?, row.get::<_, Option<i32>>(1)?))
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut ids = std::collections::HashSet::new();
    for r in rows {
        if let Ok((tid, jid)) = r {
            if let Some(t) = tid {
                ids.insert(t.to_string());
            }
            if let Some(j) = jid {
                ids.insert(j.to_string());
            }
        }
    }
    Ok(ids)
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_watch_item(row: &rusqlite::Row) -> rusqlite::Result<WatchItem> {
    let mt_str: String = row.get("media_type")?;
    let status_str: String = row.get("status")?;
    let done: bool = row.get("done")?;
    Ok(WatchItem {
        id: row.get("id")?,
        text: row.get("text")?,
        media_type: MediaType::from_str(&mt_str),
        status: WatchStatus::from_str(&status_str),
        done,
        total_seasons: row.get("total_seasons")?,
        total_episodes: row.get("total_episodes")?,
        poster_url: row.get("poster_url")?,
        tmdb_id: row.get("tmdb_id")?,
        jikan_id: row.get("jikan_id")?,
        overview: row.get("overview")?,
        trailer_url: row.get("trailer_url")?,
        season_data: row.get("season_data")?,
        created_at: row.get("created_at")?,
        completed_by: row.get("completed_by")?,
        current_season: row.get("current_season").ok().unwrap_or(None),
        current_episode: row.get("current_episode").ok().unwrap_or(None),
        episodes_watched: row.get("episodes_watched").ok().unwrap_or(None),
    })
}

#[server(headers: axum::http::HeaderMap)]
pub async fn list_watchlist() -> Result<Vec<WatchItem>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT w.id, w.text, w.media_type, w.status, w.done, w.total_seasons,
                    w.total_episodes, w.poster_url, w.tmdb_id, w.jikan_id,
                    w.overview, w.trailer_url, w.season_data,
                    w.created_at, w.completed_by,
                    p.current_season, p.current_episode, p.episodes_watched
             FROM watch_items w
             LEFT JOIN (
                 SELECT watch_item_id,
                        MAX(season) as current_season,
                        MAX(CASE WHEN season = (SELECT MAX(season) FROM watch_progress wp2 WHERE wp2.watch_item_id = watch_progress.watch_item_id) THEN episode END) as current_episode,
                        COUNT(*) as episodes_watched
                 FROM watch_progress
                 GROUP BY watch_item_id
             ) p ON p.watch_item_id = w.id
             WHERE w.user_id = ?1
             ORDER BY
                 CASE w.status
                     WHEN 'in_progress' THEN 0
                     WHEN 'unwatched' THEN 1
                     WHEN 'completed' THEN 2
                 END,
                 w.created_at DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id], |row| parse_watch_item(row))
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn add_watchlist(text: String, media_type: MediaType) -> Result<String, ServerFnError> {
    use crate::server::{auth, db, validate};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    validate::text(&text, "text")?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();
    let mt_str = media_type.label();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    conn.execute(
        "INSERT INTO watch_items (id, user_id, text, media_type, done, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, 'unwatched', ?5)",
        rusqlite::params![id, user_id, text, mt_str, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(id)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn toggle_watchlist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let display_name = auth::display_name_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE watch_items SET
         done = 1 - done,
         status = CASE WHEN done = 0 THEN 'completed' ELSE 'unwatched' END,
         completed_by = CASE WHEN done = 0 THEN ?3 ELSE NULL END
         WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id, display_name],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn delete_watchlist(id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM watch_items WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// --- Phase 1: Progress Tracking ---

#[server(headers: axum::http::HeaderMap)]
pub async fn update_watch_progress(
    item_id: String,
    season: i32,
    episode: i32,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let display_name = auth::display_name_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Verify item belongs to user
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM watch_items WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![item_id, user_id],
            |row| row.get(0),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if !exists {
        return Err(ServerFnError::new("Item not found"));
    }

    let progress_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis() as f64;

    // Insert or replace progress for this season/episode
    conn.execute(
        "INSERT OR REPLACE INTO watch_progress (id, watch_item_id, season, episode, watched_at)
         VALUES (
             COALESCE(
                 (SELECT id FROM watch_progress WHERE watch_item_id = ?1 AND season = ?2 AND episode = ?3),
                 ?4
             ),
             ?1, ?2, ?3, ?5
         )",
        rusqlite::params![item_id, season, episode, progress_id, now],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Check if all episodes are watched to auto-complete
    let (total_episodes, episodes_watched): (Option<i32>, i32) = conn
        .query_row(
            "SELECT w.total_episodes,
                    (SELECT COUNT(*) FROM watch_progress WHERE watch_item_id = w.id)
             FROM watch_items w WHERE w.id = ?1",
            rusqlite::params![item_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let new_status = if total_episodes.map(|t| episodes_watched >= t).unwrap_or(false) {
        "completed"
    } else {
        "in_progress"
    };

    let done = if new_status == "completed" { 1 } else { 0 };

    conn.execute(
        "UPDATE watch_items SET status = ?2, done = ?3,
         completed_by = CASE WHEN ?3 = 1 THEN ?4 ELSE completed_by END
         WHERE id = ?1",
        rusqlite::params![item_id, new_status, done, display_name],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn complete_season(
    item_id: String,
    season: i32,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let display_name = auth::display_name_from_headers(&headers);
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Get season_data to know how many episodes this season has
    let (season_data_str, total_episodes_flat): (Option<String>, Option<i32>) = conn
        .query_row(
            "SELECT season_data, total_episodes FROM watch_items WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![item_id, user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let eps_in_season = season_data_str
        .as_ref()
        .and_then(|sd| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(sd).ok())
        .and_then(|map| map.get(&season.to_string())?.as_i64().map(|v| v as i32))
        .unwrap_or_else(|| {
            // Fallback for anime with no season data: use total_episodes
            total_episodes_flat.unwrap_or(12)
        });

    let now = chrono::Utc::now().timestamp_millis() as f64;

    // Insert progress for all episodes in this season inside a transaction
    conn.execute("BEGIN", [])
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    for ep in 1..=eps_in_season {
        let progress_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = conn.execute(
            "INSERT OR IGNORE INTO watch_progress (id, watch_item_id, season, episode, watched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![progress_id, item_id, season, ep, now],
        ) {
            let _ = conn.execute("ROLLBACK", []);
            return Err(ServerFnError::new(e.to_string()));
        }
    }
    conn.execute("COMMIT", [])
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Update status
    let total_watched: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM watch_progress WHERE watch_item_id = ?1",
            rusqlite::params![item_id],
            |row| row.get(0),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let new_status = if total_episodes_flat.map(|t| total_watched >= t).unwrap_or(false) {
        "completed"
    } else {
        "in_progress"
    };

    let done = if new_status == "completed" { 1 } else { 0 };
    conn.execute(
        "UPDATE watch_items SET status = ?2, done = ?3,
         completed_by = CASE WHEN ?3 = 1 THEN ?4 ELSE completed_by END
         WHERE id = ?1",
        rusqlite::params![item_id, new_status, done, display_name],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn set_watch_details(
    item_id: String,
    total_seasons: Option<i32>,
    total_episodes: Option<i32>,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "UPDATE watch_items SET total_seasons = ?2, total_episodes = ?3
         WHERE id = ?1 AND user_id = ?4",
        rusqlite::params![item_id, total_seasons, total_episodes, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// --- Phase 2: External API Integration ---

#[server]
pub async fn search_media(
    query: String,
    media_type: MediaType,
) -> Result<Vec<MediaSearchResult>, ServerFnError> {
    use crate::server::media_api;

    let results = match media_type {
        MediaType::Anime => media_api::jikan_search(&query).await,
        _ => media_api::tmdb_search(&query, &media_type).await,
    }
    .map_err(|e| ServerFnError::new(e))?;

    Ok(results)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn link_external_media(
    item_id: String,
    external_id: String,
    media_type: MediaType,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db, media_api};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    match media_type {
        MediaType::Anime => {
            let mal_id: i32 = external_id
                .parse()
                .map_err(|_| ServerFnError::new("Invalid Jikan ID"))?;
            let details = media_api::jikan_details(mal_id)
                .await
                .map_err(|e| ServerFnError::new(e))?;

            conn.execute(
                "UPDATE watch_items SET jikan_id = ?2, poster_url = ?3,
                 total_episodes = ?4, overview = ?5, trailer_url = ?6
                 WHERE id = ?1 AND user_id = ?7",
                rusqlite::params![item_id, mal_id, details.poster, details.episodes, details.overview, details.trailer_url, user_id],
            )
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        _ => {
            let tmdb_id: i32 = external_id
                .parse()
                .map_err(|_| ServerFnError::new("Invalid TMDB ID"))?;
            let details = media_api::tmdb_details(tmdb_id, &media_type)
                .await
                .map_err(|e| ServerFnError::new(e))?;

            conn.execute(
                "UPDATE watch_items SET tmdb_id = ?2, poster_url = ?3,
                 total_seasons = ?4, total_episodes = ?5, overview = ?6, trailer_url = ?7, season_data = ?8
                 WHERE id = ?1 AND user_id = ?9",
                rusqlite::params![item_id, tmdb_id, details.poster, details.seasons, details.episodes, details.overview, details.trailer_url, details.season_data, user_id],
            )
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_streaming_providers(
    item_id: String,
) -> Result<Vec<StreamingProvider>, ServerFnError> {
    use crate::server::{auth, db, media_api};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let (tmdb_id, jikan_id, mt_str): (Option<i32>, Option<i32>, String) = conn
        .query_row(
            "SELECT tmdb_id, jikan_id, media_type FROM watch_items WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![item_id, user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let media_type = MediaType::from_str(&mt_str);

    if let Some(jikan_id) = jikan_id {
        let details = media_api::jikan_details(jikan_id)
            .await
            .map_err(|e| ServerFnError::new(e))?;
        return Ok(details.streaming);
    }

    if let Some(tmdb_id) = tmdb_id {
        let providers = media_api::tmdb_watch_providers(tmdb_id, &media_type)
            .await
            .map_err(|e| ServerFnError::new(e))?;
        return Ok(providers);
    }

    Ok(vec![])
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_recommendations(
    item_id: String,
) -> Result<Vec<MediaRecommendation>, ServerFnError> {
    use crate::server::{auth, db, media_api};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let (tmdb_id, jikan_id, mt_str): (Option<i32>, Option<i32>, String) = conn
        .query_row(
            "SELECT tmdb_id, jikan_id, media_type FROM watch_items WHERE id = ?1 AND user_id = ?2",
            rusqlite::params![item_id, user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let media_type = MediaType::from_str(&mt_str);

    let mut recs = if let Some(jikan_id) = jikan_id {
        media_api::jikan_recommendations(jikan_id)
            .await
            .map_err(|e| ServerFnError::new(e))?
    } else if let Some(tmdb_id) = tmdb_id {
        media_api::tmdb_recommendations(tmdb_id, &media_type)
            .await
            .map_err(|e| ServerFnError::new(e))?
    } else {
        return Ok(vec![]);
    };

    // Cross-reference with user's existing watch_items
    let existing_ids = load_existing_external_ids(&conn, &user_id)?;

    for rec in &mut recs {
        rec.already_in_list = existing_ids.contains(&rec.external_id);
    }

    Ok(recs)
}

#[server]
pub async fn is_tmdb_configured() -> Result<bool, ServerFnError> {
    use crate::server::media_api;
    Ok(media_api::tmdb_configured())
}

// --- Explore / Trending ---

#[server(headers: axum::http::HeaderMap)]
pub async fn discover_by_genre(media_type: MediaType, genre_id: i32, page: i32, provider_ids: Option<String>) -> Result<Vec<MediaSearchResult>, ServerFnError> {
    use crate::server::{auth, db, media_api};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let provider_ids_ref = provider_ids.as_deref();
    let mut results = media_api::tmdb_discover_by_genre(genre_id, &media_type, page, provider_ids_ref)
        .await
        .map_err(|e| ServerFnError::new(e))?;

    // Filter out items already in user's list
    let existing_ids = load_existing_external_ids(&conn, &user_id)?;
    results.retain(|r| !existing_ids.contains(&r.external_id));

    Ok(results)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_trending(media_type: MediaType, page: i32, provider_ids: Option<String>) -> Result<Vec<MediaSearchResult>, ServerFnError> {
    use crate::server::{auth, db, media_api};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let provider_ids_ref = provider_ids.as_deref();
    let mut results = match media_type {
        MediaType::Anime => media_api::jikan_top_anime(page)
            .await
            .map_err(|e| ServerFnError::new(e))?,
        _ => media_api::tmdb_trending(&media_type, page, provider_ids_ref)
            .await
            .map_err(|e| ServerFnError::new(e))?,
    };

    // Filter out items already in list
    let existing_ids = load_existing_external_ids(&conn, &user_id)?;
    results.retain(|r| !existing_ids.contains(&r.external_id));

    Ok(results)
}

// --- Watch Settings ---

#[server(headers: axum::http::HeaderMap)]
pub async fn get_watch_settings() -> Result<WatchSettings, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let result = conn.query_row(
        "SELECT streaming_providers, filter_by_provider FROM watch_settings WHERE user_id = ?1",
        rusqlite::params![user_id],
        |row| {
            let providers_json: String = row.get(0)?;
            let filter_by: bool = row.get(1)?;
            Ok((providers_json, filter_by))
        },
    );

    match result {
        Ok((providers_json, filter_by_provider)) => {
            let streaming_providers: Vec<String> =
                serde_json::from_str(&providers_json).unwrap_or_default();
            Ok(WatchSettings { streaming_providers, filter_by_provider })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(WatchSettings::default()),
        Err(e) => Err(ServerFnError::new(e.to_string())),
    }
}

#[server(headers: axum::http::HeaderMap)]
pub async fn save_watch_settings(settings: WatchSettings) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let providers_json = serde_json::to_string(&settings.streaming_providers)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let filter_by: i32 = if settings.filter_by_provider { 1 } else { 0 };

    conn.execute(
        "INSERT INTO watch_settings (user_id, streaming_providers, filter_by_provider)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(user_id) DO UPDATE SET
             streaming_providers = excluded.streaming_providers,
             filter_by_provider = excluded.filter_by_provider",
        rusqlite::params![user_id, providers_json, filter_by],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// --- Explore Detail (no DB item needed) ---

#[server]
pub async fn get_explore_detail(
    external_id: String,
    media_type: MediaType,
) -> Result<ExploreDetail, ServerFnError> {
    use crate::server::media_api;

    match media_type {
        MediaType::Anime => {
            let mal_id: i32 = external_id
                .parse()
                .map_err(|_| ServerFnError::new("Invalid ID"))?;
            let details = media_api::jikan_details(mal_id)
                .await
                .map_err(|e| ServerFnError::new(e))?;
            // Fetch anime recommendations (with rate limit delay)
            let recs = media_api::jikan_recommendations(mal_id)
                .await
                .unwrap_or_default()
                .into_iter()
                .take(6)
                .map(|r| MediaSearchResult {
                    external_id: r.external_id,
                    title: r.title,
                    poster_url: r.poster_url,
                    year: r.year,
                    total_seasons: None,
                    total_episodes: None,
                    media_type: MediaType::Anime,
                })
                .collect();
            Ok(ExploreDetail {
                overview: details.overview,
                trailer_url: details.trailer_url,
                providers: details.streaming,
                total_seasons: None,
                total_episodes: details.episodes,
                recommendations: recs,
            })
        }
        _ => {
            let tmdb_id: i32 = external_id
                .parse()
                .map_err(|_| ServerFnError::new("Invalid ID"))?;
            let details = media_api::tmdb_details(tmdb_id, &media_type)
                .await
                .map_err(|e| ServerFnError::new(e))?;
            let providers = media_api::tmdb_watch_providers(tmdb_id, &media_type)
                .await
                .unwrap_or_default();
            let recs = media_api::tmdb_recommendations(tmdb_id, &media_type)
                .await
                .unwrap_or_default()
                .into_iter()
                .take(6)
                .map(|r| MediaSearchResult {
                    external_id: r.external_id,
                    title: r.title,
                    poster_url: r.poster_url,
                    year: r.year,
                    total_seasons: None,
                    total_episodes: None,
                    media_type: media_type.clone(),
                })
                .collect();
            Ok(ExploreDetail {
                overview: details.overview,
                trailer_url: details.trailer_url,
                providers,
                total_seasons: details.seasons,
                total_episodes: details.episodes,
                recommendations: recs,
            })
        }
    }
}

// --- Phase 3: Franchise / Canon Graph ---

#[server(headers: axum::http::HeaderMap)]
pub async fn link_franchise(
    from_id: String,
    to_id: String,
    relation: FranchiseRelation,
) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let id = uuid::Uuid::new_v4().to_string();

    // Get max sort_order for this franchise chain
    let max_order: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM watch_franchise
             WHERE from_item_id = ?1 OR to_item_id = ?1",
            rusqlite::params![from_id],
            |row| row.get(0),
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "INSERT INTO watch_franchise (id, user_id, from_item_id, to_item_id, relation, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, user_id, from_id, to_id, relation.to_string(), max_order + 1],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn unlink_franchise(link_id: String) -> Result<(), ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    conn.execute(
        "DELETE FROM watch_franchise WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![link_id, user_id],
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_franchise_links(item_id: String) -> Result<Vec<FranchiseLink>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut stmt = conn
        .prepare(
            "SELECT f.id, f.from_item_id, f.to_item_id, f.relation, f.sort_order,
                    w.text, w.status
             FROM watch_franchise f
             JOIN watch_items w ON w.id = f.to_item_id
             WHERE (f.from_item_id = ?1 OR f.to_item_id = ?1) AND f.user_id = ?2
             ORDER BY f.sort_order",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let links = stmt
        .query_map(rusqlite::params![item_id, user_id], |row| {
            let rel_str: String = row.get(3)?;
            let status_str: String = row.get(6)?;
            Ok(FranchiseLink {
                id: row.get(0)?,
                from_item_id: row.get(1)?,
                to_item_id: row.get(2)?,
                to_item_title: row.get(5)?,
                to_item_status: WatchStatus::from_str(&status_str),
                relation: FranchiseRelation::from_str(&rel_str),
                sort_order: row.get(4)?,
            })
        })
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(links)
}

#[server(headers: axum::http::HeaderMap)]
pub async fn get_up_next() -> Result<Vec<WatchItem>, ServerFnError> {
    use crate::server::{auth, db};

    let user_id = auth::user_from_headers(&headers).map_err(|e| ServerFnError::new(e))?;
    let conn = db::pool().get().map_err(|e| ServerFnError::new(e.to_string()))?;

    // Get in-progress items + unwatched items that have a completed predecessor
    let mut stmt = conn
        .prepare(
            "SELECT w.id, w.text, w.media_type, w.status, w.done, w.total_seasons,
                    w.total_episodes, w.poster_url, w.tmdb_id, w.jikan_id,
                    w.overview, w.trailer_url, w.season_data,
                    w.created_at, w.completed_by,
                    p.current_season, p.current_episode, p.episodes_watched
             FROM watch_items w
             LEFT JOIN (
                 SELECT watch_item_id,
                        MAX(season) as current_season,
                        MAX(CASE WHEN season = (SELECT MAX(season) FROM watch_progress wp2 WHERE wp2.watch_item_id = watch_progress.watch_item_id) THEN episode END) as current_episode,
                        COUNT(*) as episodes_watched
                 FROM watch_progress
                 GROUP BY watch_item_id
             ) p ON p.watch_item_id = w.id
             WHERE w.user_id = ?1
             AND (
                 w.status = 'in_progress'
                 OR (
                     w.status = 'unwatched'
                     AND w.id IN (
                         SELECT f.to_item_id FROM watch_franchise f
                         JOIN watch_items w2 ON w2.id = f.from_item_id
                         WHERE w2.status = 'completed' AND f.relation = 'sequel'
                     )
                 )
             )
             ORDER BY
                 CASE w.status WHEN 'in_progress' THEN 0 ELSE 1 END,
                 w.created_at DESC",
        )
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let items = stmt
        .query_map(rusqlite::params![user_id], |row| parse_watch_item(row))
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

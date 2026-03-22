use crate::models::{MediaSearchResult, MediaType, StreamingProvider, MediaRecommendation};

static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
fn http_client() -> &'static reqwest::Client {
    HTTP.get_or_init(reqwest::Client::new)
}

fn tmdb_api_key() -> Result<String, String> {
    std::env::var("TMDB_API_KEY").map_err(|_| "TMDB_API_KEY not set".to_string())
}

fn tmdb_region() -> String {
    std::env::var("TMDB_REGION").unwrap_or_else(|_| "TW".to_string())
}

pub fn tmdb_configured() -> bool {
    std::env::var("TMDB_API_KEY").is_ok()
}

fn poster_full_url(path: &str) -> String {
    format!("https://image.tmdb.org/t/p/w200{path}")
}

fn logo_full_url(path: &str) -> String {
    format!("https://image.tmdb.org/t/p/w92{path}")
}

/// Parse a single TMDB result JSON object into a `MediaSearchResult`.
/// Returns `None` if required fields are missing.
fn parse_tmdb_result(r: &serde_json::Value, media_type: &MediaType) -> Option<MediaSearchResult> {
    let id = r["id"].as_i64()?;
    let title = r.get("title").or(r.get("name"))?.as_str()?.to_string();
    let poster = r["poster_path"].as_str().map(poster_full_url);
    let year = r
        .get("release_date")
        .or(r.get("first_air_date"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.get(..4))
        .map(|y| y.to_string());

    Some(MediaSearchResult {
        external_id: id.to_string(),
        title,
        poster_url: poster,
        year,
        total_seasons: None,
        total_episodes: None,
        media_type: media_type.clone(),
    })
}

// --- TMDB ---

pub async fn tmdb_search(query: &str, media_type: &MediaType) -> Result<Vec<MediaSearchResult>, String> {
    let key = tmdb_api_key()?;
    let endpoint = match media_type {
        MediaType::Movie => "movie",
        _ => "tv",
    };

    let resp: serde_json::Value = http_client()
        .get(format!("https://api.themoviedb.org/3/search/{endpoint}"))
        .query(&[("api_key", key.as_str()), ("query", query)])
        .send()
        .await
        .map_err(|e| format!("TMDB search failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    let results = resp["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(8)
        .filter_map(|r| parse_tmdb_result(r, media_type))
        .collect();

    Ok(results)
}

pub struct TmdbDetailsResult {
    pub poster: Option<String>,
    pub seasons: Option<i32>,
    pub episodes: Option<i32>,
    pub overview: Option<String>,
    pub trailer_url: Option<String>,
    /// JSON: {"1": 13, "2": 22, ...}
    pub season_data: Option<String>,
}

pub async fn tmdb_details(tmdb_id: i32, media_type: &MediaType) -> Result<TmdbDetailsResult, String> {
    let key = tmdb_api_key()?;
    let endpoint = match media_type {
        MediaType::Movie => format!("https://api.themoviedb.org/3/movie/{tmdb_id}"),
        _ => format!("https://api.themoviedb.org/3/tv/{tmdb_id}"),
    };

    let resp: serde_json::Value = http_client()
        .get(&endpoint)
        .query(&[("api_key", key.as_str()), ("append_to_response", "videos")])
        .send()
        .await
        .map_err(|e| format!("TMDB details failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    let poster = resp["poster_path"].as_str().map(poster_full_url);
    let overview = resp["overview"].as_str().map(|s| s.to_string());

    let trailer_url = resp["videos"]["results"]
        .as_array()
        .and_then(|vids| {
            vids.iter()
                .find(|v| v["type"].as_str() == Some("Trailer") && v["site"].as_str() == Some("YouTube"))
                .or_else(|| vids.iter().find(|v| v["site"].as_str() == Some("YouTube")))
        })
        .and_then(|v| v["key"].as_str())
        .map(|key| format!("https://youtube.com/watch?v={key}"));

    let (seasons, episodes, season_data) = match media_type {
        MediaType::Movie => (None, None, None),
        _ => {
            let s = resp["number_of_seasons"].as_i64().map(|v| v as i32);
            let e = resp["number_of_episodes"].as_i64().map(|v| v as i32);
            // Build season_data JSON from seasons array
            let sd = resp["seasons"].as_array().map(|seasons_arr| {
                let mut map = serde_json::Map::new();
                for season in seasons_arr {
                    let sn = season["season_number"].as_i64().unwrap_or(0);
                    let ec = season["episode_count"].as_i64().unwrap_or(0);
                    if sn > 0 {
                        map.insert(sn.to_string(), serde_json::Value::Number(ec.into()));
                    }
                }
                serde_json::Value::Object(map).to_string()
            });
            (s, e, sd)
        }
    };

    Ok(TmdbDetailsResult { poster, seasons, episodes, overview, trailer_url, season_data })
}

pub async fn tmdb_watch_providers(tmdb_id: i32, media_type: &MediaType) -> Result<Vec<StreamingProvider>, String> {
    let key = tmdb_api_key()?;
    let endpoint = match media_type {
        MediaType::Movie => format!("https://api.themoviedb.org/3/movie/{tmdb_id}/watch/providers"),
        _ => format!("https://api.themoviedb.org/3/tv/{tmdb_id}/watch/providers"),
    };

    let region = tmdb_region();
    let resp: serde_json::Value = http_client()
        .get(&endpoint)
        .query(&[("api_key", key.as_str())])
        .send()
        .await
        .map_err(|e| format!("TMDB providers failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    let mut providers = Vec::new();
    if let Some(country) = resp["results"][&region].as_object() {
        let justwatch_link = country.get("link").and_then(|v| v.as_str()).map(|s| s.to_string());
        for (ptype, label) in [("flatrate", "flatrate"), ("rent", "rent"), ("buy", "buy")] {
            if let Some(list) = country.get(ptype).and_then(|v| v.as_array()) {
                for p in list {
                    if let (Some(name), Some(logo)) = (
                        p["provider_name"].as_str(),
                        p["logo_path"].as_str(),
                    ) {
                        providers.push(StreamingProvider {
                            name: name.to_string(),
                            logo_url: logo_full_url(logo),
                            provider_type: label.to_string(),
                            link: justwatch_link.clone(),
                        });
                    }
                }
            }
        }
    }

    // Deduplicate by name (keep first occurrence, which is flatrate > rent > buy)
    let mut seen = std::collections::HashSet::new();
    providers.retain(|p| seen.insert(p.name.clone()));

    Ok(providers)
}

pub async fn tmdb_recommendations(tmdb_id: i32, media_type: &MediaType) -> Result<Vec<MediaRecommendation>, String> {
    let key = tmdb_api_key()?;
    let endpoint = match media_type {
        MediaType::Movie => format!("https://api.themoviedb.org/3/movie/{tmdb_id}/recommendations"),
        _ => format!("https://api.themoviedb.org/3/tv/{tmdb_id}/recommendations"),
    };

    let resp: serde_json::Value = http_client()
        .get(&endpoint)
        .query(&[("api_key", key.as_str())])
        .send()
        .await
        .map_err(|e| format!("TMDB recs failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    // Re-use parse_tmdb_result then convert to MediaRecommendation
    let media_type_placeholder = MediaType::Movie; // recs don't need type, just parse fields
    let results = resp["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(10)
        .filter_map(|r| {
            let parsed = parse_tmdb_result(r, &media_type_placeholder)?;
            Some(MediaRecommendation {
                external_id: parsed.external_id,
                title: parsed.title,
                poster_url: parsed.poster_url,
                year: parsed.year,
                already_in_list: false, // will be set by the server function
            })
        })
        .collect();

    Ok(results)
}

pub async fn tmdb_trending(media_type: &MediaType, page: i32, provider_ids: Option<&str>) -> Result<Vec<MediaSearchResult>, String> {
    let key = tmdb_api_key()?;
    let region = tmdb_region();

    // When filtering by providers we use the discover endpoint (supports with_watch_providers).
    // The trending endpoint doesn't support that parameter.
    let (endpoint, extra_params): (&str, Vec<(&str, String)>) = if provider_ids.is_some() {
        let ep = match media_type {
            MediaType::Movie => "https://api.themoviedb.org/3/discover/movie",
            _ => "https://api.themoviedb.org/3/discover/tv",
        };
        let mut params = vec![
            ("sort_by", "popularity.desc".to_string()),
        ];
        if let Some(pids) = provider_ids {
            params.push(("with_watch_providers", pids.to_string()));
            params.push(("watch_region", region.clone()));
        }
        (ep, params)
    } else {
        let ep = match media_type {
            MediaType::Movie => "https://api.themoviedb.org/3/trending/movie/week",
            _ => "https://api.themoviedb.org/3/trending/tv/week",
        };
        (ep, vec![])
    };

    let mut query: Vec<(&str, String)> = vec![
        ("api_key", key.clone()),
        ("page", page.to_string()),
    ];
    query.extend(extra_params);

    let resp: serde_json::Value = http_client()
        .get(endpoint)
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("TMDB trending failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    let results = resp["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(20)
        .filter_map(|r| parse_tmdb_result(r, media_type))
        .collect();

    Ok(results)
}

pub async fn jikan_top_anime(page: i32) -> Result<Vec<MediaSearchResult>, String> {
    let resp: serde_json::Value = http_client()
        .get("https://api.jikan.moe/v4/top/anime")
        .query(&[("filter", "airing"), ("limit", "20"), ("page", &page.to_string())])
        .send()
        .await
        .map_err(|e| format!("Jikan top failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Jikan parse failed: {e}"))?;

    let results = resp["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|r| {
            let id = r["mal_id"].as_i64()?;
            let title = r["title"].as_str()?.to_string();
            let poster = r["images"]["jpg"]["image_url"].as_str().map(|s| s.to_string());
            let year = r["year"].as_i64().map(|y| y.to_string());

            Some(MediaSearchResult {
                external_id: id.to_string(),
                title,
                poster_url: poster,
                year,
                total_seasons: None,
                total_episodes: None,
                media_type: MediaType::Anime,
            })
        })
        .collect();

    Ok(results)
}

pub async fn tmdb_discover_by_genre(genre_id: i32, media_type: &MediaType, page: i32, provider_ids: Option<&str>) -> Result<Vec<MediaSearchResult>, String> {
    let key = tmdb_api_key()?;
    let region = tmdb_region();
    let endpoint = match media_type {
        MediaType::Movie => "https://api.themoviedb.org/3/discover/movie",
        _ => "https://api.themoviedb.org/3/discover/tv",
    };

    let mut query: Vec<(&str, String)> = vec![
        ("api_key", key.clone()),
        ("with_genres", genre_id.to_string()),
        ("sort_by", "popularity.desc".to_string()),
        ("page", page.to_string()),
    ];
    if let Some(pids) = provider_ids {
        query.push(("with_watch_providers", pids.to_string()));
        query.push(("watch_region", region));
    }

    let resp: serde_json::Value = http_client()
        .get(endpoint)
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("TMDB discover failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("TMDB parse failed: {e}"))?;

    let results = resp["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(20)
        .filter_map(|r| parse_tmdb_result(r, media_type))
        .collect();

    Ok(results)
}

// --- Jikan (MyAnimeList) ---

pub async fn jikan_search(query: &str) -> Result<Vec<MediaSearchResult>, String> {
    let resp: serde_json::Value = http_client()
        .get("https://api.jikan.moe/v4/anime")
        .query(&[("q", query), ("limit", "8")])
        .send()
        .await
        .map_err(|e| format!("Jikan search failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Jikan parse failed: {e}"))?;

    let results = resp["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|r| {
            let id = r["mal_id"].as_i64()?;
            let title = r["title"].as_str()?.to_string();
            let poster = r["images"]["jpg"]["image_url"].as_str().map(|s| s.to_string());
            let year = r["year"].as_i64().map(|y| y.to_string());
            let episodes = r["episodes"].as_i64().map(|e| e as i32);

            Some(MediaSearchResult {
                external_id: id.to_string(),
                title,
                poster_url: poster,
                year,
                total_seasons: None,
                total_episodes: episodes,
                media_type: MediaType::Anime,
            })
        })
        .collect();

    Ok(results)
}

pub struct JikanDetailsResult {
    pub poster: Option<String>,
    pub episodes: Option<i32>,
    pub overview: Option<String>,
    pub trailer_url: Option<String>,
    pub streaming: Vec<StreamingProvider>,
}

pub async fn jikan_details(mal_id: i32) -> Result<JikanDetailsResult, String> {
    let resp: serde_json::Value = http_client()
        .get(format!("https://api.jikan.moe/v4/anime/{mal_id}"))
        .send()
        .await
        .map_err(|e| format!("Jikan details failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Jikan parse failed: {e}"))?;

    let data = &resp["data"];
    let poster = data["images"]["jpg"]["image_url"].as_str().map(|s| s.to_string());
    let episodes = data["episodes"].as_i64().map(|e| e as i32);
    let overview = data["synopsis"].as_str().map(|s| s.to_string());

    // Jikan provides embed URL; convert to normal YouTube link
    let trailer_url = data["trailer"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            data["trailer"]["embed_url"]
                .as_str()
                .and_then(|embed| {
                    // Extract video ID from embed URL
                    embed.split('/').last()
                        .and_then(|s| s.split('?').next())
                        .map(|id| format!("https://youtube.com/watch?v={id}"))
                })
        });

    let streaming = data["streaming"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|s| {
            let name = s["name"].as_str()?.to_string();
            let url = s["url"].as_str().map(|u| u.to_string());
            Some(StreamingProvider {
                name,
                logo_url: String::new(),
                provider_type: "flatrate".to_string(),
                link: url,
            })
        })
        .collect();

    Ok(JikanDetailsResult { poster, episodes, overview, trailer_url, streaming })
}

pub async fn jikan_recommendations(mal_id: i32) -> Result<Vec<MediaRecommendation>, String> {
    // Rate limit: Jikan allows 3 req/sec
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;

    let resp: serde_json::Value = http_client()
        .get(format!("https://api.jikan.moe/v4/anime/{mal_id}/recommendations"))
        .send()
        .await
        .map_err(|e| format!("Jikan recs failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Jikan parse failed: {e}"))?;

    let results = resp["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(10)
        .filter_map(|r| {
            let entry = &r["entry"];
            let id = entry["mal_id"].as_i64()?;
            let title = entry["title"].as_str()?.to_string();
            let poster = entry["images"]["jpg"]["image_url"].as_str().map(|s| s.to_string());

            Some(MediaRecommendation {
                external_id: id.to_string(),
                title,
                poster_url: poster,
                year: None,
                already_in_list: false,
            })
        })
        .collect();

    Ok(results)
}

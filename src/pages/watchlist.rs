use dioxus::prelude::*;

use crate::api::watchlist as api;
use crate::cache::{self, SyncStatus};
use crate::components::error_banner::ErrorBanner;
use crate::components::layout::SyncTrigger;
use crate::components::progress_bar::ProgressBar;
use crate::components::swipe_item::SwipeItem;
use crate::models::{
    ExploreDetail, FranchiseLink, FranchiseRelation, MediaRecommendation, MediaSearchResult,
    MediaType, StreamingProvider, WatchItem, WatchSettings, WatchStatus,
};
use crate::pages::watch_settings::provider_ids_param;
use crate::route::Route;

/// Compute the next season/episode to watch based on season_data JSON
fn compute_next_episode(
    current_season: Option<i32>,
    current_episode: Option<i32>,
    season_data: &Option<String>,
) -> (i32, i32) {
    let cur_s = current_season.unwrap_or(0);
    let cur_e = current_episode.unwrap_or(0);

    // No progress yet
    if cur_s == 0 && cur_e == 0 {
        return (1, 1);
    }

    // Try to parse season_data to know episodes per season
    if let Some(ref sd) = season_data {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(sd) {
            let eps_in_season = map
                .get(&cur_s.to_string())
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .unwrap_or(i32::MAX);

            if cur_e >= eps_in_season {
                // Season complete — advance to next season episode 1
                let next_s = cur_s + 1;
                let has_next = map.contains_key(&next_s.to_string());
                if has_next {
                    return (next_s, 1);
                }
                // No more seasons — stay at current (series complete)
                return (cur_s, cur_e + 1);
            }
        }
    }

    // No season_data or still within season — just increment episode
    (cur_s, cur_e + 1)
}

#[derive(Clone, PartialEq)]
enum FilterTab {
    All,
    UpNext,
    Explore,
}

#[component]
pub fn Watchlist() -> Element {
    let mut items = use_signal(Vec::<WatchItem>::new);
    let mut input_text = use_signal(String::new);
    let selected_type = use_signal(|| MediaType::Movie);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut sync_status: Signal<SyncStatus> = use_context();
    let sync_trigger: Signal<SyncTrigger> = use_context();
    let active_filter = use_signal(|| FilterTab::All);
    let expanded_id = use_signal(|| Option::<String>::None);
    let mut search_results = use_signal(Vec::<MediaSearchResult>::new);
    let mut searching = use_signal(|| false);
    let explore_results = use_signal(Vec::<MediaSearchResult>::new);
    let explore_type = use_signal(|| MediaType::Movie);
    let watch_settings = use_signal(WatchSettings::default);
    let mut finished_recs = use_signal(Vec::<(String, MediaType, Vec<MediaRecommendation>)>::new);
    let mut finished_recs_loaded = use_signal(|| false);

    let reload = move || {
        spawn(async move {
            sync_status.set(SyncStatus::Syncing);
            // Load based on active filter
            let result = {
                let filter = active_filter.read().clone();
                match filter {
                    FilterTab::UpNext => api::get_up_next().await,
                    _ => api::list_watchlist().await,
                }
            };
            match result {
                Ok(loaded) => {
                    cache::write("watchlist", &loaded);
                    cache::write_sync_time();
                    items.set(loaded);
                    sync_status.set(SyncStatus::Synced);
                }
                Err(e) => {
                    if items.read().is_empty() {
                        error_msg.set(Some(format!("Failed to load: {e}")));
                    }
                    sync_status.set(SyncStatus::CachedOnly);
                }
            }

            // Load finished recommendations for Up Next tab
            if *active_filter.read() == FilterTab::UpNext {
                if let Ok(recs) = api::get_finished_recommendations().await {
                    finished_recs.set(recs);
                }
                finished_recs_loaded.set(true);
            }
        });
    };

    use_effect(move || {
        if let Some(cached) = cache::read::<Vec<WatchItem>>("watchlist") {
            items.set(cached);
        }
        reload();
        // Load watch settings in background
        let mut ws = watch_settings;
        spawn(async move {
            if let Ok(s) = api::get_watch_settings().await {
                ws.set(s);
            }
        });
    });

    use_effect(move || {
        let _trigger = sync_trigger.read().0;
        reload();
    });

    let mut do_search = move |query: String, media_type: MediaType| {
        if query.trim().len() < 2 {
            search_results.set(vec![]);
            return;
        }
        searching.set(true);
        spawn(async move {
            match api::search_media(query, media_type).await {
                Ok(results) => search_results.set(results),
                Err(_) => search_results.set(vec![]),
            }
            searching.set(false);
        });
    };

    let add_item = move |text: String| {
        if text.trim().is_empty() {
            return;
        }
        let media_type = selected_type.read().clone();
        spawn(async move {
            match api::add_watchlist(text, media_type).await {
                Ok(_id) => {
                    input_text.set(String::new());
                    search_results.set(vec![]);
                    reload();
                }
                Err(e) => error_msg.set(Some(format!("Failed to add: {e}"))),
            }
        });
    };

    let add_from_search = move |result: MediaSearchResult| {
        let media_type = result.media_type.clone();
        let title = result.title.clone();
        let external_id = result.external_id.clone();
        spawn(async move {
            match api::add_watchlist(title, media_type.clone()).await {
                Ok(new_id) => {
                    input_text.set(String::new());
                    search_results.set(vec![]);
                    let _ = api::link_external_media(new_id, external_id, media_type).await;
                    reload();
                }
                Err(e) => error_msg.set(Some(format!("Failed to add: {e}"))),
            }
        });
    };

    // Filter items based on active tab
    let filtered_items: Vec<WatchItem> = {
        let filter = active_filter.read().clone();
        if filter == FilterTab::Explore {
            vec![]
        } else {
            items
                .read()
                .iter()
                .filter(|_| match filter {
                    FilterTab::All => true,
                    FilterTab::UpNext => true, // Already filtered by server
                    FilterTab::Explore => unreachable!(),
                })
                .cloned()
                .collect()
        }
    };

    rsx! {
        div { class: "p-4 pb-20 space-y-4",
            ErrorBanner { message: error_msg }

            // Filter tabs row (with settings gear on the right)
            div { class: "flex items-center gap-1.5",
                div { class: "flex-1 flex gap-1.5 bg-cyber-card/50 rounded-lg p-1",
                    {render_filter_tab("All", FilterTab::All, active_filter.clone(), reload)}
                    {render_filter_tab("Up Next", FilterTab::UpNext, active_filter.clone(), reload)}
                    {render_filter_tab("Explore", FilterTab::Explore, active_filter.clone(), reload)}
                }
                Link {
                    to: Route::WatchSettings {},
                    class: "w-11 h-11 flex items-center justify-center text-cyber-dim hover:text-neon-cyan transition-colors flex-shrink-0",
                    title: "Watch Settings",
                    svg {
                        class: "w-4 h-4",
                        xmlns: "http://www.w3.org/2000/svg",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "2",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z",
                        }
                        circle {
                            cx: "12",
                            cy: "12",
                            r: "3",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                }
            }

            // Show provider filter indicator when active
            if watch_settings.read().filter_by_provider && !watch_settings.read().streaming_providers.is_empty() {
                div { class: "flex items-center gap-1.5 px-1",
                    div { class: "w-1.5 h-1.5 rounded-full bg-neon-cyan animate-pulse" }
                    span { class: "text-[9px] text-neon-cyan tracking-wider",
                        {format!("FILTERED: {}", watch_settings.read().streaming_providers.join(", "))}
                    }
                }
            }

            if *active_filter.read() == FilterTab::Explore {
                // Explore view
                ExploreSection {
                    results: explore_results,
                    explore_type: explore_type,
                    reload: move |_| reload(),
                    error_msg: error_msg,
                    watch_settings: watch_settings,
                }
            } else {
                // Add form
                div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4",
                    form {
                        class: "space-y-3",
                        onsubmit: move |e| {
                            e.prevent_default();
                            let text = input_text.read().clone();
                            add_item(text);
                        },
                        input {
                            class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2.5 text-sm text-cyber-text outline-none focus:border-neon-purple/60 font-mono",
                            r#type: "text",
                            placeholder: "Search or add to watchlist...",
                            value: "{input_text}",
                            oninput: move |e| {
                                let val = e.value();
                                input_text.set(val.clone());
                                let mt = selected_type.read().clone();
                                do_search(val, mt);
                            },
                        }
                        div { class: "flex gap-2",
                            for mt in MediaType::all() {
                                {render_type_chip(mt.clone(), selected_type)}
                            }
                        }

                        if !search_results.read().is_empty() {
                            div { class: "bg-cyber-dark border border-cyber-border rounded-lg overflow-hidden max-h-64 overflow-y-auto",
                                for result in search_results.read().iter() {
                                    {render_search_result(result.clone(), add_from_search)}
                                }
                            }
                        }

                        if *searching.read() {
                            div { class: "text-center py-2",
                                span { class: "text-[10px] text-neon-cyan tracking-wider animate-pulse", "SEARCHING..." }
                            }
                        }

                        if search_results.read().is_empty() && !input_text.read().is_empty() {
                            button {
                                class: "w-full bg-neon-purple/10 text-neon-purple/70 border border-neon-purple/20 rounded-lg px-4 py-2 text-xs font-bold tracking-wider uppercase hover:bg-neon-purple/20 transition-colors",
                                r#type: "submit",
                                "ADD MANUALLY"
                            }
                        }
                    }
                }

                // Items list
                div { class: "space-y-0",
                    for item in filtered_items.iter() {
                        {render_item(item.clone(), filtered_items.clone(), reload, error_msg, expanded_id)}
                    }
                    if filtered_items.is_empty() {
                        div { class: "text-center py-16",
                            p { class: "text-2xl mb-3 opacity-30", "\u{1F3AC}" }
                            p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim",
                                {match *active_filter.read() {
                                    FilterTab::UpNext => "No recommendations yet — start watching something!",
                                    FilterTab::All => "Nothing to watch yet",
                                    FilterTab::Explore => "",
                                }}
                            }
                            if *active_filter.read() == FilterTab::All {
                                p { class: "text-[10px] text-cyber-dim/40 mt-2 tracking-wider",
                                    "SWIPE \u{2192} WATCHED \u{2022} SWIPE \u{2190} DELETE \u{2022} TAP TO EXPAND"
                                }
                            }
                        }
                    }
                    if !filtered_items.is_empty() && *active_filter.read() == FilterTab::All {
                        p { class: "text-center text-[9px] text-cyber-dim/30 tracking-widest mt-2 pb-1",
                            "\u{2190} DELETE \u{2022} TAP EXPAND \u{2022} SWIPE \u{2192}"
                        }
                    }
                }

                // Recommendations from finished series (Up Next tab only)
                if *active_filter.read() == FilterTab::UpNext && *finished_recs_loaded.read() && !finished_recs.read().is_empty() {
                    div { class: "mt-6 space-y-4",
                        p { class: "text-[10px] text-cyber-dim tracking-[0.3em] uppercase", "BECAUSE YOU FINISHED" }
                        for (source_title, media_type, recs) in finished_recs.read().iter() {
                            div { class: "bg-cyber-card/50 border border-cyber-border/50 rounded-xl p-3 space-y-2",
                                p { class: "text-xs text-neon-cyan font-bold tracking-wider truncate", "{source_title}" }
                                div { class: "space-y-1",
                                    for rec in recs.iter() {
                                        {render_recommendation(rec.clone(), media_type.clone(), reload, error_msg)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_filter_tab(
    label: &'static str,
    tab: FilterTab,
    mut active: Signal<FilterTab>,
    reload: impl Fn() + Copy + 'static,
) -> Element {
    let is_active = *active.read() == tab;
    let bg = if is_active {
        "bg-neon-cyan/20 text-neon-cyan border-neon-cyan/40"
    } else {
        "text-cyber-dim border-transparent hover:text-cyber-text"
    };

    rsx! {
        button {
            class: "flex-1 px-1 py-3.5 rounded-md text-[9px] font-bold tracking-wide uppercase border {bg} transition-colors",
            onclick: move |_| {
                active.set(tab.clone());
                reload();
            },
            "{label}"
        }
    }
}

const GENRE_CHIPS: &[(&str, i32)] = &[
    ("Action", 28),
    ("Comedy", 35),
    ("Drama", 18),
    ("Sci-Fi", 878),
    ("Horror", 27),
    ("Thriller", 53),
    ("Romance", 10749),
    ("Fantasy", 14),
    ("Crime", 80),
    ("Animation", 16),
];

#[component]
fn ExploreSection(
    mut results: Signal<Vec<MediaSearchResult>>,
    explore_type: Signal<MediaType>,
    reload: EventHandler<()>,
    mut error_msg: Signal<Option<String>>,
    watch_settings: Signal<WatchSettings>,
) -> Element {
    let reload = move || { reload.call(()); };
    let mut loading = use_signal(|| false);
    let mut loading_more = use_signal(|| false);
    let mut active_genre = use_signal(|| Option::<i32>::None);
    let mut breadcrumb = use_signal(|| Option::<String>::None);
    let mut current_page = use_signal(|| 1_i32);
    let mut has_more = use_signal(|| true);

    let mut load_content = move |mt: MediaType, genre: Option<i32>| {
        loading.set(true);
        active_genre.set(genre);
        breadcrumb.set(None);
        current_page.set(1);
        has_more.set(true);
        let provider_ids = provider_ids_param(&watch_settings.read());
        spawn(async move {
            let r = if let Some(gid) = genre {
                api::discover_by_genre(mt, gid, 1, provider_ids).await
            } else {
                api::get_trending(mt, 1, provider_ids).await
            };
            match r {
                Ok(items) => {
                    has_more.set(!items.is_empty());
                    results.set(items);
                }
                Err(e) => error_msg.set(Some(format!("Failed to load: {e}"))),
            }
            loading.set(false);
        });
    };

    let mut load_more = move || {
        if *loading_more.read() || !*has_more.read() || breadcrumb.read().is_some() {
            return;
        }
        loading_more.set(true);
        let next_page = *current_page.read() + 1;
        let mt = explore_type.read().clone();
        let genre = *active_genre.read();
        let provider_ids = provider_ids_param(&watch_settings.read());
        spawn(async move {
            let r = if let Some(gid) = genre {
                api::discover_by_genre(mt, gid, next_page, provider_ids).await
            } else {
                api::get_trending(mt, next_page, provider_ids).await
            };
            match r {
                Ok(new_items) => {
                    if new_items.is_empty() {
                        has_more.set(false);
                    } else {
                        current_page.set(next_page);
                        results.write().extend(new_items);
                    }
                }
                Err(_) => has_more.set(false),
            }
            loading_more.set(false);
        });
    };

    use_effect({
        let mt = explore_type.read().clone();
        move || {
            load_content(mt.clone(), None);
        }
    });

    rsx! {
        div { class: "space-y-3",
            // Type selector
            div { class: "flex gap-2",
                for mt in MediaType::all() {
                    {render_explore_type_chip(mt.clone(), explore_type, active_genre, load_content)}
                }
            }

            // Genre chips (scrollable, not for Anime since TMDB genres don't apply)
            if *explore_type.read() != MediaType::Anime {
                div { class: "relative",
                    div { class: "flex gap-1.5 overflow-x-auto scrollbar-hide pb-1 pr-8",
                        for (name, gid) in GENRE_CHIPS.iter() {
                            {render_genre_chip(name, *gid, active_genre, explore_type, load_content)}
                        }
                    }
                    div { class: "absolute right-0 top-0 bottom-0 w-8 bg-gradient-to-l from-cyber-black to-transparent pointer-events-none" }
                }
            }

            // Breadcrumb for "similar to X"
            if let Some(ref title) = *breadcrumb.read() {
                div { class: "flex items-center gap-2",
                    button {
                        class: "text-[10px] text-neon-cyan hover:text-neon-cyan/80 tracking-wider",
                        onclick: {
                            let mt = explore_type.read().clone();
                            let genre = *active_genre.read();
                            move |_| {
                                load_content(mt.clone(), genre);
                            }
                        },
                        "\u{2190} BACK"
                    }
                    span { class: "text-[10px] text-cyber-dim tracking-wider", "SIMILAR TO" }
                    span { class: "text-[10px] text-neon-orange font-bold tracking-wider truncate", "{title}" }
                }
            }

            if *loading.read() {
                div { class: "text-center py-8",
                    span { class: "text-[10px] text-neon-cyan tracking-wider animate-pulse", "LOADING..." }
                }
            }

            // Results grid
            if !results.read().is_empty() {
                div { class: "grid grid-cols-3 gap-2",
                    for result in results.read().iter() {
                        ExploreCard {
                            key: "{result.external_id}",
                            result: result.clone(),
                            reload: move |_| reload(),
                            error_msg: error_msg,
                            breadcrumb: breadcrumb,
                            explore_results: results,
                            has_more: has_more,
                        }
                    }
                }
            }

            // Load more button
            if *has_more.read() && !results.read().is_empty() && !*loading.read() {
                if *loading_more.read() {
                    div { class: "text-center py-4",
                        span { class: "text-[10px] text-neon-cyan tracking-wider animate-pulse", "LOADING MORE..." }
                    }
                } else {
                    button {
                        class: "w-full py-3 text-[10px] text-neon-cyan font-bold tracking-wider uppercase border border-neon-cyan/20 rounded-lg hover:bg-neon-cyan/10 transition-colors",
                        onclick: move |_| load_more(),
                        "LOAD MORE"
                    }
                }
            }

            if results.read().is_empty() && !*loading.read() {
                div { class: "text-center py-8",
                    p { class: "text-xs text-cyber-dim tracking-wider", "NO RESULTS" }
                }
            }
        }
    }
}

fn render_explore_type_chip(
    mt: MediaType,
    mut selected: Signal<MediaType>,
    mut active_genre: Signal<Option<i32>>,
    mut load_content: impl FnMut(MediaType, Option<i32>) + Copy + 'static,
) -> Element {
    let is_active = *selected.read() == mt;
    let label = mt.label();
    let bg = if is_active {
        "bg-neon-orange/30 text-neon-orange border-neon-orange/60"
    } else {
        "bg-cyber-dark text-cyber-dim border-cyber-border"
    };

    rsx! {
        button {
            class: "flex-1 px-4 py-2.5 rounded-lg text-xs font-medium tracking-wider uppercase border {bg} transition-colors",
            r#type: "button",
            onclick: move |_| {
                selected.set(mt.clone());
                active_genre.set(None);
                load_content(mt.clone(), None);
            },
            "{label}"
        }
    }
}

fn render_genre_chip(
    name: &'static str,
    genre_id: i32,
    active_genre: Signal<Option<i32>>,
    explore_type: Signal<MediaType>,
    mut load_content: impl FnMut(MediaType, Option<i32>) + Copy + 'static,
) -> Element {
    let is_active = *active_genre.read() == Some(genre_id);
    let bg = if is_active {
        "bg-neon-purple/30 text-neon-purple border-neon-purple/60"
    } else {
        "bg-cyber-dark/80 text-cyber-dim border-cyber-border/50 hover:text-cyber-text"
    };

    rsx! {
        button {
            class: "px-3 py-2.5 rounded-full text-[9px] font-bold tracking-wider uppercase border whitespace-nowrap {bg} transition-colors flex-shrink-0",
            onclick: move |_| {
                let mt = explore_type.read().clone();
                load_content(mt, Some(genre_id));
            },
            "{name}"
        }
    }
}

#[component]
fn ExploreCard(
    result: MediaSearchResult,
    reload: EventHandler<()>,
    error_msg: Signal<Option<String>>,
    mut breadcrumb: Signal<Option<String>>,
    mut explore_results: Signal<Vec<MediaSearchResult>>,
    mut has_more: Signal<bool>,
) -> Element {
    let mut error_msg = error_msg;
    let reload = move || { reload.call(()); };
    let has_poster = result.poster_url.is_some();
    let poster = result.poster_url.clone().unwrap_or_default();
    let title = result.title.clone();
    let title_add = result.title.clone();
    let title_similar = result.title.clone();
    let ext_id_add = result.external_id.clone();
    let ext_id_detail = result.external_id.clone();
    let mt_add = result.media_type.clone();
    let mt_detail = result.media_type.clone();
    let year_str = result.year.as_deref().unwrap_or("").to_string();

    let mut flipped = use_signal(|| false);
    let mut detail = use_signal(|| Option::<ExploreDetail>::None);
    let mut loading_detail = use_signal(|| false);
    let mut added = use_signal(|| false);

    let flip_class = if *flipped.read() { "flipped" } else { "" };

    rsx! {
        div { class: "flip-card",
            div { class: "flip-card-inner {flip_class}",
                // FRONT — poster
                div { class: "flip-card-front bg-cyber-card/80 border border-cyber-border rounded-lg overflow-hidden cursor-pointer",
                    onclick: move |_| {
                        flipped.set(true);
                        if detail.read().is_none() {
                            loading_detail.set(true);
                            let ext_id = ext_id_detail.clone();
                            let mt = mt_detail.clone();
                            spawn(async move {
                                match api::get_explore_detail(ext_id, mt).await {
                                    Ok(d) => detail.set(Some(d)),
                                    Err(_) => {}
                                }
                                loading_detail.set(false);
                            });
                        }
                    },
                    if has_poster {
                        img {
                            class: "w-full aspect-[2/3] object-cover",
                            src: "{poster}",
                        }
                    } else {
                        div { class: "w-full aspect-[2/3] bg-cyber-card border border-cyber-border/40 flex flex-col items-center justify-center gap-1",
                            span { class: "text-2xl opacity-20", "\u{1F3AC}" }
                            span { class: "text-[8px] text-cyber-dim/60 tracking-wider", "NO POSTER" }
                        }
                    }
                    div { class: "p-1.5",
                        p { class: "text-[10px] text-cyber-text font-medium truncate", "{result.title}" }
                        if !year_str.is_empty() {
                            p { class: "text-[8px] text-cyber-dim", "{year_str}" }
                        }
                    }
                }

                // BACK — details
                div { class: "flip-card-back bg-cyber-card border border-neon-cyan/30 rounded-lg overflow-hidden cursor-pointer",
                    div { class: "p-2 h-full flex flex-col",
                        // Title + close
                        div { class: "flex items-start justify-between gap-1 mb-1",
                            p { class: "text-[10px] text-neon-cyan font-bold tracking-wider leading-tight flex-1", "{title}" }
                            button {
                                class: "text-xs text-cyber-dim hover:text-cyber-text p-1 -mr-1 -mt-0.5 flex-shrink-0",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    flipped.set(false);
                                },
                                "\u{2715}"
                            }
                        }

                        if *loading_detail.read() {
                            div { class: "flex-1 flex items-center justify-center",
                                span { class: "text-[9px] text-neon-cyan tracking-wider animate-pulse", "LOADING..." }
                            }
                        }

                        if let Some(ref d) = *detail.read() {
                            // Synopsis (capped height so action buttons stay visible)
                            if let Some(ref overview) = d.overview {
                                div { class: "max-h-20 overflow-y-auto mb-1.5 scrollbar-hide",
                                    p { class: "text-[9px] text-cyber-text/70 leading-relaxed", "{overview}" }
                                }
                            }

                            // Trailer
                            if let Some(ref trailer) = d.trailer_url {
                                a {
                                    class: "flex items-center gap-1.5 bg-neon-magenta/15 border border-neon-magenta/30 rounded px-2 py-1.5 mb-1.5 hover:bg-neon-magenta/25 transition-colors",
                                    href: "{trailer}",
                                    target: "_blank",
                                    onclick: move |e| e.stop_propagation(),
                                    span { class: "text-xs", "\u{25B6}\u{FE0F}" }
                                    span { class: "text-[9px] text-neon-magenta font-bold tracking-wider", "TRAILER" }
                                }
                            }

                            // Streaming providers
                            if !d.providers.is_empty() {
                                div { class: "flex flex-wrap gap-1 mb-1.5",
                                    for provider in d.providers.iter().take(3) {
                                        {render_explore_provider(provider.clone())}
                                    }
                                }
                            }

                            // Episodes/seasons info
                            div { class: "flex gap-2 mb-1.5",
                                if let Some(s) = d.total_seasons {
                                    span { class: "text-[8px] text-cyber-dim", "{s} seasons" }
                                }
                                if let Some(e) = d.total_episodes {
                                    span { class: "text-[8px] text-cyber-dim", "{e} episodes" }
                                }
                            }

                            // Similar button — drills into recommendations
                            if !d.recommendations.is_empty() {
                                button {
                                    class: "w-full text-[10px] text-neon-orange font-bold tracking-wider py-1.5 rounded border border-neon-orange/30 hover:bg-neon-orange/15 transition-colors mb-1",
                                    onclick: {
                                        let title = title_similar.clone();
                                        let recs = d.recommendations.clone();
                                        move |e: Event<MouseData>| {
                                            e.stop_propagation();
                                            breadcrumb.set(Some(title.clone()));
                                            has_more.set(false);
                                            explore_results.set(recs.clone());
                                        }
                                    },
                                    "\u{1F50D} SIMILAR"
                                }
                            }
                        }

                        // Add button (always visible)
                        if *added.read() {
                            div {
                                class: "w-full text-[10px] text-neon-green font-bold tracking-wider py-1.5 rounded border border-neon-green/30 bg-neon-green/10 text-center mt-auto",
                                "\u{2713} ADDED"
                            }
                        } else {
                            button {
                                class: "w-full text-[10px] text-neon-cyan font-bold tracking-wider py-1.5 rounded border border-neon-cyan/30 hover:bg-neon-cyan/15 transition-colors mt-auto",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    let title = title_add.clone();
                                    let mt = mt_add.clone();
                                    let ext_id = ext_id_add.clone();
                                    added.set(true);
                                    spawn(async move {
                                        match api::add_watchlist(title, mt.clone()).await {
                                            Ok(new_id) => {
                                                let _ = api::link_external_media(new_id, ext_id, mt).await;
                                                reload();
                                            }
                                            Err(e) => {
                                                added.set(false);
                                                error_msg.set(Some(format!("Failed: {e}")));
                                            }
                                        }
                                    });
                                },
                                "+ ADD TO LIST"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_explore_provider(provider: StreamingProvider) -> Element {
    let has_link = provider.link.is_some();
    let link = provider.link.clone().unwrap_or_default();
    let has_logo = !provider.logo_url.is_empty();

    if has_link {
        rsx! {
            a {
                class: "flex items-center gap-1 bg-cyber-dark/80 rounded px-1.5 py-0.5 hover:bg-cyber-dark transition-colors",
                href: "{link}",
                target: "_blank",
                onclick: move |e| e.stop_propagation(),
                if has_logo {
                    img { class: "w-4 h-4 rounded", src: "{provider.logo_url}" }
                }
                span { class: "text-[8px] text-neon-cyan", "{provider.name}" }
            }
        }
    } else {
        rsx! {
            div { class: "flex items-center gap-1 bg-cyber-dark/80 rounded px-1.5 py-0.5",
                span { class: "text-[8px] text-cyber-text", "{provider.name}" }
            }
        }
    }
}

fn render_type_chip(mt: MediaType, mut selected: Signal<MediaType>) -> Element {
    let is_active = *selected.read() == mt;
    let label = mt.label();
    let bg = if is_active {
        "bg-neon-purple/30 text-neon-purple border-neon-purple/60"
    } else {
        "bg-cyber-dark text-cyber-dim border-cyber-border"
    };

    rsx! {
        button {
            class: "flex-1 px-4 py-2.5 rounded-lg text-xs font-medium tracking-wider uppercase border {bg} transition-colors",
            r#type: "button",
            onclick: move |_| selected.set(mt.clone()),
            "{label}"
        }
    }
}

fn render_search_result(
    result: MediaSearchResult,
    on_select: impl Fn(MediaSearchResult) + Copy + 'static,
) -> Element {
    let r = result.clone();
    let year_str = result.year.as_deref().unwrap_or("");
    let has_poster = result.poster_url.is_some();
    let poster = result.poster_url.clone().unwrap_or_default();

    rsx! {
        button {
            class: "w-full flex items-center gap-3 p-2 hover:bg-cyber-card/60 transition-colors border-b border-cyber-border/30 last:border-0 text-left",
            r#type: "button",
            onclick: move |_| on_select(r.clone()),
            if has_poster {
                img {
                    class: "w-8 h-12 object-cover rounded flex-shrink-0",
                    src: "{poster}",
                }
            } else {
                div { class: "w-8 h-12 bg-cyber-border/30 rounded flex-shrink-0 flex items-center justify-center",
                    span { class: "text-[8px] text-cyber-dim", "N/A" }
                }
            }
            div { class: "flex-1 min-w-0",
                p { class: "text-sm text-cyber-text truncate", "{result.title}" }
                if !year_str.is_empty() {
                    p { class: "text-[10px] text-cyber-dim", "{year_str}" }
                }
            }
        }
    }
}

fn media_badge_color(mt: &MediaType) -> &'static str {
    match mt {
        MediaType::Movie => "bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/30",
        MediaType::Series => "bg-neon-green/10 text-neon-green border border-neon-green/30",
        MediaType::Anime => "bg-neon-pink/10 text-neon-pink border border-neon-pink/30",
    }
}

fn status_badge(status: &WatchStatus) -> Element {
    let (text, color) = match status {
        WatchStatus::Unwatched => ("NEW", "text-neon-yellow"),
        WatchStatus::InProgress => ("WATCHING", "text-neon-cyan"),
        WatchStatus::Completed => ("WATCHED", "text-neon-green"),
    };
    rsx! {
        span { class: "text-[9px] font-bold tracking-wider {color}", "{text}" }
    }
}

fn render_item(
    item: WatchItem,
    all_items: Vec<WatchItem>,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
    mut expanded_id: Signal<Option<String>>,
) -> Element {
    let id = item.id.clone();
    let id2 = item.id.clone();
    let id_expand = item.id.clone();
    let done = item.done;
    let badge = media_badge_color(&item.media_type);
    let label = item.media_type.label();
    let is_expanded = expanded_id.read().as_ref() == Some(&item.id);

    let has_poster = item.poster_url.is_some();
    let poster = item.poster_url.clone().unwrap_or_default();

    let progress_text = match (&item.media_type, item.current_season, item.current_episode, item.total_episodes) {
        (MediaType::Movie, _, _, _) => None,
        (_, Some(s), Some(e), _) => {
            // Try to get per-season episode count from season_data
            let season_eps = item.season_data.as_ref()
                .and_then(|sd| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(sd).ok())
                .and_then(|map| map.get(&s.to_string())?.as_i64().map(|v| v as i32));
            match season_eps {
                Some(sep) => Some(format!("S{s} E{e}/{sep}")),
                None => match item.total_episodes {
                    Some(total) => Some(format!("S{s} E{e} \u{2022} {total} total")),
                    None => Some(format!("S{s} E{e} (ongoing)")),
                }
            }
        }
        (_, None, None, Some(total)) => Some(format!("0 / {total}")),
        (_, None, None, None) if item.jikan_id.is_some() || item.tmdb_id.is_some() => Some("ongoing".to_string()),
        _ => None,
    };

    let episodes_watched = item.episodes_watched.unwrap_or(0);
    let total_episodes = item.total_episodes.unwrap_or(0);
    let show_progress_bar = total_episodes > 0 && !done;

    // For series: swipe right advances episode. For movies: swipe right toggles.
    let is_episodic = matches!(item.media_type, MediaType::Series | MediaType::Anime);
    let (next_season, next_episode) = compute_next_episode(
        item.current_season,
        item.current_episode,
        &item.season_data,
    );

    rsx! {
        div {
            SwipeItem {
                completed: done,
                on_swipe_right: move |_| {
                    let id = id.clone();
                    if is_episodic {
                        spawn(async move {
                            match api::update_watch_progress(id, next_season, next_episode).await {
                                Ok(()) => reload(),
                                Err(e) => error_msg.set(Some(format!("Failed to update: {e}"))),
                            }
                        });
                    } else {
                        spawn(async move {
                            match api::toggle_watchlist(id).await {
                                Ok(()) => reload(),
                                Err(e) => error_msg.set(Some(format!("Failed to toggle: {e}"))),
                            }
                        });
                    }
                },
                on_swipe_left: move |_| {
                    let id = id2.clone();
                    spawn(async move {
                        match api::delete_watchlist(id).await {
                            Ok(()) => reload(),
                            Err(e) => error_msg.set(Some(format!("Failed to delete: {e}"))),
                        }
                    });
                },
                div {
                    class: "cursor-pointer",
                    onclick: move |_| {
                        let current = expanded_id.read().clone();
                        if current.as_ref() == Some(&id_expand) {
                            expanded_id.set(None);
                        } else {
                            expanded_id.set(Some(id_expand.clone()));
                        }
                    },
                    div { class: "flex items-center gap-3 min-h-[56px]",
                        // Poster thumbnail
                        if has_poster {
                            img {
                                class: "w-10 h-14 object-cover rounded flex-shrink-0",
                                src: "{poster}",
                            }
                        } else {
                            div { class: "w-10 h-14 bg-cyber-border/20 rounded flex-shrink-0 flex items-center justify-center border border-cyber-border/30",
                                span { class: "text-[8px] text-cyber-dim/50 font-mono", "?" }
                            }
                        }
                        div { class: "flex-1 min-w-0",
                            p { class: "text-sm font-medium truncate", "{item.text}" }
                            div { class: "flex items-center gap-2 mt-0.5",
                                if let Some(ref pt) = progress_text {
                                    span { class: "text-[10px] text-neon-cyan font-mono", "{pt}" }
                                }
                            }
                            if show_progress_bar {
                                div { class: "mt-1",
                                    ProgressBar { watched: episodes_watched, total: total_episodes }
                                }
                            }
                        }
                        div { class: "flex flex-col items-end gap-1 flex-shrink-0",
                            span { class: "text-[10px] px-2 py-0.5 rounded font-medium tracking-wider uppercase {badge}", "{label}" }
                            {status_badge(&item.status)}
                            if let Some(by) = &item.completed_by {
                                if done {
                                    p { class: "text-[10px] text-cyber-dim", "by {by}" }
                                }
                            }
                            // Expand indicator
                            {
                                let rotate = if is_expanded { "rotate-180" } else { "" };
                                rsx! { svg {
                                    class: "w-3 h-3 text-cyber-dim/40 transition-transform {rotate}",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    fill: "none",
                                    view_box: "0 0 24 24",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    path {
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        d: "M19 9l-7 7-7-7",
                                    }
                                }}
                            }
                        }
                    }
                }
            }

            // Expanded detail panel
            if is_expanded {
                DetailPanel {
                    key: "{item.id}",
                    item: item.clone(),
                    all_items: all_items.clone(),
                    reload: move |_| reload(),
                    error_msg: error_msg,
                }
            }
        }
    }
}

#[component]
fn DetailPanel(
    item: WatchItem,
    all_items: Vec<WatchItem>,
    reload: EventHandler<()>,
    error_msg: Signal<Option<String>>,
) -> Element {
    let mut error_msg = error_msg;
    let reload = move || { reload.call(()); };

    let item_id = item.id.clone();
    let item_id2 = item.id.clone();
    let item_id3 = item.id.clone();
    let is_episodic = matches!(item.media_type, MediaType::Series | MediaType::Anime);

    let (next_season, next_episode) = compute_next_episode(
        item.current_season,
        item.current_episode,
        &item.season_data,
    );

    // Parse season_data to get total seasons list
    let season_map: Option<serde_json::Map<String, serde_json::Value>> = item
        .season_data
        .as_ref()
        .and_then(|sd| serde_json::from_str(sd).ok());

    let num_seasons = season_map
        .as_ref()
        .map(|m| m.keys().filter_map(|k| k.parse::<i32>().ok()).max().unwrap_or(0))
        .or(item.total_seasons)
        .unwrap_or(1);

    // Per-season progress (loaded async)
    let mut season_progress = use_signal(std::collections::HashMap::<i32, i32>::new);
    let mut season_progress_loaded = use_signal(|| false);

    use_effect({
        let item_id = item_id.clone();
        move || {
            let item_id = item_id.clone();
            spawn(async move {
                if let Ok(progress) = api::get_season_progress(item_id).await {
                    season_progress.set(progress);
                }
                season_progress_loaded.set(true);
            });
        }
    });

    // Streaming providers
    let mut providers = use_signal(Vec::<StreamingProvider>::new);
    let mut providers_loaded = use_signal(|| false);

    // Recommendations
    let mut recs = use_signal(Vec::<MediaRecommendation>::new);
    let mut recs_loaded = use_signal(|| false);

    // Franchise links
    let mut franchise_links = use_signal(Vec::<FranchiseLink>::new);
    let mut franchise_loaded = use_signal(|| false);

    // Franchise linking UI
    let mut show_link_ui = use_signal(|| false);
    let mut link_search_text = use_signal(String::new);
    let mut link_search_results = use_signal(Vec::<WatchItem>::new);
    let link_relation = use_signal(|| FranchiseRelation::Sequel);

    let has_external = item.tmdb_id.is_some() || item.jikan_id.is_some();
    let mut show_synopsis = use_signal(|| false);

    // Load data on mount — key ensures fresh hooks per item
    use_effect({
        let item_id = item_id.clone();
        move || {
            let item_id = item_id.clone();
            spawn(async move {
                if let Ok(p) = api::get_streaming_providers(item_id.clone()).await {
                    providers.set(p);
                }
                providers_loaded.set(true);

                if let Ok(r) = api::get_recommendations(item_id.clone()).await {
                    recs.set(r);
                }
                recs_loaded.set(true);

                if let Ok(f) = api::get_franchise_links(item_id).await {
                    franchise_links.set(f);
                }
                franchise_loaded.set(true);
            });
        }
    });

    rsx! {
        div { class: "bg-cyber-dark/80 border-l-2 border-l-neon-cyan/40 border-t border-neon-cyan/20 border-b border-r border-cyber-border/50 rounded-b-lg -mt-2 mb-2 p-3 space-y-3",

            // Quick actions for episodic content
            if is_episodic {
                div { class: "space-y-2",
                    // +1 episode button
                    button {
                        class: "w-full bg-neon-cyan/15 text-neon-cyan border border-neon-cyan/30 rounded-lg py-2 text-xs font-bold tracking-wider hover:bg-neon-cyan/25 transition-colors",
                        onclick: {
                            let item_id = item_id2.clone();
                            move |_| {
                                let item_id = item_id.clone();
                                spawn(async move {
                                    match api::update_watch_progress(item_id, next_season, next_episode).await {
                                        Ok(()) => reload(),
                                        Err(e) => error_msg.set(Some(format!("Failed: {e}"))),
                                    }
                                });
                            }
                        },
                        "+1 EP (S{next_season}E{next_episode})"
                    }

                    // Season chips — mark entire seasons as done
                    if num_seasons > 0 {
                        div {
                            p { class: "text-[10px] text-cyber-dim tracking-wider uppercase mb-1.5", "MARK SEASON DONE" }
                            div { class: "flex flex-wrap gap-1.5",
                                for s in 1..=num_seasons {
                                    {
                                        let item_id_s = item_id2.clone();
                                        let eps_total = season_map.as_ref()
                                            .and_then(|m| m.get(&s.to_string())?.as_i64().map(|v| v as i32))
                                            .unwrap_or(0);
                                        let eps_watched = *season_progress.read().get(&s).unwrap_or(&0);
                                        let is_complete = eps_total > 0 && eps_watched >= eps_total;
                                        let is_partial = eps_watched > 0 && !is_complete;

                                        let chip_class = if is_complete {
                                            "bg-neon-green/25 text-neon-green border-neon-green/50"
                                        } else if is_partial {
                                            "bg-neon-orange/15 text-neon-orange border-neon-orange/30"
                                        } else {
                                            "bg-cyber-card/60 text-cyber-dim border-cyber-border/40 hover:bg-neon-green/15 hover:text-neon-green hover:border-neon-green/30"
                                        };

                                        let label = if is_complete {
                                            format!("\u{2713} S{s}")
                                        } else if is_partial {
                                            format!("S{s} {eps_watched}/{eps_total}")
                                        } else if eps_total > 0 {
                                            format!("S{s} ({}ep)", eps_total)
                                        } else {
                                            format!("S{s}")
                                        };

                                        rsx! {
                                            button {
                                                class: "border rounded-md px-2.5 py-1 text-[11px] font-bold tracking-wider transition-colors {chip_class}",
                                                disabled: is_complete,
                                                onclick: move |_| {
                                                    let item_id = item_id_s.clone();
                                                    spawn(async move {
                                                        match api::complete_season(item_id, s).await {
                                                            Ok(()) => reload(),
                                                            Err(e) => error_msg.set(Some(format!("Failed: {e}"))),
                                                        }
                                                    });
                                                },
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Synopsis (collapsible)
            if let Some(ref overview) = item.overview {
                div {
                    button {
                        class: "flex items-center gap-1.5 text-[10px] text-cyber-dim tracking-wider uppercase hover:text-cyber-text transition-colors py-1",
                        onclick: move |_| {
                            let current = *show_synopsis.read();
                            show_synopsis.set(!current);
                        },
                        if *show_synopsis.read() { "\u{25BC} SYNOPSIS" } else { "\u{25B6} SYNOPSIS" }
                    }
                    if *show_synopsis.read() {
                        p { class: "text-xs text-cyber-text/80 leading-relaxed mt-1.5", "{overview}" }
                    }
                }
            }

            // Trailer
            if let Some(ref trailer) = item.trailer_url {
                a {
                    class: "flex items-center gap-2 bg-neon-magenta/10 border border-neon-magenta/30 rounded-lg px-3 py-2 hover:bg-neon-magenta/20 transition-colors",
                    href: "{trailer}",
                    target: "_blank",
                    span { class: "text-sm", "\u{25B6}\u{FE0F}" }
                    span { class: "text-[10px] text-neon-magenta font-bold tracking-wider uppercase", "WATCH TRAILER" }
                }
            }

            // Streaming providers
            if *providers_loaded.read() && !providers.read().is_empty() {
                div {
                    p { class: "text-[10px] text-cyber-dim tracking-wider uppercase mb-1.5", "WHERE TO WATCH" }
                    div { class: "flex flex-wrap gap-2",
                        for provider in providers.read().iter() {
                            {render_provider(provider.clone())}
                        }
                    }
                }
            }
            if *providers_loaded.read() && providers.read().is_empty() && has_external {
                div {
                    p { class: "text-[10px] text-cyber-dim/50 tracking-wider", "NO STREAMING PROVIDERS FOUND FOR YOUR REGION" }
                }
            }
            if !has_external {
                div {
                    p { class: "text-[10px] text-cyber-dim/40 tracking-wider italic", "SEARCH \u{2192} ADD TO LINK TMDB/JIKAN FOR STREAMING & RECS" }
                }
            }

            // Franchise links
            if *franchise_loaded.read() {
                div {
                    div { class: "flex items-center justify-between mb-1.5",
                        p { class: "text-[10px] text-cyber-dim tracking-wider uppercase", "FRANCHISE" }
                        button {
                            class: "text-[10px] text-neon-purple hover:text-neon-purple/80 tracking-wider px-2 py-2",
                            onclick: move |_| {
                            let current = *show_link_ui.read();
                            show_link_ui.set(!current);
                        },
                            if *show_link_ui.read() { "CANCEL" } else { "+ LINK" }
                        }
                    }
                    if !franchise_links.read().is_empty() {
                        div { class: "space-y-1",
                            for link in franchise_links.read().iter() {
                                {render_franchise_link(link.clone(), reload, error_msg)}
                            }
                        }
                    }

                    // Link UI
                    if *show_link_ui.read() {
                        div { class: "mt-2 space-y-2 bg-cyber-card/40 rounded-lg p-2",
                            // Relation type selector
                            div { class: "flex gap-1",
                                for rel in [FranchiseRelation::Sequel, FranchiseRelation::Prequel, FranchiseRelation::Spinoff] {
                                    {render_relation_chip(rel.clone(), link_relation)}
                                }
                            }
                            // Search existing items
                            input {
                                class: "w-full bg-cyber-dark border border-cyber-border rounded px-3 py-1.5 text-xs text-cyber-text outline-none focus:border-neon-purple/60 font-mono",
                                placeholder: "Search your watchlist...",
                                value: "{link_search_text}",
                                oninput: move |e| {
                                    let val = e.value();
                                    link_search_text.set(val.clone());
                                    let id = item_id3.clone();
                                    let filtered: Vec<WatchItem> = all_items
                                        .iter()
                                        .filter(|i| i.id != id && i.text.to_lowercase().contains(&val.to_lowercase()))
                                        .take(5)
                                        .cloned()
                                        .collect();
                                    link_search_results.set(filtered);
                                },
                            }
                            for result in link_search_results.read().iter() {
                                {render_link_candidate(item_id.clone(), result.clone(), link_relation.read().clone(), reload, error_msg, show_link_ui, franchise_links)}
                            }
                        }
                    }
                }
            }

            // Recommendations
            if *recs_loaded.read() && !recs.read().is_empty() {
                div {
                    p { class: "text-[10px] text-cyber-dim tracking-wider uppercase mb-1.5", "RECOMMENDED" }
                    div { class: "space-y-1",
                        for rec in recs.read().iter().take(5) {
                            {render_recommendation(rec.clone(), item.media_type.clone(), reload, error_msg)}
                        }
                    }
                }
            }
        }
    }
}

fn render_provider(provider: StreamingProvider) -> Element {
    let has_logo = !provider.logo_url.is_empty();
    let has_link = provider.link.is_some();
    let link = provider.link.clone().unwrap_or_default();
    let type_label = match provider.provider_type.as_str() {
        "rent" => " (Rent)",
        "buy" => " (Buy)",
        _ => "",
    };

    if has_link {
        rsx! {
            a {
                class: "flex items-center gap-1.5 bg-cyber-card/60 rounded-lg px-2 py-1.5 hover:bg-cyber-card/80 transition-colors",
                href: "{link}",
                target: "_blank",
                if has_logo {
                    img {
                        class: "w-5 h-5 rounded",
                        src: "{provider.logo_url}",
                    }
                }
                span { class: "text-[10px] text-neon-cyan", "{provider.name}{type_label}" }
            }
        }
    } else {
        rsx! {
            div { class: "flex items-center gap-1.5 bg-cyber-card/60 rounded-lg px-2 py-1",
                if has_logo {
                    img {
                        class: "w-5 h-5 rounded",
                        src: "{provider.logo_url}",
                    }
                }
                span { class: "text-[10px] text-cyber-text", "{provider.name}{type_label}" }
            }
        }
    }
}

fn render_franchise_link(
    link: FranchiseLink,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let link_id = link.id.clone();
    let status_color = match link.to_item_status {
        WatchStatus::Completed => "text-neon-green",
        WatchStatus::InProgress => "text-neon-cyan",
        WatchStatus::Unwatched => "text-cyber-dim",
    };

    rsx! {
        div { class: "flex items-center gap-2 bg-cyber-card/40 rounded px-2 py-1.5",
            span { class: "text-[9px] text-neon-purple font-bold tracking-wider uppercase", "{link.relation.label()}" }
            span { class: "text-xs text-cyber-text flex-1 truncate", "{link.to_item_title}" }
            span { class: "text-[9px] font-bold tracking-wider {status_color}",
                {match link.to_item_status {
                    WatchStatus::Completed => "DONE",
                    WatchStatus::InProgress => "WATCHING",
                    WatchStatus::Unwatched => "NEW",
                }}
            }
            button {
                class: "text-xs text-neon-magenta/60 hover:text-neon-magenta ml-1 p-2 -mr-2",
                onclick: move |_| {
                    let link_id = link_id.clone();
                    spawn(async move {
                        match api::unlink_franchise(link_id).await {
                            Ok(()) => reload(),
                            Err(e) => error_msg.set(Some(format!("Failed: {e}"))),
                        }
                    });
                },
                "\u{2715}"
            }
        }
    }
}

fn render_relation_chip(rel: FranchiseRelation, mut selected: Signal<FranchiseRelation>) -> Element {
    let is_active = *selected.read() == rel;
    let label = rel.label();
    let bg = if is_active {
        "bg-neon-purple/30 text-neon-purple border-neon-purple/60"
    } else {
        "bg-cyber-dark text-cyber-dim border-cyber-border"
    };

    rsx! {
        button {
            class: "flex-1 px-2 py-1 rounded text-[10px] font-medium tracking-wider uppercase border {bg} transition-colors",
            r#type: "button",
            onclick: move |_| selected.set(rel.clone()),
            "{label}"
        }
    }
}

fn render_link_candidate(
    from_id: String,
    candidate: WatchItem,
    relation: FranchiseRelation,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
    mut show_link_ui: Signal<bool>,
    mut franchise_links: Signal<Vec<FranchiseLink>>,
) -> Element {
    let to_id = candidate.id.clone();

    rsx! {
        button {
            class: "w-full flex items-center gap-2 p-1.5 hover:bg-cyber-card/60 rounded transition-colors text-left",
            r#type: "button",
            onclick: move |_| {
                let from = from_id.clone();
                let to = to_id.clone();
                let rel = relation.clone();
                spawn(async move {
                    match api::link_franchise(from.clone(), to, rel).await {
                        Ok(()) => {
                            show_link_ui.set(false);
                            // Reload franchise links
                            if let Ok(f) = api::get_franchise_links(from).await {
                                franchise_links.set(f);
                            }
                            reload();
                        }
                        Err(e) => error_msg.set(Some(format!("Failed: {e}"))),
                    }
                });
            },
            div { class: "flex-1 min-w-0",
                p { class: "text-xs text-cyber-text truncate", "{candidate.text}" }
            }
            span { class: "text-[9px] px-1.5 py-0.5 rounded font-medium tracking-wider uppercase {media_badge_color(&candidate.media_type)}", "{candidate.media_type.label()}" }
        }
    }
}

fn render_recommendation(
    rec: MediaRecommendation,
    parent_media_type: MediaType,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let has_poster = rec.poster_url.is_some();
    let poster = rec.poster_url.clone().unwrap_or_default();
    let year_str = rec.year.as_deref().unwrap_or("");
    let already = rec.already_in_list;
    let title = rec.title.clone();
    let external_id = rec.external_id.clone();

    rsx! {
        div { class: "flex items-center gap-2 bg-cyber-card/40 rounded px-2 py-1.5",
            if has_poster {
                img {
                    class: "w-6 h-9 object-cover rounded flex-shrink-0",
                    src: "{poster}",
                }
            }
            div { class: "flex-1 min-w-0",
                p { class: "text-xs text-cyber-text truncate", "{rec.title}" }
                if !year_str.is_empty() {
                    p { class: "text-[9px] text-cyber-dim", "{year_str}" }
                }
            }
            if already {
                span { class: "text-[9px] text-neon-green font-bold tracking-wider", "IN LIST" }
            } else {
                button {
                    class: "text-[10px] text-neon-cyan font-bold tracking-wider px-2 py-2 rounded border border-neon-cyan/20 hover:bg-neon-cyan/10 transition-colors",
                    onclick: move |_| {
                        let title = title.clone();
                        let mt = parent_media_type.clone();
                        let ext_id = external_id.clone();
                        spawn(async move {
                            match api::add_watchlist(title, mt.clone()).await {
                                Ok(new_id) => {
                                    let _ = api::link_external_media(new_id, ext_id, mt).await;
                                    reload();
                                }
                                Err(e) => error_msg.set(Some(format!("Failed: {e}"))),
                            }
                        });
                    },
                    "+ ADD"
                }
            }
        }
    }
}

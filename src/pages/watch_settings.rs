use dioxus::prelude::*;

use crate::api::watchlist as api;
use crate::components::error_banner::ErrorBanner;
use crate::models::WatchSettings;

/// Maps provider display names to TMDB provider IDs (TW region).
const STREAMING_PROVIDERS: &[(&str, i32)] = &[
    ("Netflix", 8),
    ("Disney+", 337),
    ("HBO Max", 384),
    ("Apple TV+", 350),
    ("Amazon Prime", 119),
    ("Catchplay", 159),
    ("YouTube Premium", 188),
    ("Crunchyroll", 283),
    ("LINE TV", 420),
];

pub fn provider_ids_param(settings: &WatchSettings) -> Option<String> {
    if !settings.filter_by_provider || settings.streaming_providers.is_empty() {
        return None;
    }
    let ids: Vec<String> = settings
        .streaming_providers
        .iter()
        .filter_map(|name| {
            STREAMING_PROVIDERS
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, id)| id.to_string())
        })
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids.join("|"))
    }
}

#[component]
pub fn WatchSettingsPage() -> Element {
    let mut settings = use_signal(WatchSettings::default);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| true);
    let nav = use_navigator();

    use_effect(move || {
        spawn(async move {
            match api::get_watch_settings().await {
                Ok(s) => settings.set(s),
                Err(e) => error_msg.set(Some(format!("Failed to load settings: {e}"))),
            }
            loading.set(false);
        });
    });

    let auto_save = move || {
        let s = settings.read().clone();
        spawn(async move {
            if let Err(e) = api::save_watch_settings(s).await {
                error_msg.set(Some(format!("Failed to save: {e}")));
            }
        });
    };

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            // Header row with back button
            div { class: "flex items-center gap-3",
                button {
                    class: "p-2 text-neon-cyan hover:text-neon-cyan/80 transition-colors",
                    r#type: "button",
                    onclick: move |_| { nav.go_back(); },
                    svg {
                        class: "w-5 h-5",
                        xmlns: "http://www.w3.org/2000/svg",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke: "currentColor",
                        stroke_width: "2",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M15 19l-7-7 7-7",
                        }
                    }
                }
                h1 { class: "text-sm font-bold tracking-[0.3em] uppercase text-neon-cyan",
                    "WATCH SETTINGS"
                }
            }

            if *loading.read() {
                div { class: "text-center py-12",
                    span { class: "text-[10px] text-neon-cyan tracking-wider animate-pulse", "LOADING..." }
                }
            } else {
                // Filter toggle card
                div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4 space-y-1",
                    div { class: "flex items-center justify-between",
                        div {
                            p { class: "text-sm font-bold tracking-wider text-cyber-text", "Filter by streaming service" }
                            p { class: "text-[10px] text-cyber-dim mt-0.5 tracking-wider",
                                "Only show content available on selected services"
                            }
                        }
                        // Toggle switch
                        button {
                            class: "relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none flex-shrink-0 ml-3",
                            class: if settings.read().filter_by_provider { "bg-neon-cyan/70" } else { "bg-cyber-border" },
                            r#type: "button",
                            onclick: move |_| {
                                let current = settings.read().filter_by_provider;
                                settings.write().filter_by_provider = !current;
                                auto_save();
                            },
                            span {
                                class: "inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform",
                                class: if settings.read().filter_by_provider { "translate-x-6" } else { "translate-x-1" },
                            }
                        }
                    }
                }

                // Streaming services card
                div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4 space-y-3",
                    p { class: "text-[11px] font-bold tracking-[0.2em] uppercase text-cyber-dim",
                        "YOUR STREAMING SERVICES"
                    }
                    div { class: "flex flex-wrap gap-2",
                        for (name, _id) in STREAMING_PROVIDERS.iter() {
                            {
                                let name_str = name.to_string();
                                let name_check = name.to_string();
                                let is_active = settings.read().streaming_providers.contains(&name_str);
                                let chip_class = if is_active {
                                    "bg-neon-cyan/30 text-neon-cyan border-neon-cyan/60"
                                } else {
                                    "bg-cyber-dark text-cyber-dim border-cyber-border hover:text-cyber-text hover:border-cyber-border/80"
                                };
                                rsx! {
                                    button {
                                        class: "px-3 py-1.5 rounded-full text-xs font-bold tracking-wider uppercase border transition-colors {chip_class}",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let mut s = settings.write();
                                            if s.streaming_providers.contains(&name_check) {
                                                s.streaming_providers.retain(|p| p != &name_check);
                                            } else {
                                                s.streaming_providers.push(name_check.clone());
                                            }
                                            drop(s);
                                            auto_save();
                                        },
                                        "{name}"
                                    }
                                }
                            }
                        }
                    }

                    if settings.read().streaming_providers.is_empty() {
                        p { class: "text-[10px] text-cyber-dim/60 tracking-wider mt-1",
                            "Select services to enable filtering"
                        }
                    } else {
                        p { class: "text-[10px] text-neon-cyan/60 tracking-wider mt-1",
                            {format!("{} service{} selected", settings.read().streaming_providers.len(),
                                if settings.read().streaming_providers.len() == 1 { "" } else { "s" })}
                        }
                    }
                }

                // Info note
                if !settings.read().filter_by_provider {
                    div { class: "bg-neon-orange/5 border border-neon-orange/20 rounded-xl p-3",
                        p { class: "text-[10px] text-neon-orange/80 tracking-wider leading-relaxed",
                            "Filtering is off \u{2014} Explore shows all content regardless of availability. Enable the toggle above to filter Trending and Genre results to your services."
                        }
                    }
                }
            }
        }
    }
}

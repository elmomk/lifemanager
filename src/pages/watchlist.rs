use dioxus::prelude::*;

use crate::cache::{self, SyncStatus};
use crate::components::layout::SyncTrigger;
use crate::api::watchlist as watchlist_api;
use crate::components::error_banner::ErrorBanner;
use crate::components::swipe_item::SwipeItem;
use crate::models::{MediaType, WatchItem};

#[component]
pub fn Watchlist() -> Element {
    let mut items = use_signal(Vec::<WatchItem>::new);
    let mut input_text = use_signal(String::new);
    let selected_type = use_signal(|| MediaType::Movie);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut sync_status: Signal<SyncStatus> = use_context();
    let sync_trigger: Signal<SyncTrigger> = use_context();

    let reload = move || {
        spawn(async move {
            sync_status.set(SyncStatus::Syncing);
            match watchlist_api::list_watchlist().await {
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
        });
    };

    use_effect(move || {
        if let Some(cached) = cache::read::<Vec<WatchItem>>("watchlist") {
            items.set(cached);
        }
        reload();
    });

    use_effect(move || {
        let _trigger = sync_trigger.read().0;
        reload();
    });

    let add_item = move |text: String| {
        if text.trim().is_empty() {
            return;
        }
        let media_type = selected_type.read().clone();
        spawn(async move {
            match watchlist_api::add_watchlist(text, media_type).await {
                Ok(()) => {
                    input_text.set(String::new());
                    reload();
                }
                Err(e) => error_msg.set(Some(format!("Failed to add: {e}"))),
            }
        });
    };

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4",
                form {
                    class: "space-y-3",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let text = input_text.read().clone();
                        add_item(text);
                    },
                    input {
                        class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text outline-none focus:border-neon-purple/60 font-mono",
                        r#type: "text",
                        placeholder: "Add to watchlist...",
                        value: "{input_text}",
                        oninput: move |e| input_text.set(e.value()),
                    }
                    // Media type selector
                    div { class: "flex gap-2",
                        for mt in MediaType::all() {
                            { render_type_chip(mt.clone(), selected_type) }
                        }
                    }
                    button {
                        class: "w-full bg-neon-purple/20 text-neon-purple border border-neon-purple/40 rounded-lg px-4 py-2 text-xs font-bold tracking-wider uppercase hover:bg-neon-purple/30 transition-colors glow-purple",
                        r#type: "submit",
                        "ADD"
                    }
                }
            }

            div { class: "space-y-0",
                for item in items.read().iter() {
                    { render_item(item.clone(), reload, error_msg) }
                }
                if items.read().is_empty() {
                    div { class: "text-center py-16",
                        p { class: "text-2xl mb-3 opacity-30", "\u{1F3AC}" }
                        p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim", "Nothing to watch yet" }
                        p { class: "text-[10px] text-cyber-dim/40 mt-2 tracking-wider",
                            "SWIPE \u{2192} WATCHED \u{2022} SWIPE \u{2190} DELETE"
                        }
                    }
                }
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

fn media_badge_color(mt: &MediaType) -> &'static str {
    match mt {
        MediaType::Movie => "bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/30",
        MediaType::Series => "bg-neon-green/10 text-neon-green border border-neon-green/30",
        MediaType::Anime => "bg-neon-pink/10 text-neon-pink border border-neon-pink/30",
        MediaType::Cartoon => "bg-neon-yellow/10 text-neon-yellow border border-neon-yellow/30",
    }
}

fn render_item(
    item: WatchItem,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let id = item.id.clone();
    let id2 = item.id.clone();
    let done = item.done;
    let badge = media_badge_color(&item.media_type);
    let label = item.media_type.label();

    rsx! {
        SwipeItem {
            completed: done,
            on_swipe_right: move |_| {
                let id = id.clone();
                spawn(async move {
                    match watchlist_api::toggle_watchlist(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to toggle: {e}"))),
                    }
                });
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    match watchlist_api::delete_watchlist(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to delete: {e}"))),
                    }
                });
            },
            div { class: "flex items-center gap-3",
                div { class: "flex-1",
                    p { class: "text-sm font-medium", "{item.text}" }
                }
                span { class: "text-[10px] px-2 py-0.5 rounded font-medium tracking-wider uppercase {badge}", "{label}" }
                if done {
                    div { class: "text-right",
                        span { class: "text-xs text-neon-green font-bold tracking-wider", "WATCHED" }
                        if let Some(by) = &item.completed_by {
                            p { class: "text-[10px] text-cyber-dim", "{by}" }
                        }
                    }
                }
            }
        }
    }
}

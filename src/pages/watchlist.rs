use dioxus::prelude::*;

use crate::api::watchlist as watchlist_api;
use crate::components::swipe_item::SwipeItem;
use crate::models::{MediaType, WatchItem};

#[component]
pub fn Watchlist() -> Element {
    let mut items = use_signal(Vec::<WatchItem>::new);
    let mut input_text = use_signal(String::new);
    let selected_type = use_signal(|| MediaType::Movie);
    let mut refresh = use_signal(|| 0u32);

    use_effect(move || {
        let _ = refresh();
        spawn(async move {
            match watchlist_api::list_watchlist().await {
                Ok(loaded) => items.set(loaded),
                Err(e) => tracing::error!("Failed to load watchlist: {e}"),
            }
        });
    });

    let add_item = move |text: String| {
        if text.trim().is_empty() {
            return;
        }
        let media_type = selected_type.read().clone();
        spawn(async move {
            if watchlist_api::add_watchlist(text, media_type).await.is_ok() {
                input_text.set(String::new());
                refresh.set(refresh() + 1);
            }
        });
    };

    rsx! {
        div { class: "p-4 space-y-4",
            div { class: "bg-white/70 dark:bg-gray-800/70 backdrop-blur-lg rounded-2xl p-4 shadow-sm",
                form {
                    class: "space-y-3",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let text = input_text.read().clone();
                        add_item(text);
                    },
                    input {
                        class: "w-full bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500",
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
                        class: "w-full bg-purple-500 text-white rounded-xl px-4 py-2 text-sm font-medium hover:bg-purple-600 transition-colors",
                        r#type: "submit",
                        "Add"
                    }
                }
            }

            div { class: "space-y-0",
                for item in items.read().iter() {
                    { render_item(item.clone(), refresh) }
                }
                if items.read().is_empty() {
                    div { class: "text-center text-gray-400 dark:text-gray-600 py-8",
                        p { "Nothing to watch yet" }
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
        "bg-purple-500 text-white"
    } else {
        "bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
    };

    rsx! {
        button {
            class: "flex-1 px-3 py-1.5 rounded-xl text-xs font-medium {bg} transition-colors",
            r#type: "button",
            onclick: move |_| selected.set(mt.clone()),
            "{label}"
        }
    }
}

fn media_badge_color(mt: &MediaType) -> &'static str {
    match mt {
        MediaType::Movie => "bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300",
        MediaType::Series => "bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300",
        MediaType::Anime => "bg-pink-100 dark:bg-pink-900/50 text-pink-700 dark:text-pink-300",
        MediaType::Cartoon => "bg-yellow-100 dark:bg-yellow-900/50 text-yellow-700 dark:text-yellow-300",
    }
}

fn render_item(item: WatchItem, mut refresh: Signal<u32>) -> Element {
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
                    if watchlist_api::toggle_watchlist(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    if watchlist_api::delete_watchlist(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            div { class: "flex items-center gap-3",
                div { class: "flex-1",
                    p { class: "text-sm font-medium", "{item.text}" }
                }
                span { class: "text-xs px-2 py-0.5 rounded-lg font-medium {badge}", "{label}" }
                if done {
                    span { class: "text-xs text-green-500 font-medium", "Watched" }
                }
            }
        }
    }
}

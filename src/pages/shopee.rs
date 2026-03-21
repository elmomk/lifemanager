use dioxus::prelude::*;

use crate::api::shopee as shopee_api;
use crate::components::shopee_ocr::ShopeeOcr;
use crate::components::swipe_item::SwipeItem;
use crate::models::ShopeePackage;

const STORE_CHIPS: &[&str] = &["7-11", "FamilyMart", "Hi-Life", "OK Mart"];

#[component]
pub fn Shopee() -> Element {
    let mut items = use_signal(Vec::<ShopeePackage>::new);
    let mut input_title = use_signal(String::new);
    let mut input_store = use_signal(String::new);
    let mut input_code = use_signal(String::new);
    let mut refresh = use_signal(|| 0u32);

    use_effect(move || {
        let _ = refresh();
        spawn(async move {
            match shopee_api::list_shopee().await {
                Ok(loaded) => items.set(loaded),
                Err(e) => tracing::error!("Failed to load packages: {e}"),
            }
        });
    });

    rsx! {
        div { class: "p-4 space-y-4",
            div { class: "bg-white/70 dark:bg-gray-800/70 backdrop-blur-lg rounded-2xl p-4 shadow-sm",
                form {
                    class: "space-y-2",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let title = input_title.read().clone();
                        if title.trim().is_empty() {
                            return;
                        }
                        let store = {
                            let s = input_store.read().clone();
                            if s.is_empty() { None } else { Some(s) }
                        };
                        let code = {
                            let c = input_code.read().clone();
                            if c.is_empty() { None } else { Some(c) }
                        };
                        spawn(async move {
                            if shopee_api::add_shopee(title, store, code).await.is_ok() {
                                input_title.set(String::new());
                                input_store.set(String::new());
                                input_code.set(String::new());
                                refresh.set(refresh() + 1);
                            }
                        });
                    },
                    input {
                        class: "w-full bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500",
                        r#type: "text",
                        placeholder: "Package description...",
                        value: "{input_title}",
                        oninput: move |e| input_title.set(e.value()),
                    }
                    div { class: "flex gap-2",
                        input {
                            class: "flex-1 bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500",
                            r#type: "text",
                            placeholder: "Store...",
                            value: "{input_store}",
                            oninput: move |e| input_store.set(e.value()),
                        }
                        input {
                            class: "w-24 bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500",
                            r#type: "text",
                            placeholder: "Code",
                            value: "{input_code}",
                            oninput: move |e| input_code.set(e.value()),
                        }
                        ShopeeOcr {
                            on_code_extracted: move |code: String| {
                                input_code.set(code);
                            },
                        }
                    }
                    // Store quick-select
                    div { class: "flex gap-2 overflow-x-auto pb-1",
                        for store in STORE_CHIPS {
                            { render_store_chip(store, input_store) }
                        }
                    }
                    button {
                        class: "w-full bg-orange-500 text-white rounded-xl px-4 py-2 text-sm font-medium hover:bg-orange-600 transition-colors",
                        r#type: "submit",
                        "Add Package"
                    }
                }
            }

            div { class: "space-y-0",
                for pkg in items.read().iter() {
                    { render_package(pkg.clone(), refresh) }
                }
                if items.read().is_empty() {
                    div { class: "text-center text-gray-400 dark:text-gray-600 py-8",
                        p { "No packages to pick up" }
                    }
                }
            }
        }
    }
}

fn render_store_chip(store: &&str, mut input_store: Signal<String>) -> Element {
    let store = store.to_string();
    let store_clone = store.clone();
    rsx! {
        button {
            class: "shrink-0 px-3 py-1.5 rounded-full text-xs font-medium bg-orange-100 dark:bg-orange-900/50 text-orange-700 dark:text-orange-300 hover:bg-orange-200 dark:hover:bg-orange-800 transition-colors",
            r#type: "button",
            onclick: move |_| input_store.set(store_clone.clone()),
            "{store}"
        }
    }
}

fn render_package(pkg: ShopeePackage, mut refresh: Signal<u32>) -> Element {
    let id = pkg.id.clone();
    let id2 = pkg.id.clone();
    let picked_up = pkg.picked_up;

    rsx! {
        SwipeItem {
            completed: picked_up,
            on_swipe_right: move |_| {
                let id = id.clone();
                spawn(async move {
                    if shopee_api::toggle_shopee(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    if shopee_api::delete_shopee(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            div { class: "space-y-1",
                div { class: "flex items-center gap-2",
                    p { class: "text-sm font-medium flex-1", "{pkg.title}" }
                    if picked_up {
                        span { class: "text-xs text-green-500 font-medium", "Picked up" }
                    }
                }
                div { class: "flex gap-2 text-xs text-gray-400",
                    if let Some(store) = &pkg.store {
                        span { class: "bg-gray-100 dark:bg-gray-700 px-2 py-0.5 rounded-lg", "{store}" }
                    }
                    if let Some(code) = &pkg.code {
                        span { class: "bg-yellow-100 dark:bg-yellow-900/50 text-yellow-700 dark:text-yellow-300 px-2 py-0.5 rounded-lg font-mono", "{code}" }
                    }
                }
            }
        }
    }
}

use dioxus::prelude::*;

use crate::api::shopee as shopee_api;
use crate::components::error_banner::ErrorBanner;
use crate::components::shopee_ocr::ShopeeOcr;
use crate::components::swipe_item::SwipeItem;
use crate::models::{OcrResult, ShopeePackage};

const STORE_CHIPS: &[&str] = &["7-11", "FamilyMart", "Hi-Life", "OK Mart"];

#[component]
pub fn Shopee() -> Element {
    let mut items = use_signal(Vec::<ShopeePackage>::new);
    let mut input_title = use_signal(String::new);
    let mut input_store = use_signal(String::new);
    let mut input_code = use_signal(String::new);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let reload = move || {
        spawn(async move {
            match shopee_api::list_shopee().await {
                Ok(loaded) => items.set(loaded),
                Err(e) => error_msg.set(Some(format!("Failed to load: {e}"))),
            }
        });
    };

    use_effect(move || { reload(); });

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4",
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
                            match shopee_api::add_shopee(title, store, code).await {
                                Ok(()) => {
                                    input_title.set(String::new());
                                    input_store.set(String::new());
                                    input_code.set(String::new());
                                    reload();
                                }
                                Err(e) => error_msg.set(Some(format!("Failed to add: {e}"))),
                            }
                        });
                    },
                    input {
                        class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text outline-none focus:border-neon-orange/60 font-mono",
                        r#type: "text",
                        placeholder: "Package description...",
                        value: "{input_title}",
                        oninput: move |e| input_title.set(e.value()),
                    }
                    div { class: "flex gap-2",
                        input {
                            class: "flex-1 bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text outline-none focus:border-neon-orange/60 font-mono",
                            r#type: "text",
                            placeholder: "Store...",
                            value: "{input_store}",
                            oninput: move |e| input_store.set(e.value()),
                        }
                        input {
                            class: "w-24 bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text outline-none focus:border-neon-orange/60 font-mono",
                            r#type: "text",
                            placeholder: "Code",
                            value: "{input_code}",
                            oninput: move |e| input_code.set(e.value()),
                        }
                        ShopeeOcr {
                            on_results: move |results: Vec<OcrResult>| {
                                if results.len() == 1 {
                                    // Single package: fill the form
                                    let r = &results[0];
                                    if let Some(ref code) = r.code {
                                        input_code.set(code.clone());
                                    }
                                    if let Some(ref store) = r.store {
                                        input_store.set(store.clone());
                                    }
                                    if let Some(ref title) = r.title {
                                        input_title.set(title.clone());
                                    }
                                } else {
                                    // Multiple packages: auto-add all
                                    for r in results {
                                        let title = r.title.unwrap_or_else(|| "Shopee Package".to_string());
                                        let store = r.store;
                                        let code = r.code;
                                        spawn(async move {
                                            let _ = shopee_api::add_shopee(title, store, code).await;
                                            reload();
                                        });
                                    }
                                }
                            },
                        }
                    }
                    // Store quick-select
                    div { class: "relative",
                        div { class: "absolute right-0 top-0 bottom-1 w-8 bg-gradient-to-l from-cyber-card to-transparent pointer-events-none z-10 rounded-r-md" }
                        div { class: "flex gap-2 overflow-x-auto pb-1 scrollbar-hide",
                            for store in STORE_CHIPS {
                                { render_store_chip(store, input_store) }
                            }
                        }
                    }
                    button {
                        class: "w-full bg-neon-orange/20 text-neon-orange border border-neon-orange/40 rounded-lg px-4 py-2 text-xs font-bold tracking-wider uppercase hover:bg-neon-orange/30 transition-colors glow-orange",
                        r#type: "submit",
                        "ADD PACKAGE"
                    }
                }
            }

            div { class: "space-y-0",
                for pkg in items.read().iter() {
                    { render_package(pkg.clone(), reload, error_msg) }
                }
                if items.read().is_empty() {
                    div { class: "text-center py-12",
                        p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim", "No packages to pick up" }
                        p { class: "text-[10px] text-cyber-dim/50 mt-3 tracking-wider",
                            "SWIPE \u{2192} PICKED UP \u{2022} SWIPE \u{2190} DELETE"
                        }
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
            class: "shrink-0 whitespace-nowrap px-4 py-2.5 rounded-md text-xs font-medium tracking-wider bg-neon-orange/10 text-neon-orange border border-neon-orange/30 hover:bg-neon-orange/20 transition-colors",
            r#type: "button",
            onclick: move |_| input_store.set(store_clone.clone()),
            "{store}"
        }
    }
}

fn render_package(
    pkg: ShopeePackage,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let id = pkg.id.clone();
    let id2 = pkg.id.clone();
    let picked_up = pkg.picked_up;

    rsx! {
        SwipeItem {
            completed: picked_up,
            on_swipe_right: move |_| {
                let id = id.clone();
                spawn(async move {
                    match shopee_api::toggle_shopee(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to toggle: {e}"))),
                    }
                });
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    match shopee_api::delete_shopee(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to delete: {e}"))),
                    }
                });
            },
            div { class: "space-y-1",
                div { class: "flex items-center gap-2",
                    p { class: "text-sm font-medium flex-1", "{pkg.title}" }
                    if picked_up {
                        div { class: "text-right",
                            span { class: "text-xs text-neon-green font-bold tracking-wider", "PICKED UP" }
                            if let Some(by) = &pkg.completed_by {
                                p { class: "text-[10px] text-cyber-dim", "{by}" }
                            }
                        }
                    }
                }
                div { class: "flex gap-2 text-xs text-cyber-dim",
                    if let Some(store) = &pkg.store {
                        span { class: "bg-cyber-dark border border-cyber-border px-2 py-0.5 rounded font-mono", "{store}" }
                    }
                    if let Some(code) = &pkg.code {
                        span { class: "bg-neon-yellow/10 text-neon-yellow border border-neon-yellow/30 px-2 py-0.5 rounded font-mono", "{code}" }
                    }
                }
            }
        }
    }
}

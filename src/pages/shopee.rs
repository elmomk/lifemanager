use dioxus::prelude::*;

use crate::cache::{self, SyncStatus};
use crate::components::layout::SyncTrigger;
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
    let mut sync_status: Signal<SyncStatus> = use_context();
    let sync_trigger: Signal<SyncTrigger> = use_context();

    let reload = move || {
        spawn(async move {
            sync_status.set(SyncStatus::Syncing);
            match shopee_api::list_shopee().await {
                Ok(loaded) => {
                    cache::write("shopee", &loaded);
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
        if let Some(cached) = cache::read::<Vec<ShopeePackage>>("shopee") {
            items.set(cached);
        }
        reload();
    });

    use_effect(move || {
        let _trigger = sync_trigger.read().0;
        reload();
    });

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
                    div { class: "flex gap-2 items-start",
                        input {
                            class: "flex-1 min-w-0 bg-cyber-dark border border-cyber-border rounded-lg px-3 py-2 text-sm text-cyber-text outline-none focus:border-neon-orange/60 font-mono",
                            r#type: "text",
                            placeholder: "Store...",
                            value: "{input_store}",
                            oninput: move |e| input_store.set(e.value()),
                        }
                        input {
                            class: "w-20 bg-cyber-dark border border-cyber-border rounded-lg px-3 py-2 text-sm text-cyber-text outline-none focus:border-neon-orange/60 font-mono",
                            r#type: "text",
                            placeholder: "Code",
                            value: "{input_code}",
                            oninput: move |e| input_code.set(e.value()),
                        }
                        ShopeeOcr {
                            on_results: move |results: Vec<OcrResult>| {
                                // Check for matches against existing packages, then add/update
                                let current_items: Vec<ShopeePackage> = items.read().clone();

                                for r in results {
                                    let title = r.title.clone().unwrap_or_else(|| "Shopee Package".to_string());
                                    let store = r.store.clone();
                                    let code = r.code.clone();

                                    // Try to find a matching existing package
                                    let matching = current_items.iter().find(|pkg| {
                                        if pkg.picked_up { return false; }
                                        // Match by title substring
                                        if let Some(ref ocr_title) = r.title {
                                            let ocr_clean = ocr_title.replace("【", "").replace("】", "");
                                            let pkg_clean = pkg.title.replace("【", "").replace("】", "");
                                            if !ocr_clean.is_empty() && (pkg_clean.contains(&ocr_clean) || ocr_clean.contains(&pkg_clean)) {
                                                return true;
                                            }
                                        }
                                        // Match by store + code
                                        if let (Some(ref pkg_store), Some(ref ocr_store)) = (&pkg.store, &r.store) {
                                            if pkg_store == ocr_store {
                                                if let (Some(ref pkg_code), Some(ref ocr_code)) = (&pkg.code, &r.code) {
                                                    return pkg_code == ocr_code;
                                                }
                                            }
                                        }
                                        false
                                    });

                                    if let Some(existing) = matching {
                                        // Update: add code to existing package if it was missing
                                        if existing.code.is_none() {
                                            if let Some(ref new_code) = code {
                                                let id = existing.id.clone();
                                                let code_val = new_code.clone();
                                                spawn(async move {
                                                    let _ = shopee_api::update_shopee_code(id, code_val).await;
                                                    reload();
                                                });
                                            }
                                        }
                                        // Already exists with code — skip
                                    } else {
                                        // New package — add it
                                        let title = title.clone();
                                        let store = store.clone();
                                        let code = code.clone();
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
                    div { class: "text-center py-16",
                        p { class: "text-2xl mb-3 opacity-30", "\u{1F4E6}" }
                        p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim", "No packages to pick up" }
                        p { class: "text-[10px] text-cyber-dim/40 mt-2 tracking-wider",
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

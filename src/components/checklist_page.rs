use dioxus::prelude::*;

use crate::api::checklist;
use crate::api::defaults;
use crate::components::error_banner::ErrorBanner;
use crate::components::quick_add::QuickAdd;
use crate::components::swipe_item::SwipeItem;
use crate::models::{ChecklistItem, ItemCategory};

#[component]
pub fn ChecklistPage(
    category: ItemCategory,
    placeholder: &'static str,
    initial_chips: Vec<String>,
    empty_text: &'static str,
    done_label: &'static str,
    accent_color: &'static str,
) -> Element {
    let mut items = use_signal(Vec::<ChecklistItem>::new);
    let mut chips = use_signal(Vec::<String>::new);
    let mut input_text = use_signal(String::new);
    let mut input_date = use_signal(|| Option::<String>::None);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| false);
    let seed_chips = use_signal(move || initial_chips.clone());

    let reload = move || {
        spawn(async move {
            match checklist::list_checklist(category).await {
                Ok(loaded) => items.set(loaded),
                Err(e) => error_msg.set(Some(format!("Failed to load: {e}"))),
            }
        });
    };

    let reload_chips = move || {
        let fb = seed_chips.read().clone();
        spawn(async move {
            match defaults::list_defaults(category).await {
                Ok(loaded) => {
                    if loaded.is_empty() {
                        for chip in &fb {
                            let _ = defaults::add_default(chip.clone(), category).await;
                        }
                        chips.set(fb);
                    } else {
                        chips.set(loaded);
                    }
                }
                Err(_) => chips.set(fb),
            }
        });
    };

    // Load on mount
    use_effect(move || {
        reload();
        reload_chips();
    });

    let mut do_add = move |text: String| {
        if text.trim().is_empty() {
            return;
        }
        let date = input_date.read().clone();
        loading.set(true);
        spawn(async move {
            match checklist::add_checklist(text, category, date).await {
                Ok(()) => {
                    input_text.set(String::new());
                    input_date.set(None);
                    reload();
                }
                Err(e) => error_msg.set(Some(format!("Failed to add: {e}"))),
            }
            loading.set(false);
        });
    };

    let (btn_class, input_class) = match accent_color {
        "green" => (
            "bg-neon-green/20 text-neon-green border border-neon-green/40 rounded-lg px-4 py-2.5 text-xs font-bold tracking-wider uppercase hover:bg-neon-green/30 transition-colors glow-green disabled:opacity-50",
            "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2.5 text-sm text-cyber-text outline-none focus:border-neon-green/60 font-mono",
        ),
        _ => (
            "bg-neon-cyan/20 text-neon-cyan border border-neon-cyan/40 rounded-lg px-4 py-2.5 text-xs font-bold tracking-wider uppercase hover:bg-neon-cyan/30 transition-colors glow-cyan disabled:opacity-50",
            "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2.5 text-sm text-cyber-text outline-none focus:border-neon-cyan/60 font-mono",
        ),
    };

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            // Add form
            div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-3",
                form {
                    class: "space-y-2",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let text = input_text.read().clone();
                        do_add(text);
                    },
                    // Row 1: text input full width
                    input {
                        class: input_class,
                        r#type: "text",
                        placeholder: placeholder,
                        value: "{input_text}",
                        oninput: move |e| input_text.set(e.value()),
                    }
                    // Row 2: date + ADD button
                    div { class: "flex gap-2",
                        input {
                            class: "flex-1 min-w-0 bg-cyber-dark border border-cyber-border rounded-lg px-3 py-2.5 text-sm text-cyber-text font-mono",
                            r#type: "date",
                            value: input_date.read().as_deref().unwrap_or(""),
                            oninput: move |e| {
                                let v = e.value();
                                input_date.set(if v.is_empty() { None } else { Some(v) });
                            },
                        }
                        button {
                            class: btn_class,
                            r#type: "submit",
                            disabled: loading(),
                            if loading() {
                                svg {
                                    class: "w-4 h-4 animate-spin",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    circle {
                                        cx: "12", cy: "12", r: "10",
                                        stroke: "currentColor", stroke_width: "4",
                                        class: "opacity-25",
                                    }
                                    path {
                                        d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z",
                                        fill: "currentColor",
                                        class: "opacity-75",
                                    }
                                }
                            } else {
                                "ADD"
                            }
                        }
                    }
                }

                // Quick add chips
                div { class: "mt-3",
                    QuickAdd {
                        chips: chips.read().clone(),
                        on_select: move |text: String| {
                            do_add(text);
                        },
                        on_delete: move |text: String| {
                            spawn(async move {
                                match defaults::delete_default(text, category).await {
                                    Ok(()) => reload_chips(),
                                    Err(e) => error_msg.set(Some(format!("Failed to remove: {e}"))),
                                }
                            });
                        },
                    }
                }
            }

            // Items list
            div { class: "space-y-0",
                for item in items.read().iter() {
                    { render_checklist_item(item.clone(), done_label, category, reload, reload_chips, error_msg) }
                }
                if items.read().is_empty() {
                    div { class: "text-center py-12",
                        p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim", "{empty_text}" }
                        p { class: "text-[10px] text-cyber-dim/50 mt-3 tracking-wider",
                            "SWIPE \u{2192} COMPLETE \u{2022} SWIPE \u{2190} DELETE"
                        }
                    }
                }
            }
        }
    }
}

fn render_checklist_item(
    item: ChecklistItem,
    done_label: &'static str,
    category: ItemCategory,
    reload: impl Fn() + Copy + 'static,
    reload_chips: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let id = item.id.clone();
    let id2 = item.id.clone();
    let done = item.done;
    let item_text = item.text.clone();

    rsx! {
        SwipeItem {
            completed: done,
            on_swipe_right: move |_| {
                if done {
                    let text = item_text.clone();
                    spawn(async move {
                        match defaults::add_default(text, category).await {
                            Ok(()) => reload_chips(),
                            Err(e) => error_msg.set(Some(format!("Failed to save default: {e}"))),
                        }
                    });
                } else {
                    let id = id.clone();
                    spawn(async move {
                        match checklist::toggle_checklist(id).await {
                            Ok(()) => reload(),
                            Err(e) => error_msg.set(Some(format!("Failed to toggle: {e}"))),
                        }
                    });
                }
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    match checklist::delete_checklist(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to delete: {e}"))),
                    }
                });
            },
            div { class: "flex items-center gap-3",
                div { class: "flex-1",
                    p { class: "text-sm font-medium", "{item.text}" }
                    if let Some(date) = &item.date {
                        span { class: "text-xs text-cyber-dim font-mono", "{date}" }
                    }
                }
                if item.done {
                    div { class: "text-right",
                        span { class: "text-xs text-neon-green font-bold tracking-wider", "{done_label}" }
                        if let Some(by) = &item.completed_by {
                            p { class: "text-[10px] text-cyber-dim", "{by}" }
                        }
                    }
                }
            }
        }
    }
}

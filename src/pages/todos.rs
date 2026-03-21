use dioxus::prelude::*;

use crate::api::checklist;
use crate::components::quick_add::QuickAdd;
use crate::components::swipe_item::SwipeItem;
use crate::models::{ChecklistItem, ItemCategory};

const CHIPS: &[&str] = &["Laundry", "Clean", "Pay Bills", "Exercise", "Cook", "Study"];

#[component]
pub fn Todos() -> Element {
    let mut items = use_signal(Vec::<ChecklistItem>::new);
    let mut input_text = use_signal(String::new);
    let mut input_date = use_signal(|| Option::<String>::None);
    let mut refresh = use_signal(|| 0u32);

    // Load items
    use_effect(move || {
        let _ = refresh();
        spawn(async move {
            match checklist::list_checklist(ItemCategory::Todo).await {
                Ok(loaded) => items.set(loaded),
                Err(e) => tracing::error!("Failed to load todos: {e}"),
            }
        });
    });

    let add_item = move |text: String| {
        if text.trim().is_empty() {
            return;
        }
        let date = input_date.read().clone();
        spawn(async move {
            if checklist::add_checklist(text, ItemCategory::Todo, date).await.is_ok() {
                input_text.set(String::new());
                input_date.set(None);
                refresh.set(refresh() + 1);
            }
        });
    };

    rsx! {
        div { class: "p-4 space-y-4",
            // Add form
            div { class: "bg-white/70 dark:bg-gray-800/70 backdrop-blur-lg rounded-2xl p-4 shadow-sm",
                form {
                    class: "flex gap-2",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let text = input_text.read().clone();
                        add_item(text);
                    },
                    input {
                        class: "flex-1 bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-blue-500",
                        r#type: "text",
                        placeholder: "Add a task...",
                        value: "{input_text}",
                        oninput: move |e| input_text.set(e.value()),
                    }
                    input {
                        class: "bg-gray-100 dark:bg-gray-700 rounded-xl px-3 py-2 text-sm",
                        r#type: "date",
                        value: input_date.read().as_deref().unwrap_or(""),
                        oninput: move |e| {
                            let v = e.value();
                            input_date.set(if v.is_empty() { None } else { Some(v) });
                        },
                    }
                    button {
                        class: "bg-blue-500 text-white rounded-xl px-4 py-2 text-sm font-medium hover:bg-blue-600 transition-colors",
                        r#type: "submit",
                        "Add"
                    }
                }

                // Quick add chips
                div { class: "mt-3",
                    QuickAdd {
                        chips: CHIPS.iter().map(|s| s.to_string()).collect(),
                        on_select: move |text: String| {
                            add_item(text);
                        },
                    }
                }
            }

            // Items list
            div { class: "space-y-0",
                for item in items.read().iter() {
                    { render_item(item.clone(), refresh) }
                }
                if items.read().is_empty() {
                    div { class: "text-center text-gray-400 dark:text-gray-600 py-8",
                        p { "No tasks yet" }
                    }
                }
            }
        }
    }
}

fn render_item(item: ChecklistItem, mut refresh: Signal<u32>) -> Element {
    let id = item.id.clone();
    let id2 = item.id.clone();
    let done = item.done;

    rsx! {
        SwipeItem {
            completed: done,
            on_swipe_right: move |_| {
                let id = id.clone();
                spawn(async move {
                    if checklist::toggle_checklist(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            on_swipe_left: move |_| {
                let id = id2.clone();
                spawn(async move {
                    if checklist::delete_checklist(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            div { class: "flex items-center gap-3",
                div { class: "flex-1",
                    p { class: "text-sm font-medium", "{item.text}" }
                    if let Some(date) = &item.date {
                        span { class: "text-xs text-gray-400", "{date}" }
                    }
                }
                if item.done {
                    span { class: "text-xs text-green-500 font-medium", "Done" }
                }
            }
        }
    }
}

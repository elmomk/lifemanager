use dioxus::prelude::*;

#[component]
pub fn QuickAdd(chips: Vec<String>, on_select: EventHandler<String>) -> Element {
    rsx! {
        div { class: "flex gap-2 overflow-x-auto pb-2 scrollbar-hide",
            for chip in chips {
                { render_chip(chip.clone(), on_select) }
            }
        }
    }
}

fn render_chip(label: String, on_select: EventHandler<String>) -> Element {
    let label_clone = label.clone();
    rsx! {
        button {
            class: "shrink-0 px-3 py-1.5 rounded-full text-sm font-medium bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 hover:bg-blue-200 dark:hover:bg-blue-800 transition-colors",
            onclick: move |_| on_select.call(label_clone.clone()),
            "{label}"
        }
    }
}

use dioxus::prelude::*;

#[component]
pub fn QuickAdd(
    chips: Vec<String>,
    on_select: EventHandler<String>,
    on_delete: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        div { class: "relative",
            div { class: "absolute right-0 top-0 bottom-2 w-8 bg-gradient-to-l from-cyber-card to-transparent pointer-events-none z-10 rounded-r-md" }
            div { class: "flex gap-2 overflow-x-auto pb-2 scrollbar-hide",
                for chip in chips {
                    { render_chip(chip.clone(), on_select, on_delete) }
                }
            }
        }
    }
}

fn render_chip(
    label: String,
    on_select: EventHandler<String>,
    on_delete: Option<EventHandler<String>>,
) -> Element {
    let label_tap = label.clone();
    let label_del = label.clone();

    rsx! {
        div { class: "shrink-0 relative",
            // Delete badge (top-right X)
            if let Some(ref handler) = on_delete {
                {
                    let handler = handler.clone();
                    rsx! {
                        button {
                            class: "absolute -top-1.5 -right-1.5 z-10 w-5 h-5 flex items-center justify-center rounded-full bg-neon-magenta/80 text-white text-[9px] font-bold leading-none hover:bg-neon-magenta transition-colors",
                            onclick: move |e| {
                                e.stop_propagation();
                                handler.call(label_del.clone());
                            },
                            "\u{00d7}"
                        }
                    }
                }
            }
            // Chip button
            button {
                class: "whitespace-nowrap px-4 py-2.5 rounded-md text-xs font-medium tracking-wider uppercase bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/30 hover:bg-neon-cyan/20 transition-colors",
                onclick: move |_| {
                    on_select.call(label_tap.clone());
                },
                "{label}"
            }
        }
    }
}

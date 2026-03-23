use dioxus::prelude::*;

#[component]
pub fn QuickAdd(
    chips: Vec<String>,
    on_select: EventHandler<String>,
    on_delete: Option<EventHandler<String>>,
    #[props(default = "cyan")] accent: &'static str,
) -> Element {
    let mut editing = use_signal(|| false);
    let has_delete = on_delete.is_some();

    let (chip_bg, chip_text, chip_border) = match accent {
        "green" => ("bg-neon-green/10", "text-neon-green", "border-neon-green/30"),
        "orange" => ("bg-neon-orange/10", "text-neon-orange", "border-neon-orange/30"),
        "purple" => ("bg-neon-purple/10", "text-neon-purple", "border-neon-purple/30"),
        _ => ("bg-neon-cyan/10", "text-neon-cyan", "border-neon-cyan/30"),
    };

    rsx! {
        div { class: "relative",
            div { class: "absolute right-0 top-0 bottom-2 w-8 bg-gradient-to-l from-cyber-card to-transparent pointer-events-none z-10 rounded-r-md" }
            div { class: "flex gap-2 overflow-x-auto pb-2 scrollbar-hide items-center",
                // Edit toggle button
                if has_delete {
                    button {
                        class: "shrink-0 w-9 h-9 flex items-center justify-center rounded-md border transition-colors {chip_text}",
                        class: if editing() { "bg-neon-magenta/20 border-neon-magenta/40 !text-neon-magenta" } else { "{chip_bg} {chip_border}" },
                        onclick: move |_| editing.set(!editing()),
                        if editing() {
                            "\u{2713}"  // checkmark
                        } else {
                            "\u{270E}"  // pencil
                        }
                    }
                }
                for chip in chips {
                    { render_chip(chip.clone(), on_select, on_delete, editing(), chip_bg, chip_text, chip_border) }
                }
            }
        }
    }
}

fn render_chip(
    label: String,
    on_select: EventHandler<String>,
    on_delete: Option<EventHandler<String>>,
    editing: bool,
    chip_bg: &str,
    chip_text: &str,
    chip_border: &str,
) -> Element {
    let label_tap = label.clone();
    let label_del = label.clone();

    let show_delete = editing && on_delete.is_some();

    rsx! {
        div { class: "shrink-0 flex items-center rounded-md border transition-all {chip_bg} {chip_border}",
            class: if show_delete { "pr-0" } else { "" },
            // Chip label — tap to add
            button {
                class: "whitespace-nowrap px-3 py-2 text-xs font-medium tracking-wider uppercase {chip_text} transition-colors",
                onclick: move |_| {
                    if !editing {
                        on_select.call(label_tap.clone());
                    }
                },
                "{label}"
            }
            // Inline delete × (only in edit mode)
            if show_delete {
                if let Some(ref handler) = on_delete {
                    {
                        let handler = handler.clone();
                        rsx! {
                            button {
                                class: "px-2 py-2 text-xs text-neon-magenta/60 hover:text-neon-magenta border-l border-neon-magenta/20 transition-colors",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    handler.call(label_del.clone());
                                },
                                "\u{00d7}"
                            }
                        }
                    }
                }
            }
        }
    }
}

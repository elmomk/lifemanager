use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

use crate::cache::{self, SyncStatus};
use crate::components::layout::SyncTrigger;
use crate::api::cycles as cycles_api;
use crate::components::error_banner::ErrorBanner;
use crate::components::swipe_item::SwipeItem;
use crate::models::Cycle;

const SYMPTOM_CHIPS: &[&str] = &["Cramps", "Headache", "Fatigue", "Bloating", "Mood Swings", "Back Pain", "Nausea"];

#[component]
pub fn Period() -> Element {
    let mut cycles = use_signal(Vec::<Cycle>::new);
    let mut input_date = use_signal(|| Local::now().format("%Y-%m-%d").to_string());
    let mut input_end_date = use_signal(|| Option::<String>::None);
    let mut selected_symptoms = use_signal(Vec::<String>::new);
    let mut show_form = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut sync_status: Signal<SyncStatus> = use_context();
    let sync_trigger: Signal<SyncTrigger> = use_context();

    let reload = move || {
        spawn(async move {
            sync_status.set(SyncStatus::Syncing);
            match cycles_api::list_cycles().await {
                Ok(loaded) => {
                    cache::write("cycles", &loaded);
                    cache::write_sync_time();
                    cycles.set(loaded);
                    sync_status.set(SyncStatus::Synced);
                }
                Err(e) => {
                    if cycles.read().is_empty() {
                        error_msg.set(Some(format!("Failed to load: {e}")));
                    }
                    sync_status.set(SyncStatus::CachedOnly);
                }
            }
        });
    };

    use_effect(move || {
        if let Some(cached) = cache::read::<Vec<Cycle>>("cycles") {
            cycles.set(cached);
        }
        reload();
    });

    use_effect(move || {
        let _trigger = sync_trigger.read().0;
        reload();
    });

    let cycle_count = cycles.read().len();

    let prediction = {
        let c = cycles.read();
        Cycle::predict_next_start(&c)
    };

    let countdown = prediction.map(|pred| {
        let today = Local::now().date_naive();
        (pred - today).num_days()
    });

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            // Prediction card
            if let Some(pred) = prediction {
                div { class: "bg-cyber-card border border-neon-pink/30 rounded-xl p-6 text-center glow-pink",
                    p { class: "text-xs text-cyber-dim tracking-widest uppercase mb-1", "NEXT EXPECTED" }
                    p { class: "text-2xl font-bold text-neon-pink text-glow-pink font-mono", "{pred}" }
                    if let Some(days) = countdown {
                        p { class: "text-xs text-cyber-dim mt-1 tracking-wider",
                            if days == 0 {
                                "TODAY"
                            } else if days == 1 {
                                "TOMORROW"
                            } else if days > 0 {
                                "IN {days} DAYS"
                            } else {
                                { format!("{} DAYS AGO", days.abs()) }
                            }
                        }
                    }
                    if cycle_count < 3 {
                        {
                            let suffix = if cycle_count != 1 { "S" } else { "" };
                            rsx! {
                                p { class: "text-[10px] text-cyber-dim/50 mt-2 tracking-wider",
                                    "BASED ON {cycle_count} CYCLE{suffix} \u{2022} MORE DATA = BETTER ACCURACY"
                                }
                            }
                        }
                    }
                }
            }

            // PMS care card — shows 1-10 days before predicted period
            if let Some(days) = countdown {
                if days >= 1 && days <= 10 {
                    div { class: "bg-gradient-to-br from-[#1a1028] to-[#0f1528] border border-neon-purple/30 rounded-xl p-5 glow-purple",
                        div { class: "flex items-start gap-4",
                            // Adorable character — a little blanket-wrapped moon
                            div { class: "text-4xl flex-shrink-0", "\u{1F319}" }
                            div { class: "flex-1",
                                p { class: "text-sm font-bold text-neon-purple mb-1",
                                    if days <= 3 {
                                        "Almost time... be extra gentle \u{1F49C}"
                                    } else if days <= 7 {
                                        "Hey lovely, take it easy \u{2728}"
                                    } else {
                                        "Heads up, sweetheart \u{1F33C}"
                                    }
                                }
                                p { class: "text-xs text-cyber-dim leading-relaxed",
                                    if days <= 3 {
                                        "Your period is just {days} days away. Cozy blankets, warm drinks, and extra hugs recommended. You're doing amazing."
                                    } else if days <= 7 {
                                        "About {days} days to go. If you're feeling a bit down or emotional, that's completely normal. Be kind to yourself \u{1F49B}"
                                    } else {
                                        "~{days} days until your next cycle. A good time to stock up on snacks and schedule some self-care \u{1F60A}"
                                    }
                                }
                                // Self-care suggestions
                                div { class: "flex flex-wrap gap-1.5 mt-3",
                                    for tip in &["Hot tea \u{2615}", "Chocolate \u{1F36B}", "Warm bath \u{1F6C1}", "Rest \u{1F4A4}"] {
                                        span { class: "text-[10px] bg-neon-purple/10 text-neon-purple/80 border border-neon-purple/20 px-2 py-1 rounded-full",
                                            "{tip}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Log button / form
            if *show_form.read() {
                div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4 space-y-3",
                    div { class: "flex gap-2",
                        div { class: "flex-1",
                            label { class: "text-xs text-cyber-dim tracking-widest uppercase mb-1 block", "START DATE" }
                            input {
                                class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text font-mono",
                                r#type: "date",
                                value: "{input_date}",
                                oninput: move |e| input_date.set(e.value()),
                            }
                        }
                        div { class: "flex-1",
                            label { class: "text-xs text-cyber-dim tracking-widest uppercase mb-1 block", "END DATE" }
                            input {
                                class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text font-mono",
                                r#type: "date",
                                value: input_end_date.read().as_deref().unwrap_or(""),
                                oninput: move |e| {
                                    let v = e.value();
                                    input_end_date.set(if v.is_empty() { None } else { Some(v) });
                                },
                            }
                        }
                    }
                    // Symptom chips
                    label { class: "text-[10px] text-cyber-dim tracking-widest uppercase block", "SYMPTOMS" }
                    div { class: "flex flex-wrap gap-2",
                        for symptom in SYMPTOM_CHIPS {
                            { render_symptom_chip(symptom, selected_symptoms) }
                        }
                    }
                    div { class: "flex gap-2",
                        button {
                            class: "flex-1 bg-cyber-dark border border-cyber-border rounded-lg px-4 py-3 text-xs font-medium tracking-wider text-cyber-dim",
                            onclick: move |_| show_form.set(false),
                            "CANCEL"
                        }
                        button {
                            class: "flex-1 bg-neon-pink/20 text-neon-pink border border-neon-pink/40 rounded-lg px-4 py-3 text-xs font-bold tracking-wider hover:bg-neon-pink/30 transition-colors glow-pink",
                            onclick: move |_| {
                                let start = input_date.read().clone();
                                if NaiveDate::parse_from_str(&start, "%Y-%m-%d").is_ok() {
                                    let end = input_end_date.read().clone();
                                    let symptoms = selected_symptoms.read().clone();
                                    spawn(async move {
                                        match cycles_api::add_cycle(start, end, symptoms).await {
                                            Ok(()) => {
                                                show_form.set(false);
                                                selected_symptoms.set(Vec::new());
                                                reload();
                                            }
                                            Err(e) => error_msg.set(Some(format!("Failed to log: {e}"))),
                                        }
                                    });
                                }
                            },
                            "LOG CYCLE"
                        }
                    }
                }
            } else {
                button {
                    class: "w-full bg-neon-pink/20 text-neon-pink border border-neon-pink/40 rounded-xl px-4 py-3 text-xs font-bold tracking-wider uppercase hover:bg-neon-pink/30 transition-colors glow-pink",
                    onclick: move |_| show_form.set(true),
                    "LOG NEW CYCLE"
                }
            }

            // History
            div { class: "space-y-0",
                for cycle in cycles.read().iter() {
                    { render_cycle(cycle.clone(), reload, error_msg) }
                }
                if cycles.read().is_empty() {
                    div { class: "text-center py-16",
                        p { class: "text-2xl mb-3 opacity-30", "\u{1F319}" }
                        p { class: "text-xs tracking-[0.3em] uppercase text-cyber-dim", "No cycles logged yet" }
                        p { class: "text-[10px] text-cyber-dim/40 mt-2 tracking-wider",
                            "SWIPE \u{2190} DELETE"
                        }
                    }
                }
            }
        }
    }
}

fn render_symptom_chip(symptom: &&str, mut selected: Signal<Vec<String>>) -> Element {
    let s = symptom.to_string();
    let is_active = selected.read().contains(&s);
    let bg = if is_active {
        "bg-neon-pink/30 text-neon-pink border-neon-pink/60"
    } else {
        "bg-cyber-dark text-cyber-dim border-cyber-border"
    };
    let s_clone = s.clone();

    rsx! {
        button {
            class: "px-4 py-2.5 rounded-md text-xs font-medium tracking-wider border {bg} transition-colors",
            r#type: "button",
            onclick: move |_| {
                let mut current = selected.read().clone();
                if current.contains(&s_clone) {
                    current.retain(|x| x != &s_clone);
                } else {
                    current.push(s_clone.clone());
                }
                selected.set(current);
            },
            "{s}"
        }
    }
}

fn render_cycle(
    cycle: Cycle,
    reload: impl Fn() + Copy + 'static,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let id = cycle.id.clone();

    rsx! {
        SwipeItem {
            completed: false,
            on_swipe_left: move |_| {
                let id = id.clone();
                spawn(async move {
                    match cycles_api::delete_cycle(id).await {
                        Ok(()) => reload(),
                        Err(e) => error_msg.set(Some(format!("Failed to delete: {e}"))),
                    }
                });
            },
            div { class: "space-y-1",
                div { class: "flex items-center gap-2",
                    p { class: "text-sm font-bold font-mono text-neon-pink", "{cycle.start_date}" }
                    if let Some(end) = &cycle.end_date {
                        span { class: "text-xs text-cyber-dim font-mono", "to {end}" }
                    }
                    if let Some(days) = cycle.duration_days() {
                        span { class: "text-[10px] bg-neon-pink/10 text-neon-pink border border-neon-pink/30 px-2 py-0.5 rounded font-mono",
                            "{days}d"
                        }
                    }
                }
                if !cycle.symptoms.is_empty() {
                    div { class: "flex flex-wrap gap-1",
                        for symptom in &cycle.symptoms {
                            span { class: "text-[10px] bg-cyber-dark border border-cyber-border px-2 py-0.5 rounded text-cyber-dim font-mono",
                                "{symptom}"
                            }
                        }
                    }
                }
            }
        }
    }
}

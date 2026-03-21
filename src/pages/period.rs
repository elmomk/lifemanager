use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

use crate::api::cycles as cycles_api;
use crate::components::swipe_item::SwipeItem;
use crate::models::Cycle;

const SYMPTOM_CHIPS: &[&str] = &["Cramps", "Headache", "Fatigue", "Bloating", "Mood Swings", "Back Pain", "Nausea"];

#[component]
pub fn Period() -> Element {
    let mut cycles = use_signal(Vec::<Cycle>::new);
    let mut input_date = use_signal(|| Local::now().format("%Y-%m-%d").to_string());
    let mut input_end_date = use_signal(|| Option::<String>::None);
    let mut selected_symptoms = use_signal(Vec::<String>::new);
    let mut refresh = use_signal(|| 0u32);
    let mut show_form = use_signal(|| false);

    use_effect(move || {
        let _ = refresh();
        spawn(async move {
            match cycles_api::list_cycles().await {
                Ok(loaded) => cycles.set(loaded),
                Err(e) => tracing::error!("Failed to load cycles: {e}"),
            }
        });
    });

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
            // Prediction card
            if let Some(pred) = prediction {
                div { class: "bg-gradient-to-r from-pink-500/10 to-purple-500/10 dark:from-pink-500/20 dark:to-purple-500/20 backdrop-blur-lg rounded-3xl p-6 text-center",
                    p { class: "text-sm text-gray-500 dark:text-gray-400 mb-1", "Next expected" }
                    p { class: "text-2xl font-bold text-pink-600 dark:text-pink-400", "{pred}" }
                    if let Some(days) = countdown {
                        p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1",
                            if days == 0 {
                                "Today"
                            } else if days == 1 {
                                "Tomorrow"
                            } else if days > 0 {
                                "in {days} days"
                            } else {
                                { format!("{} days ago", days.abs()) }
                            }
                        }
                    }
                }
            }

            // Log button / form
            if *show_form.read() {
                div { class: "bg-white/70 dark:bg-gray-800/70 backdrop-blur-lg rounded-2xl p-4 shadow-sm space-y-3",
                    div { class: "flex gap-2",
                        div { class: "flex-1",
                            label { class: "text-xs text-gray-500 mb-1 block", "Start Date" }
                            input {
                                class: "w-full bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm",
                                r#type: "date",
                                value: "{input_date}",
                                oninput: move |e| input_date.set(e.value()),
                            }
                        }
                        div { class: "flex-1",
                            label { class: "text-xs text-gray-500 mb-1 block", "End Date (optional)" }
                            input {
                                class: "w-full bg-gray-100 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm",
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
                    label { class: "text-xs text-gray-500 block", "Symptoms" }
                    div { class: "flex flex-wrap gap-2",
                        for symptom in SYMPTOM_CHIPS {
                            { render_symptom_chip(symptom, selected_symptoms) }
                        }
                    }
                    div { class: "flex gap-2",
                        button {
                            class: "flex-1 bg-gray-200 dark:bg-gray-700 rounded-xl px-4 py-2 text-sm font-medium",
                            onclick: move |_| show_form.set(false),
                            "Cancel"
                        }
                        button {
                            class: "flex-1 bg-pink-500 text-white rounded-xl px-4 py-2 text-sm font-medium hover:bg-pink-600 transition-colors",
                            onclick: move |_| {
                                let start = input_date.read().clone();
                                if NaiveDate::parse_from_str(&start, "%Y-%m-%d").is_ok() {
                                    let end = input_end_date.read().clone();
                                    let symptoms = selected_symptoms.read().clone();
                                    spawn(async move {
                                        if cycles_api::add_cycle(start, end, symptoms).await.is_ok() {
                                            show_form.set(false);
                                            selected_symptoms.set(Vec::new());
                                            refresh.set(refresh() + 1);
                                        }
                                    });
                                }
                            },
                            "Log Cycle"
                        }
                    }
                }
            } else {
                button {
                    class: "w-full bg-pink-500 text-white rounded-2xl px-4 py-3 text-sm font-medium hover:bg-pink-600 transition-colors",
                    onclick: move |_| show_form.set(true),
                    "Log New Cycle"
                }
            }

            // History
            div { class: "space-y-0",
                for cycle in cycles.read().iter() {
                    { render_cycle(cycle.clone(), refresh) }
                }
                if cycles.read().is_empty() {
                    div { class: "text-center text-gray-400 dark:text-gray-600 py-8",
                        p { "No cycles logged yet" }
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
        "bg-pink-500 text-white"
    } else {
        "bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
    };
    let s_clone = s.clone();

    rsx! {
        button {
            class: "px-3 py-1.5 rounded-full text-xs font-medium {bg} transition-colors",
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

fn render_cycle(cycle: Cycle, mut refresh: Signal<u32>) -> Element {
    let id = cycle.id.clone();

    rsx! {
        SwipeItem {
            completed: false,
            on_swipe_left: move |_| {
                let id = id.clone();
                spawn(async move {
                    if cycles_api::delete_cycle(id).await.is_ok() {
                        refresh.set(refresh() + 1);
                    }
                });
            },
            div { class: "space-y-1",
                div { class: "flex items-center gap-2",
                    p { class: "text-sm font-medium", "{cycle.start_date}" }
                    if let Some(end) = &cycle.end_date {
                        span { class: "text-xs text-gray-400", "to {end}" }
                    }
                    if let Some(days) = cycle.duration_days() {
                        span { class: "text-xs bg-pink-100 dark:bg-pink-900/50 text-pink-600 dark:text-pink-400 px-2 py-0.5 rounded-lg",
                            "{days} days"
                        }
                    }
                }
                if !cycle.symptoms.is_empty() {
                    div { class: "flex flex-wrap gap-1",
                        for symptom in &cycle.symptoms {
                            span { class: "text-xs bg-gray-100 dark:bg-gray-700 px-2 py-0.5 rounded-lg text-gray-500",
                                "{symptom}"
                            }
                        }
                    }
                }
            }
        }
    }
}

use chrono::{Datelike, Local, NaiveDate};
use dioxus::prelude::*;
use std::collections::HashMap;

use crate::cache::{self, SyncStatus};
use crate::components::layout::SyncTrigger;
use crate::api::cycles as cycles_api;
use crate::api::mood as mood_api;
use crate::components::error_banner::ErrorBanner;
use crate::components::swipe_item::SwipeItem;
use crate::models::{Cycle, CycleSettings, PhaseInfo, PhaseInsight, CyclePhase, MoodEntry, current_phase, birth_control_phase, phase_for_date};

const SYMPTOM_CHIPS: &[&str] = &["Cramps", "Headache", "Fatigue", "Bloating", "Mood Swings", "Back Pain", "Nausea"];
const MOOD_EMOJIS: &[&str] = &["\u{1F622}", "\u{1F615}", "\u{1F610}", "\u{1F642}", "\u{1F60A}"]; // 😢😕😐🙂😊

#[component]
pub fn Period() -> Element {
    let mut cycles = use_signal(Vec::<Cycle>::new);
    let mut settings = use_signal(CycleSettings::default);
    let mut input_date = use_signal(|| Local::now().format("%Y-%m-%d").to_string());
    let mut input_end_date = use_signal(|| Option::<String>::None);
    let mut selected_symptoms = use_signal(Vec::<String>::new);
    let mut show_form = use_signal(|| false);
    // Panels are mutually exclusive: only one of calendar/insights/settings open at a time
    let mut active_panel = use_signal(|| Option::<&'static str>::None);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut sync_status: Signal<SyncStatus> = use_context();
    let sync_trigger: Signal<SyncTrigger> = use_context();

    // Mood logger state
    let mut mood_val = use_signal(|| Option::<i32>::None);
    let mut energy_val = use_signal(|| Option::<i32>::None);
    let mut libido_val = use_signal(|| Option::<i32>::None);
    let mut mood_notes = use_signal(String::new);
    let mut mood_saved = use_signal(|| false);
    let mut checkin_editing = use_signal(|| false);
    let mut checkin_open = use_signal(|| false); // for unlogged: tap to expand form
    let dashboard_expanded = use_signal(|| false);
    let mut pms_expanded = use_signal(|| false);
    let mut show_all_history = use_signal(|| false);
    let mut insights = use_signal(Vec::<PhaseInsight>::new);

    // Calendar state
    let calendar_month = use_signal(|| {
        let now = Local::now().date_naive();
        NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap()
    });
    let mut mood_logs = use_signal(Vec::<MoodEntry>::new);
    let selected_day = use_signal(|| Option::<NaiveDate>::None);

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

    // Load cycles + settings + today's mood on mount
    use_effect(move || {
        if let Some(cached) = cache::read::<Vec<Cycle>>("cycles") {
            cycles.set(cached);
        }
        if let Some(cached) = cache::read::<CycleSettings>("cycle_settings") {
            settings.set(cached);
        }
        reload();
        spawn(async move {
            if let Ok(s) = cycles_api::get_cycle_settings().await {
                cache::write("cycle_settings", &s);
                settings.set(s);
            }
        });
        // Load today's mood entry if it exists
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        spawn(async move {
            if let Ok(Some(entry)) = mood_api::get_mood_for_date(today_str).await {
                mood_val.set(Some(entry.mood));
                energy_val.set(Some(entry.energy));
                libido_val.set(Some(entry.libido));
                if let Some(n) = entry.notes {
                    mood_notes.set(n);
                }
                mood_saved.set(true);
            }
        });
        // Load all mood logs for calendar
        spawn(async move {
            if let Ok(logs) = mood_api::list_mood_logs().await {
                mood_logs.set(logs);
            }
        });
    });

    use_effect(move || {
        let _trigger = sync_trigger.read().0;
        reload();
    });

    let today = Local::now().date_naive();
    let s = settings.read().clone();
    let cycle_count = cycles.read().len();

    // Compute phase info from most recent cycle
    let phase_info: Option<PhaseInfo> = cycles.read().first().and_then(|c| {
        if s.on_birth_control {
            birth_control_phase(c.start_date, today, &s)
        } else {
            current_phase(c.start_date, today, &s)
        }
    });

    let prediction = {
        let c = cycles.read();
        Cycle::predict_next_start(&c, &s)
    };

    let countdown = prediction.map(|pred| (pred - today).num_days());

    // Irregular cycle warning
    let variance = Cycle::cycle_variance(&cycles.read());
    let is_irregular = variance.map_or(false, |v| v > 3.0);

    // Is cycle overdue?
    let is_overdue = cycles.read().first().map_or(false, |c| {
        let cycle_day = (today - c.start_date).num_days() + 1;
        cycle_day > s.average_cycle_length
    });

    rsx! {
        div { class: "p-4 space-y-4",
            ErrorBanner { message: error_msg }

            // Phase Dashboard — compact strip by default, expandable
            if let Some(ref info) = phase_info {
                { render_phase_dashboard(info, &s, countdown, cycle_count, dashboard_expanded) }
            } else if is_overdue {
                div { class: "bg-cyber-card border border-neon-pink/40 rounded-xl px-4 py-3 flex items-center gap-3",
                    span { class: "text-xl", "\u{1F4A1}" }
                    div { class: "flex-1",
                        p { class: "text-xs font-bold text-neon-pink", "PERIOD MAY BE LATE" }
                        p { class: "text-[10px] text-cyber-dim",
                            "Expected cycle length passed. Log a new cycle when it starts."
                        }
                    }
                }
            }

            // Daily Mood Check-in
            if phase_info.is_some() || !cycles.read().is_empty() {
                div { class: "bg-cyber-card/80 border border-neon-cyan/20 rounded-xl p-4 space-y-3",

                    // STATE 1: Compact summary (logged & not editing)
                    if *mood_saved.read() && !*checkin_editing.read() {
                        button {
                            class: "flex items-center gap-3 w-full rounded-lg hover:bg-cyber-dark/30 transition-colors",
                            onclick: move |_| checkin_editing.set(true),
                            span { class: "text-[10px] text-neon-cyan tracking-widest uppercase font-bold", "CHECK-IN" }
                            // Mood emoji
                            if let Some(m) = *mood_val.read() {
                                span { class: "text-lg", "{MOOD_EMOJIS[(m as usize).saturating_sub(1).min(4)]}" }
                            }
                            if let Some(e) = *energy_val.read() {
                                span { class: "text-xs text-neon-green font-mono font-bold", "\u{26A1}{e}" }
                            }
                            if let Some(l) = *libido_val.read() {
                                span { class: "text-xs text-neon-orange font-mono font-bold", "\u{1F525}{l}" }
                            }
                            if !mood_notes.read().is_empty() {
                                {
                                    let preview: String = mood_notes.read().chars().take(15).collect();
                                    let ellipsis = if mood_notes.read().len() > 15 { "\u{2026}" } else { "" };
                                    rsx! {
                                        span { class: "text-[10px] text-cyber-dim/60 italic truncate flex-1 text-left",
                                            "\"{preview}{ellipsis}\""
                                        }
                                    }
                                }
                            }
                            span { class: "text-[10px] text-neon-green ml-auto", "\u{2713}" }
                        }
                    }

                    // STATE 2: Compact CTA (not yet logged, form closed)
                    if !*mood_saved.read() && !*checkin_open.read() {
                        button {
                            class: "flex items-center gap-3 w-full rounded-lg hover:bg-cyber-dark/30 transition-colors",
                            onclick: move |_| checkin_open.set(true),
                            span { class: "text-[10px] text-neon-cyan tracking-widest uppercase font-bold", "CHECK-IN" }
                            span { class: "text-[10px] text-neon-cyan/50 ml-auto", "TAP TO LOG" }
                        }
                    }

                    // STATE 3: Editing form (form open, or editing existing)
                    if *checkin_open.read() || *checkin_editing.read() {
                        // Mood row — emoji chips
                        div { class: "space-y-1",
                            div { class: "flex items-center gap-2",
                                span { class: "text-xs text-cyber-dim w-12", "\u{1F9E0} Mood" }
                                div { class: "flex gap-1.5 flex-1",
                                    for i in 1..=5i32 {
                                        {
                                            let is_selected = mood_val.read().map_or(false, |v| v == i);
                                            let emoji = MOOD_EMOJIS[(i - 1) as usize];
                                            let cls = if is_selected {
                                                "w-10 h-10 rounded-lg bg-neon-cyan/25 border border-neon-cyan/60 text-lg flex items-center justify-center transition-all scale-110"
                                            } else {
                                                "w-10 h-10 rounded-lg bg-cyber-dark border border-cyber-border text-lg flex items-center justify-center transition-all opacity-50 hover:opacity-80"
                                            };
                                            rsx! {
                                                button {
                                                    class: "{cls}",
                                                    onclick: move |_| { mood_val.set(Some(i)); mood_saved.set(false); },
                                                    "{emoji}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Energy row
                        div { class: "space-y-1",
                            div { class: "flex items-center gap-2",
                                span { class: "text-xs text-cyber-dim w-12", "\u{26A1} Energy" }
                                div { class: "flex gap-1.5 flex-1",
                                    for i in 1..=5i32 {
                                        {
                                            let is_selected = energy_val.read().map_or(false, |v| v == i);
                                            let bg_sel = match i {
                                                1 => "bg-neon-green/15 border-neon-green/40 text-neon-green/60",
                                                2 => "bg-neon-green/20 border-neon-green/50 text-neon-green/70",
                                                3 => "bg-neon-green/25 border-neon-green/60 text-neon-green/80",
                                                4 => "bg-neon-green/30 border-neon-green/70 text-neon-green/90",
                                                _ => "bg-neon-green/40 border-neon-green/80 text-neon-green",
                                            };
                                            let cls = if is_selected {
                                                format!("w-10 h-10 rounded-lg border text-sm font-bold flex items-center justify-center transition-all scale-110 {bg_sel}")
                                            } else {
                                                "w-10 h-10 rounded-lg bg-cyber-dark border border-cyber-border text-sm text-cyber-dim flex items-center justify-center transition-all opacity-50 hover:opacity-80".to_string()
                                            };
                                            rsx! {
                                                button {
                                                    class: "{cls}",
                                                    onclick: move |_| { energy_val.set(Some(i)); mood_saved.set(false); },
                                                    "{i}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Drive row
                        div { class: "space-y-1",
                            div { class: "flex items-center gap-2",
                                span { class: "text-xs text-cyber-dim w-12", "\u{1F525} Drive" }
                                div { class: "flex gap-1.5 flex-1",
                                    for i in 1..=5i32 {
                                        {
                                            let is_selected = libido_val.read().map_or(false, |v| v == i);
                                            let bg_sel = match i {
                                                1 => "bg-neon-orange/15 border-neon-orange/40 text-neon-orange/60",
                                                2 => "bg-neon-orange/20 border-neon-orange/50 text-neon-orange/70",
                                                3 => "bg-neon-orange/25 border-neon-orange/60 text-neon-orange/80",
                                                4 => "bg-neon-orange/30 border-neon-orange/70 text-neon-orange/90",
                                                _ => "bg-neon-orange/40 border-neon-orange/80 text-neon-orange",
                                            };
                                            let cls = if is_selected {
                                                format!("w-10 h-10 rounded-lg border text-sm font-bold flex items-center justify-center transition-all scale-110 {bg_sel}")
                                            } else {
                                                "w-10 h-10 rounded-lg bg-cyber-dark border border-cyber-border text-sm text-cyber-dim flex items-center justify-center transition-all opacity-50 hover:opacity-80".to_string()
                                            };
                                            rsx! {
                                                button {
                                                    class: "{cls}",
                                                    onclick: move |_| { libido_val.set(Some(i)); mood_saved.set(false); },
                                                    "{i}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Notes
                        input {
                            class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-3 py-2 text-xs text-cyber-text placeholder:text-cyber-dim/30",
                            placeholder: "Notes (optional)",
                            value: "{mood_notes}",
                            oninput: move |e| { mood_notes.set(e.value()); mood_saved.set(false); },
                        }

                        // Action buttons
                        {
                            let can_save = mood_val.read().is_some() && energy_val.read().is_some() && libido_val.read().is_some() && !*mood_saved.read();
                            let is_editing = *checkin_editing.read();
                            rsx! {
                                div { class: "flex gap-2",
                                    // Cancel button
                                    button {
                                        class: "flex-1 bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2.5 text-xs text-cyber-dim tracking-wider",
                                        onclick: move |_| { checkin_editing.set(false); checkin_open.set(false); },
                                        "CANCEL"
                                    }
                                    // Save/Update button
                                    {
                                        let btn_cls = if can_save {
                                            "flex-1 bg-neon-cyan/20 text-neon-cyan border border-neon-cyan/40 rounded-lg px-4 py-2.5 text-xs font-bold tracking-wider hover:bg-neon-cyan/30 transition-colors"
                                        } else {
                                            "flex-1 bg-neon-cyan/20 text-neon-cyan border border-neon-cyan/40 rounded-lg px-4 py-2.5 text-xs font-bold tracking-wider transition-colors opacity-30 cursor-not-allowed"
                                        };
                                        let label = if is_editing { "UPDATE" } else { "SAVE CHECK-IN" };
                                        rsx! {
                                            button {
                                                class: "{btn_cls}",
                                                disabled: !can_save,
                                                onclick: move |_| {
                                                    if let (Some(m), Some(e), Some(l)) = (*mood_val.read(), *energy_val.read(), *libido_val.read()) {
                                                        let n = {
                                                            let notes = mood_notes.read().clone();
                                                            if notes.is_empty() { None } else { Some(notes) }
                                                        };
                                                        let date_str = Local::now().format("%Y-%m-%d").to_string();
                                                        spawn(async move {
                                                            match mood_api::log_mood(date_str, m, e, l, n).await {
                                                                Ok(()) => {
                                                                    mood_saved.set(true);
                                                                    checkin_editing.set(false);
                                                                    checkin_open.set(false);
                                                                }
                                                                Err(err) => error_msg.set(Some(format!("Failed to log mood: {err}"))),
                                                            }
                                                        });
                                                    }
                                                },
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Irregular cycle warning (compact inline)
            if is_irregular {
                div { class: "bg-neon-orange/5 border border-neon-orange/30 rounded-lg px-3 py-2 flex items-center gap-2",
                    span { class: "text-xs", "\u{26A0}\u{FE0F}" }
                    p { class: "text-[10px] text-neon-orange/80",
                        "Cycles vary >3 days \u{2014} predictions less accurate"
                    }
                }
            }

            // PMS care — compact banner, expandable
            {
                let show_pms_phase = phase_info.as_ref().map_or(false, |i| i.phase == CyclePhase::LateLuteal && !s.on_birth_control);
                let show_pms_countdown = !show_pms_phase && countdown.map_or(false, |d| d >= 1 && d <= 5);
                let pms_days = countdown.unwrap_or(0);

                if show_pms_phase || show_pms_countdown {
                    rsx! {
                        div { class: "bg-gradient-to-br from-[#1a1028] to-[#0f1528] border border-neon-purple/30 rounded-xl overflow-hidden",
                            // Compact banner — always visible
                            button {
                                class: "flex items-center gap-3 w-full px-4 py-3 hover:bg-neon-purple/5 transition-colors",
                                onclick: move |_| pms_expanded.set(!pms_expanded()),
                                span { class: "text-lg", "\u{1F319}" }
                                p { class: "text-xs font-bold text-neon-purple flex-1 text-left",
                                    if show_pms_phase {
                                        "Sensitive phase \u{2014} be extra gentle"
                                    } else {
                                        "Period in ~{pms_days} days"
                                    }
                                }
                                span { class: "text-[10px] text-neon-purple/50",
                                    if *pms_expanded.read() { "\u{25B2}" } else { "\u{25BC}" }
                                }
                            }
                            // Expanded details
                            if *pms_expanded.read() {
                                div { class: "px-4 pb-4 space-y-2",
                                    p { class: "text-xs text-cyber-dim leading-relaxed",
                                        if show_pms_phase {
                                            "Hormones are dropping \u{2014} irritability and fog are normal. Be kind to yourself."
                                        } else {
                                            "A good time to stock up on snacks and schedule some self-care."
                                        }
                                    }
                                    div { class: "flex flex-wrap gap-1.5",
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
                } else {
                    rsx! {}
                }
            }

            // Action buttons row
            div { class: "flex gap-2",
                button {
                    class: "flex-1 bg-neon-pink/20 text-neon-pink border border-neon-pink/40 rounded-xl px-4 py-3 text-xs font-bold tracking-wider uppercase hover:bg-neon-pink/30 transition-colors glow-pink",
                    onclick: move |_| show_form.set(!show_form()),
                    if *show_form.read() { "CANCEL" } else { "LOG CYCLE" }
                }
                {
                    let panels: Vec<(&str, &str, &str)> = vec![
                        ("calendar", "\u{1F4C5}", "HISTORY"),
                        ("insights", "\u{1F4CA}", "INSIGHTS"),
                        ("settings", "\u{2699}\u{FE0F}", "SETTINGS"),
                    ];
                    rsx! {
                        for (id, icon, label) in panels.iter() {
                            {
                                let id = *id;
                                let icon = *icon;
                                let label = *label;
                                let is_active = active_panel.read().map_or(false, |p| p == id);
                                let cls = if is_active {
                                    "bg-neon-cyan/20 border border-neon-cyan/40 rounded-xl px-3 py-2 flex flex-col items-center gap-0.5 transition-colors"
                                } else {
                                    "bg-cyber-card border border-cyber-border rounded-xl px-3 py-2 flex flex-col items-center gap-0.5 hover:border-neon-cyan/30 transition-colors"
                                };
                                let text_cls = if is_active { "text-[7px] tracking-wider text-neon-cyan" } else { "text-[7px] tracking-wider text-cyber-dim" };
                                let icon_cls = if is_active { "text-sm text-neon-cyan" } else { "text-sm text-cyber-dim" };
                                rsx! {
                                    button {
                                        class: "{cls}",
                                        onclick: move |_| {
                                            let current = *active_panel.read();
                                            if current == Some(id) {
                                                active_panel.set(None);
                                            } else {
                                                active_panel.set(Some(id));
                                                if id == "insights" {
                                                    spawn(async move {
                                                        match mood_api::get_mood_insights().await {
                                                            Ok(data) => insights.set(data),
                                                            Err(e) => error_msg.set(Some(format!("Failed to load insights: {e}"))),
                                                        }
                                                    });
                                                }
                                            }
                                        },
                                        span { class: "{icon_cls}", "{icon}" }
                                        span { class: "{text_cls}", "{label}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Log cycle form
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
                    label { class: "text-[10px] text-cyber-dim tracking-widest uppercase block", "SYMPTOMS" }
                    div { class: "flex flex-wrap gap-2",
                        for symptom in SYMPTOM_CHIPS {
                            { render_symptom_chip(symptom, selected_symptoms) }
                        }
                    }
                    button {
                        class: "w-full bg-neon-pink/20 text-neon-pink border border-neon-pink/40 rounded-lg px-4 py-3 text-xs font-bold tracking-wider hover:bg-neon-pink/30 transition-colors glow-pink",
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
                        "SAVE"
                    }
                }
            }

            // Panels (mutually exclusive)
            if active_panel.read().map_or(false, |p| p == "calendar") {
                { render_mood_calendar(
                    calendar_month,
                    &mood_logs.read(),
                    &cycles.read(),
                    &s,
                    selected_day,
                ) }
            }

            if active_panel.read().map_or(false, |p| p == "insights") {
                { render_insights_panel(&insights.read()) }
            }

            if active_panel.read().map_or(false, |p| p == "settings") {
                { render_settings_panel(settings, active_panel, error_msg) }
            }

            // History — truncated to 3, expandable
            {
                let all_cycles = cycles.read();
                let total = all_cycles.len();
                let show_all = *show_all_history.read();
                let visible: Vec<Cycle> = if show_all {
                    all_cycles.iter().cloned().collect()
                } else {
                    all_cycles.iter().take(3).cloned().collect()
                };
                let hidden = total.saturating_sub(3);

                rsx! {
                    div { class: "space-y-0",
                        for cycle in visible.iter() {
                            { render_cycle(cycle.clone(), reload, error_msg) }
                        }
                        if total == 0 {
                            div { class: "text-center py-12",
                                p { class: "text-2xl mb-2 opacity-30", "\u{1F319}" }
                                p { class: "text-[10px] tracking-[0.3em] uppercase text-cyber-dim", "No cycles logged yet" }
                            }
                        }
                        if !show_all && hidden > 0 {
                            button {
                                class: "w-full py-2 text-[10px] text-neon-cyan/60 tracking-wider hover:text-neon-cyan transition-colors",
                                onclick: move |_| show_all_history.set(true),
                                "SHOW ALL ({total})"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_phase_dashboard(
    info: &PhaseInfo,
    settings: &CycleSettings,
    countdown: Option<i64>,
    cycle_count: usize,
    mut expanded: Signal<bool>,
) -> Element {
    let color = info.phase.color_class();
    let label = info.phase.label();
    let icon = info.phase.icon();
    let day = info.cycle_day;
    let acl = settings.average_cycle_length;
    let progress_pct = ((day as f64 / acl as f64) * 100.0).min(100.0);

    // Countdown text for the strip
    let countdown_text = match countdown {
        Some(0) => "today".to_string(),
        Some(1) => "tomorrow".to_string(),
        Some(d) if d > 0 => format!("in {d}d"),
        Some(d) => format!("{}d ago", d.abs()),
        None => String::new(),
    };

    rsx! {
        div { class: "bg-cyber-card border border-{color}/30 rounded-xl overflow-hidden",
            // Compact strip — always visible
            button {
                class: "flex items-center gap-3 w-full px-4 py-3 hover:bg-cyber-dark/30 transition-colors",
                onclick: move |_| expanded.set(!expanded()),
                span { class: "text-xl", "{icon}" }
                div { class: "flex-1 text-left",
                    div { class: "flex items-center gap-2",
                        span { class: "text-xs font-bold text-{color} tracking-wider uppercase", "{label}" }
                        span { class: "text-[10px] text-cyber-dim font-mono", "Day {day}/{acl}" }
                    }
                    if !countdown_text.is_empty() {
                        p { class: "text-[10px] text-neon-pink/80 font-mono",
                            "\u{1F4C5} next period {countdown_text}"
                        }
                    }
                }
                span { class: "text-[10px] text-{color}/50",
                    if *expanded.read() { "\u{25B2}" } else { "\u{25BC}" }
                }
            }

            // Thin progress bar — always visible
            div { class: "w-full bg-cyber-dark h-1 relative",
                div {
                    class: "h-full bg-{color}/60 transition-all duration-500",
                    style: "width: {progress_pct}%",
                }
            }

            // Expanded details
            if *expanded.read() {
                div { class: "px-4 py-4 space-y-4",
                    // Description
                    p { class: "text-xs text-cyber-dim text-center leading-relaxed",
                        "{info.description}"
                    }

                    // Mood / Energy / Libido cards
                    div { class: "grid grid-cols-3 gap-2",
                        { render_stat_card("\u{1F9E0}", "Mood", info.mood, color) }
                        { render_stat_card("\u{26A1}", "Energy", info.energy, color) }
                        { render_stat_card("\u{1F525}", "Drive", info.libido, color) }
                    }

                    // Days remaining + data confidence
                    if info.days_in_phase_remaining > 0 {
                        {
                            let suffix = if info.days_in_phase_remaining != 1 { "S" } else { "" };
                            let remaining = info.days_in_phase_remaining;
                            rsx! {
                                p { class: "text-[10px] text-cyber-dim/50 text-center tracking-wider",
                                    "{remaining} DAY{suffix} LEFT IN THIS PHASE"
                                }
                            }
                        }
                    }
                    if cycle_count < 3 {
                        {
                            let suffix = if cycle_count != 1 { "S" } else { "" };
                            rsx! {
                                p { class: "text-[10px] text-cyber-dim/40 text-center tracking-wider",
                                    "BASED ON {cycle_count} CYCLE{suffix} \u{2022} MORE DATA = BETTER ACCURACY"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_stat_card(icon: &str, label: &str, value: &str, color: &str) -> Element {
    // Map text values to a 1-5 pip level
    let level: usize = match value {
        "Low" | "Introspective" | "Sensitive" => 1,
        "Warming Up" | "Rising" | "Stable" | "Steady" => 2,
        "Baseline" | "Moderate" | "Calm & Nesting" | "Upbeat & Sociable" => 3,
        "High" | "Confident & Magnetic" => 4,
        "Peak" => 5,
        _ => 3,
    };

    rsx! {
        div { class: "bg-cyber-dark/50 border border-{color}/15 rounded-lg p-3 text-center space-y-1",
            p { class: "text-sm", "{icon}" }
            p { class: "text-[9px] text-cyber-dim tracking-widest uppercase", "{label}" }
            // Pip indicators
            div { class: "flex justify-center gap-1",
                for i in 1..=5usize {
                    {
                        let pip_cls = if i <= level {
                            format!("w-1.5 h-1.5 rounded-full bg-{color}/80")
                        } else {
                            format!("w-1.5 h-1.5 rounded-full bg-{color}/15")
                        };
                        rsx! { div { class: "{pip_cls}" } }
                    }
                }
            }
            p { class: "text-[10px] font-bold text-{color}", "{value}" }
        }
    }
}

fn render_insights_panel(insights: &[PhaseInsight]) -> Element {
    if insights.is_empty() {
        return rsx! {
            div { class: "bg-cyber-card/80 border border-neon-cyan/20 rounded-xl p-5 text-center space-y-2",
                p { class: "text-xs text-neon-cyan tracking-widest uppercase font-bold", "INSIGHTS" }
                p { class: "text-2xl opacity-30", "\u{1F4CA}" }
                p { class: "text-xs text-cyber-dim",
                    "Log your mood daily to build personalized insights."
                }
                p { class: "text-[10px] text-cyber-dim/50",
                    "The algorithm compares how you actually feel vs. the biological baseline for each phase."
                }
            }
        };
    }

    let total_entries: usize = insights.iter().map(|i| i.sample_count).sum();

    rsx! {
        div { class: "bg-cyber-card/80 border border-neon-cyan/20 rounded-xl p-4 space-y-4",
            div { class: "flex items-center justify-between",
                p { class: "text-xs text-neon-cyan tracking-widest uppercase font-bold", "INSIGHTS" }
                p { class: "text-[10px] text-cyber-dim",
                    "{total_entries} mood entries analyzed"
                }
            }

            if total_entries < 10 {
                p { class: "text-[10px] text-neon-orange/70 bg-neon-orange/5 border border-neon-orange/20 rounded-lg px-3 py-2",
                    "Keep logging daily! More data = better personal insights."
                }
            }

            // Per-phase cards
            for insight in insights.iter() {
                { render_insight_card(insight) }
            }
        }
    }
}

fn render_insight_card(insight: &PhaseInsight) -> Element {
    let color = insight.phase.color_class();
    let icon = insight.phase.icon();
    let label = insight.phase.label();
    let n = insight.sample_count;

    // Deviation bar helper: clamp to -2..+2, map to percentage
    let mood_bar = ((insight.mood_deviation / 2.0) * 50.0).clamp(-50.0, 50.0);
    let energy_bar = ((insight.energy_deviation / 2.0) * 50.0).clamp(-50.0, 50.0);
    let libido_bar = ((insight.libido_deviation / 2.0) * 50.0).clamp(-50.0, 50.0);

    rsx! {
        div { class: "bg-cyber-dark/50 border border-{color}/20 rounded-lg p-3 space-y-2",
            // Header
            div { class: "flex items-center gap-2",
                span { class: "text-sm", "{icon}" }
                span { class: "text-[11px] font-bold text-{color}", "{label}" }
                span { class: "text-[9px] text-cyber-dim ml-auto", "{n} entries" }
            }

            // Deviation bars
            div { class: "space-y-1.5",
                { render_deviation_row("\u{1F9E0}", "Mood", insight.avg_mood, insight.baseline_mood, mood_bar) }
                { render_deviation_row("\u{26A1}", "Energy", insight.avg_energy, insight.baseline_energy, energy_bar) }
                { render_deviation_row("\u{1F525}", "Drive", insight.avg_libido, insight.baseline_libido, libido_bar) }
            }

            // Insight text
            p { class: "text-[10px] text-cyber-dim leading-relaxed",
                "{insight.insight_text}"
            }
        }
    }
}

fn render_deviation_row(icon: &str, _label: &str, actual: f64, baseline: f64, bar_pct: f64) -> Element {
    let actual_fmt = format!("{:.1}", actual);
    let baseline_fmt = format!("{:.1}", baseline);

    // Bar direction and color
    let (bar_style, bar_color) = if bar_pct >= 0.0 {
        (format!("left: 50%; width: {}%;", bar_pct.abs()), "bg-neon-green/50")
    } else {
        (format!("left: {}%; width: {}%;", 50.0 + bar_pct, bar_pct.abs()), "bg-neon-pink/50")
    };

    rsx! {
        div { class: "flex items-center gap-2",
            span { class: "text-[10px] w-4", "{icon}" }
            // Bar container
            div { class: "flex-1 h-2 bg-cyber-dark rounded-full relative overflow-hidden",
                // Center line
                div { class: "absolute top-0 bottom-0 left-1/2 w-px bg-cyber-border z-10" }
                // Deviation bar
                div {
                    class: "absolute top-0 bottom-0 rounded-full {bar_color}",
                    style: "{bar_style}",
                }
            }
            // Actual vs baseline
            span { class: "text-[9px] text-cyber-dim font-mono w-16 text-right",
                "{actual_fmt} / {baseline_fmt}"
            }
        }
    }
}

fn render_mood_calendar(
    mut month_signal: Signal<NaiveDate>,
    mood_logs: &[MoodEntry],
    cycles: &[Cycle],
    settings: &CycleSettings,
    mut selected_day: Signal<Option<NaiveDate>>,
) -> Element {
    let month_start = *month_signal.read();
    let year = month_start.year();
    let month = month_start.month();
    let today = Local::now().date_naive();

    // Month name
    let month_name = match month {
        1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
        5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
        9 => "SEP", 10 => "OCT", 11 => "NOV", _ => "DEC",
    };

    // Build mood lookup: date -> MoodEntry
    let mood_map: HashMap<NaiveDate, &MoodEntry> = mood_logs.iter().map(|e| (e.date, e)).collect();

    // Days in this month
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }.unwrap();
    let days_in_month = (next_month - month_start).num_days() as u32;

    // Day of week for the 1st (Mon=0 ... Sun=6)
    let first_weekday = month_start.weekday().num_days_from_monday() as usize;

    // Previous/next month dates
    let prev_month = if month == 1 {
        NaiveDate::from_ymd_opt(year - 1, 12, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month - 1, 1)
    }.unwrap();

    // Build day cells: Option<(day_number, date)>
    let total_cells = first_weekday + days_in_month as usize;
    let rows = (total_cells + 6) / 7; // ceil division
    let cells: Vec<Option<NaiveDate>> = (0..rows * 7)
        .map(|i| {
            if i < first_weekday {
                None
            } else {
                let day = (i - first_weekday + 1) as u32;
                if day <= days_in_month {
                    NaiveDate::from_ymd_opt(year, month, day)
                } else {
                    None
                }
            }
        })
        .collect();

    // Selected day detail
    let detail = selected_day.read().and_then(|d| mood_map.get(&d).copied());
    let detail_phase = selected_day.read().and_then(|d| phase_for_date(d, cycles, settings));

    rsx! {
        div { class: "bg-cyber-card/80 border border-neon-cyan/20 rounded-xl p-4 space-y-3",
            // Header with month navigation
            div { class: "flex items-center justify-between",
                button {
                    class: "text-cyber-dim hover:text-neon-cyan text-sm w-11 h-11 flex items-center justify-center transition-colors rounded-lg hover:bg-cyber-dark/50",
                    onclick: move |_| month_signal.set(prev_month),
                    "\u{25C0}"
                }
                p { class: "text-xs text-neon-cyan tracking-[0.3em] uppercase font-bold",
                    "{month_name} {year}"
                }
                button {
                    class: "text-cyber-dim hover:text-neon-cyan text-sm w-11 h-11 flex items-center justify-center transition-colors rounded-lg hover:bg-cyber-dark/50",
                    onclick: move |_| month_signal.set(next_month),
                    "\u{25B6}"
                }
            }

            // Weekday headers
            div { class: "grid grid-cols-7 gap-1",
                for day_name in &["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                    div { class: "text-center text-[8px] text-cyber-dim/50 tracking-wider font-bold py-1",
                        "{day_name}"
                    }
                }
            }

            // Calendar grid
            div { class: "grid grid-cols-7 gap-1",
                for cell in cells.iter() {
                    {
                        match cell {
                            None => rsx! { div { class: "h-10" } },
                            Some(date) => {
                                let d = *date;
                                let day_num = d.day();
                                let is_today = d == today;
                                let is_selected = selected_day.read().map_or(false, |s| s == d);
                                let entry = mood_map.get(&d);
                                let phase = phase_for_date(d, cycles, settings);

                                // Phase background tint
                                let phase_bg = if d <= today {
                                    match &phase {
                                        Some(CyclePhase::Menstruation) => "bg-neon-pink/8",
                                        Some(CyclePhase::Follicular) => "bg-neon-green/8",
                                        Some(CyclePhase::Ovulation) => "bg-neon-orange/8",
                                        Some(CyclePhase::EarlyLuteal) => "bg-neon-purple/8",
                                        Some(CyclePhase::LateLuteal) => "bg-neon-magenta/8",
                                        None => "",
                                    }
                                } else {
                                    ""
                                };

                                let border = if is_selected {
                                    "border border-neon-cyan/60"
                                } else if is_today {
                                    "border border-neon-cyan/30"
                                } else {
                                    "border border-transparent"
                                };

                                let future_dim = if d > today { "opacity-30" } else { "" };

                                rsx! {
                                    button {
                                        class: "h-10 rounded-lg {phase_bg} {border} {future_dim} flex flex-col items-center justify-center transition-all hover:bg-cyber-dark/50",
                                        onclick: move |_| {
                                            if d <= today {
                                                let current = *selected_day.read();
                                                if current == Some(d) {
                                                    selected_day.set(None);
                                                } else {
                                                    selected_day.set(Some(d));
                                                }
                                            }
                                        },
                                        // Day number
                                        span { class: "text-[9px] text-cyber-dim font-mono leading-none",
                                            "{day_num}"
                                        }
                                        // Mood emoji if logged
                                        if let Some(e) = entry {
                                            span { class: "text-[11px] leading-none",
                                                "{MOOD_EMOJIS[(e.mood as usize).saturating_sub(1).min(4)]}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Legend
            div { class: "flex flex-wrap gap-2 justify-center pt-1",
                for (label, color) in [
                    ("Period", "neon-pink"),
                    ("Follicular", "neon-green"),
                    ("Ovulation", "neon-orange"),
                    ("Luteal", "neon-purple"),
                ] {
                    div { class: "flex items-center gap-1",
                        div { class: "w-2 h-2 rounded-sm bg-{color}/40" }
                        span { class: "text-[8px] text-cyber-dim/60", "{label}" }
                    }
                }
            }

            // Selected day detail card
            if let Some(d) = *selected_day.read() {
                div { class: "bg-cyber-dark/50 border border-cyber-border rounded-lg p-3 space-y-2",
                    div { class: "flex items-center justify-between",
                        p { class: "text-[11px] font-bold text-cyber-text font-mono", "{d}" }
                        if let Some(ref phase) = detail_phase {
                            {
                                let pc = phase.color_class();
                                let pl = phase.label();
                                let cls = format!("text-[9px] text-{pc}/80 bg-{pc}/10 border border-{pc}/20 px-2 py-0.5 rounded");
                                rsx! {
                                    span { class: "{cls}", "{pl}" }
                                }
                            }
                        }
                    }
                    if let Some(entry) = detail {
                        div { class: "grid grid-cols-3 gap-2",
                            div { class: "text-center",
                                p { class: "text-[9px] text-cyber-dim", "Mood" }
                                p { class: "text-sm", "{MOOD_EMOJIS[(entry.mood as usize).saturating_sub(1).min(4)]}" }
                            }
                            div { class: "text-center",
                                p { class: "text-[9px] text-cyber-dim", "Energy" }
                                p { class: "text-sm font-bold text-neon-green", "{entry.energy}" }
                            }
                            div { class: "text-center",
                                p { class: "text-[9px] text-cyber-dim", "Drive" }
                                p { class: "text-sm font-bold text-neon-orange", "{entry.libido}" }
                            }
                        }
                        if let Some(ref notes) = entry.notes {
                            p { class: "text-[10px] text-cyber-dim/70 italic", "\"{notes}\"" }
                        }
                    } else {
                        p { class: "text-[10px] text-cyber-dim/40 text-center", "No mood logged this day" }
                    }
                }
            }
        }
    }
}

fn render_settings_panel(
    mut settings: Signal<CycleSettings>,
    mut active_panel: Signal<Option<&'static str>>,
    mut error_msg: Signal<Option<String>>,
) -> Element {
    let s = settings.read().clone();

    rsx! {
        div { class: "bg-cyber-card/80 border border-cyber-border rounded-xl p-4 space-y-4",
            p { class: "text-xs text-neon-cyan tracking-widest uppercase font-bold", "\u{2699}\u{FE0F} CYCLE SETTINGS" }

            // Average Cycle Length
            div { class: "space-y-1",
                label { class: "text-[10px] text-cyber-dim tracking-widest uppercase block",
                    "CYCLE LENGTH (DAYS)"
                }
                input {
                    class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text font-mono",
                    r#type: "number",
                    min: "21",
                    max: "45",
                    value: "{s.average_cycle_length}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<i64>() {
                            let mut current = settings.read().clone();
                            current.average_cycle_length = v.clamp(21, 45);
                            settings.set(current);
                        }
                    },
                }
                p { class: "text-[9px] text-cyber-dim/50", "Valid range: 21\u{2013}45 days" }
            }

            // Average Period Duration
            div { class: "space-y-1",
                label { class: "text-[10px] text-cyber-dim tracking-widest uppercase block",
                    "PERIOD DURATION (DAYS)"
                }
                input {
                    class: "w-full bg-cyber-dark border border-cyber-border rounded-lg px-4 py-2 text-sm text-cyber-text font-mono",
                    r#type: "number",
                    min: "2",
                    max: "10",
                    value: "{s.average_period_duration}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<i64>() {
                            let mut current = settings.read().clone();
                            current.average_period_duration = v.clamp(2, 10);
                            settings.set(current);
                        }
                    },
                }
                p { class: "text-[9px] text-cyber-dim/50", "Valid range: 2\u{2013}10 days" }
            }

            // Birth Control Toggle
            div { class: "flex items-center justify-between",
                label { class: "text-[10px] text-cyber-dim tracking-widest uppercase",
                    "HORMONAL BIRTH CONTROL"
                }
                button {
                    class: if s.on_birth_control {
                        "w-12 h-6 rounded-full bg-neon-cyan/40 border border-neon-cyan/60 relative transition-colors"
                    } else {
                        "w-12 h-6 rounded-full bg-cyber-dark border border-cyber-border relative transition-colors"
                    },
                    onclick: move |_| {
                        let mut current = settings.read().clone();
                        current.on_birth_control = !current.on_birth_control;
                        settings.set(current);
                    },
                    div {
                        class: if s.on_birth_control {
                            "w-5 h-5 rounded-full bg-neon-cyan absolute top-0.5 right-0.5 transition-all"
                        } else {
                            "w-5 h-5 rounded-full bg-cyber-dim absolute top-0.5 left-0.5 transition-all"
                        },
                    }
                }
            }

            // Save button
            button {
                class: "w-full bg-neon-cyan/20 text-neon-cyan border border-neon-cyan/40 rounded-lg px-4 py-3 text-xs font-bold tracking-wider hover:bg-neon-cyan/30 transition-colors",
                onclick: move |_| {
                    let s = settings.read().clone();
                    spawn(async move {
                        match cycles_api::save_cycle_settings(s.clone()).await {
                            Ok(()) => {
                                cache::write("cycle_settings", &s);
                                active_panel.set(None);
                            }
                            Err(e) => error_msg.set(Some(format!("Failed to save settings: {e}"))),
                        }
                    });
                },
                "SAVE SETTINGS"
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

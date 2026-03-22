use dioxus::prelude::*;

use crate::api::google as google_api;

#[component]
pub fn GoogleSyncPanel() -> Element {
    let mut status = use_signal(|| Option::<bool>::None);
    let mut sync_result = use_signal(|| Option::<String>::None);
    let mut syncing = use_signal(|| false);

    use_effect(move || {
        spawn(async move {
            if let Ok(configured) = google_api::google_calendar_status().await {
                status.set(Some(configured));
            }
        });
    });

    let is_configured = status.read().unwrap_or(false);

    rsx! {
        div { class: "bg-cyber-card/60 border border-cyber-border rounded-xl p-3 mt-4",
            div { class: "flex items-center justify-between",
                div { class: "flex items-center gap-2",
                    span { class: "text-xs tracking-wider uppercase text-cyber-dim font-mono", "Google Calendar" }
                    if is_configured {
                        span { class: "w-2 h-2 rounded-full bg-neon-green inline-block" }
                    } else {
                        span { class: "w-2 h-2 rounded-full bg-cyber-dim/40 inline-block" }
                    }
                }
                if is_configured {
                    button {
                        class: "text-xs px-3 py-1 rounded bg-neon-cyan/10 text-neon-cyan border border-neon-cyan/30 hover:bg-neon-cyan/20 transition-colors font-mono tracking-wider disabled:opacity-50",
                        disabled: *syncing.read(),
                        onclick: move |_| {
                            syncing.set(true);
                            sync_result.set(None);
                            spawn(async move {
                                match google_api::google_full_sync().await {
                                    Ok(msg) => sync_result.set(Some(msg)),
                                    Err(e) => sync_result.set(Some(format!("Error: {e}"))),
                                }
                                syncing.set(false);
                            });
                        },
                        if *syncing.read() { "SYNCING..." } else { "RE-SYNC" }
                    }
                }
            }
            if !is_configured {
                p { class: "text-[10px] text-cyber-dim/60 mt-1 font-mono",
                    "Set GOOGLE_SA_KEY_FILE + GOOGLE_CALENDAR_ID to enable"
                }
            }
            if let Some(ref result) = *sync_result.read() {
                p { class: "text-xs text-neon-green/80 mt-2 font-mono", "{result}" }
            }
        }
    }
}

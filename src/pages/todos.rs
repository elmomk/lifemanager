use dioxus::prelude::*;

use crate::components::checklist_page::ChecklistPage;
use crate::components::google_sync::GoogleSyncPanel;
use crate::models::ItemCategory;

const CHIPS: &[&str] = &["Laundry", "Clean", "Pay Bills", "Exercise", "Cook", "Study"];

#[component]
pub fn Todos() -> Element {
    rsx! {
        div {
            ChecklistPage {
                category: ItemCategory::Todo,
                placeholder: "Add a to-do...",
                initial_chips: CHIPS.iter().map(|s| s.to_string()).collect(),
                empty_text: "No to-dos yet",
                done_label: "DONE",
                accent_color: "cyan",
            }
            div { class: "px-4",
                GoogleSyncPanel {}
            }
        }
    }
}

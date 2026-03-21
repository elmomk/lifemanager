use dioxus::prelude::*;

use crate::components::checklist_page::ChecklistPage;
use crate::models::ItemCategory;

const CHIPS: &[&str] = &["Laundry", "Clean", "Pay Bills", "Exercise", "Cook", "Study"];

#[component]
pub fn Todos() -> Element {
    rsx! {
        ChecklistPage {
            category: ItemCategory::Todo,
            placeholder: "Add a task...",
            initial_chips: CHIPS.iter().map(|s| s.to_string()).collect(),
            empty_text: "No tasks yet",
            done_label: "DONE",
            accent_color: "cyan",
        }
    }
}

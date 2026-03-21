use dioxus::prelude::*;

use crate::components::checklist_page::ChecklistPage;
use crate::models::ItemCategory;

const CHIPS: &[&str] = &["Milk", "Eggs", "Bread", "Rice", "Chicken", "Vegetables", "Fruit", "Water"];

#[component]
pub fn Groceries() -> Element {
    rsx! {
        ChecklistPage {
            category: ItemCategory::Grocery,
            placeholder: "Add grocery item...",
            initial_chips: CHIPS.iter().map(|s| s.to_string()).collect(),
            empty_text: "No grocery items yet",
            done_label: "GOT IT",
            accent_color: "green",
        }
    }
}

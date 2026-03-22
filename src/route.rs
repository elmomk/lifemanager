use dioxus::prelude::*;

use crate::components::layout::AppLayout;
use crate::pages::*;

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
        #[route("/todos")]
        Todos {},
        #[route("/groceries")]
        Groceries {},
        #[route("/shopee")]
        Shopee {},
        #[route("/watchlist")]
        Watchlist {},
        #[route("/watchlist/settings")]
        WatchSettings {},
        #[route("/period")]
        Period {},
    #[end_layout]
    #[redirect("/", || Route::Todos {})]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        div { class: "flex items-center justify-center h-full",
            p { class: "text-lg text-gray-500", "Page not found" }
        }
    }
}

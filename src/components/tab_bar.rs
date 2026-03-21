use dioxus::prelude::*;

use crate::components::icons::*;
use crate::route::Route;

#[component]
pub fn TabBar() -> Element {
    let route: Route = use_route();

    let tabs: Vec<(Route, &str, Element)> = vec![
        (Route::Todos {}, "Todos", rsx! { CheckSquareIcon { class: "w-6 h-6".to_string() } }),
        (Route::Groceries {}, "Groceries", rsx! { ShoppingCartIcon { class: "w-6 h-6".to_string() } }),
        (Route::Shopee {}, "Shopee", rsx! { PackageIcon { class: "w-6 h-6".to_string() } }),
        (Route::Watchlist {}, "Watch", rsx! { TvIcon { class: "w-6 h-6".to_string() } }),
        (Route::Period {}, "Cycle", rsx! { HeartIcon { class: "w-6 h-6".to_string() } }),
    ];

    rsx! {
        nav { class: "fixed bottom-0 left-0 right-0 z-50 bg-white/80 dark:bg-gray-900/80 backdrop-blur-lg border-t border-gray-200/50 dark:border-gray-700/50 safe-bottom",
            div { class: "flex justify-around items-center h-16 max-w-lg mx-auto",
                for (target, label, icon) in tabs {
                    { render_tab(target, label, icon, &route) }
                }
            }
        }
    }
}

fn render_tab(target: Route, label: &str, icon: Element, current: &Route) -> Element {
    let is_active = std::mem::discriminant(&target) == std::mem::discriminant(current);
    let color = if is_active {
        "text-blue-500"
    } else {
        "text-gray-400 dark:text-gray-500"
    };

    rsx! {
        Link {
            to: target,
            class: "flex flex-col items-center gap-0.5 px-3 py-1 {color} transition-colors",
            {icon}
            span { class: "text-xs font-medium", "{label}" }
        }
    }
}

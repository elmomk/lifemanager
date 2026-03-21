use dioxus::prelude::*;

use crate::components::tab_bar::TabBar;
use crate::route::Route;

#[component]
pub fn AppLayout() -> Element {
    let route: Route = use_route();

    let title = match route {
        Route::Todos {} => "TO-DOS",
        Route::Groceries {} => "GROCERIES",
        Route::Shopee {} => "SHOPEE PICK-UPS",
        Route::Watchlist {} => "WATCHLIST",
        Route::Period {} => "CYCLE TRACKER",
        _ => "LIFE MANAGER",
    };

    rsx! {
        div { class: "scanlines min-h-screen bg-cyber-black text-cyber-text font-mono",
            // Header
            header { class: "fixed top-0 left-0 right-0 z-50 bg-cyber-dark/90 backdrop-blur-lg border-b border-neon-cyan/20",
                div { class: "flex items-center justify-between h-14 px-4 max-w-lg mx-auto",
                    h1 { class: "text-sm font-bold tracking-[0.2em] text-neon-cyan text-glow-cyan", "{title}" }
                }
            }

            // Content
            main { class: "pt-14 pb-16 max-w-lg mx-auto",
                Outlet::<Route> {}
            }

            TabBar {}
        }
    }
}

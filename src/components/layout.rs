use dioxus::prelude::*;

use crate::components::tab_bar::TabBar;
use crate::route::Route;

#[component]
pub fn AppLayout() -> Element {
    let route: Route = use_route();

    let title = match route {
        Route::Todos {} => "To-Dos",
        Route::Groceries {} => "Groceries",
        Route::Shopee {} => "Shopee Pick-ups",
        Route::Watchlist {} => "Watchlist",
        Route::Period {} => "Cycle Tracker",
        _ => "Life Manager",
    };

    rsx! {
        div { class: "min-h-screen bg-gray-50 dark:bg-gray-950 text-gray-900 dark:text-gray-100",
            // Header
            header { class: "fixed top-0 left-0 right-0 z-50 bg-white/80 dark:bg-gray-900/80 backdrop-blur-lg border-b border-gray-200/50 dark:border-gray-700/50",
                div { class: "flex items-center justify-between h-14 px-4 max-w-lg mx-auto",
                    h1 { class: "text-lg font-semibold", "{title}" }
                    ThemeToggle {}
                }
            }

            // Content area with padding for fixed header/footer
            main { class: "pt-14 pb-16 max-w-lg mx-auto",
                Outlet::<Route> {}
            }

            TabBar {}
        }
    }
}

#[component]
fn ThemeToggle() -> Element {
    rsx! {
        button {
            class: "p-2 rounded-xl hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors",
            onclick: move |_| {
                document::eval(
                    r#"
                    let html = document.documentElement;
                    if (html.classList.contains('dark')) {
                        html.classList.remove('dark');
                        localStorage.setItem('theme', 'light');
                    } else {
                        html.classList.add('dark');
                        localStorage.setItem('theme', 'dark');
                    }
                    "#,
                );
            },
            svg {
                class: "w-5 h-5",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "4" }
                path { d: "M12 2v2" }
                path { d: "M12 20v2" }
                path { d: "m4.93 4.93 1.41 1.41" }
                path { d: "m17.66 17.66 1.41 1.41" }
                path { d: "M2 12h2" }
                path { d: "M20 12h2" }
                path { d: "m6.34 17.66-1.41 1.41" }
                path { d: "m19.07 4.93-1.41 1.41" }
            }
        }
    }
}

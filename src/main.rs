mod components;
mod models;
mod pages;
mod route;
mod api;
#[cfg(not(target_arch = "wasm32"))]
mod server;

use dioxus::prelude::*;

use route::Route;

static CSS: Asset = asset!("/assets/main.css");

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    server::db::init();

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Apply saved theme on load (client-only)
    use_effect(|| {
        document::eval(
            r#"
            if (localStorage.getItem('theme') === 'dark') {
                document.documentElement.classList.add('dark');
            }
            "#,
        );
    });

    rsx! {
        document::Stylesheet { href: CSS }
        document::Link { rel: "manifest", href: "/lifemanager/manifest.json" }
        document::Link { rel: "apple-touch-icon", href: "/lifemanager/icons/icon-192.png" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Meta { name: "theme-color", content: "#3b82f6" }
        document::Meta { name: "apple-mobile-web-app-capable", content: "yes" }
        document::Meta { name: "apple-mobile-web-app-status-bar-style", content: "black-translucent" }

        Router::<Route> {}
    }
}

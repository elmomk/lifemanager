use dioxus::prelude::*;

use crate::api::shopee::ocr_shopee;
use crate::models::OcrResult;

#[component]
pub fn ShopeeOcr(on_results: EventHandler<Vec<OcrResult>>) -> Element {
    let mut loading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let handle_file = move |_| {
        spawn(async move {
            let js = r#"
                const input = document.createElement('input');
                input.type = 'file';
                input.accept = 'image/*';
                input.onchange = () => {
                    const file = input.files[0];
                    if (!file) { dioxus.send(''); return; }
                    const reader = new FileReader();
                    reader.onload = () => dioxus.send(reader.result);
                    reader.onerror = () => dioxus.send('');
                    reader.readAsDataURL(file);
                };
                input.oncancel = () => dioxus.send('');
                input.click();
            "#;

            let mut eval = document::eval(js);
            let base64_data = match eval.recv::<String>().await {
                Ok(s) => {
                    if s.is_empty() {
                        return;
                    }
                    s
                }
                Err(e) => {
                    error_msg.set(Some(format!("File read error: {e}")));
                    return;
                }
            };

            loading.set(true);
            error_msg.set(None);

            match ocr_shopee(base64_data).await {
                Ok(results) => {
                    loading.set(false);
                    on_results.call(results);
                }
                Err(e) => {
                    loading.set(false);
                    error_msg.set(Some(format!("{e}")));
                }
            }
        });
    };

    rsx! {
        div { class: "flex flex-col items-center gap-1",
            button {
                class: "flex items-center justify-center w-10 h-10 bg-neon-orange/10 text-neon-orange border border-neon-orange/30 rounded-lg hover:bg-neon-orange/20 transition-colors disabled:opacity-50",
                r#type: "button",
                disabled: loading(),
                onclick: handle_file,
                if loading() {
                    svg {
                        class: "w-5 h-5 animate-spin",
                        view_box: "0 0 24 24",
                        fill: "none",
                        circle {
                            cx: "12", cy: "12", r: "10",
                            stroke: "currentColor", stroke_width: "4",
                            class: "opacity-25",
                        }
                        path {
                            d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z",
                            fill: "currentColor",
                            class: "opacity-75",
                        }
                    }
                } else {
                    svg {
                        class: "w-5 h-5",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M23 19a2 2 0 01-2 2H3a2 2 0 01-2-2V8a2 2 0 012-2h4l2-3h6l2 3h4a2 2 0 012 2z" }
                        circle { cx: "12", cy: "13", r: "4" }
                    }
                }
            }
            if let Some(err) = error_msg() {
                p { class: "text-[10px] text-neon-magenta max-w-[120px] text-center font-mono", "{err}" }
            }
        }
    }
}

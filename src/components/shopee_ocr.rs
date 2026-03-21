use dioxus::prelude::*;

use crate::api::shopee::ocr_shopee_code;

#[component]
pub fn ShopeeOcr(on_code_extracted: EventHandler<String>) -> Element {
    let mut loading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    let handle_file = move |_| {
        spawn(async move {
            // Trigger file input via JS and read as base64
            let js = r#"
                await new Promise((resolve, reject) => {
                    const input = document.createElement('input');
                    input.type = 'file';
                    input.accept = 'image/*';
                    input.onchange = () => {
                        const file = input.files[0];
                        if (!file) { resolve(''); return; }
                        const reader = new FileReader();
                        reader.onload = () => resolve(reader.result);
                        reader.onerror = () => reject('Failed to read file');
                        reader.readAsDataURL(file);
                    };
                    input.oncancel = () => resolve('');
                    input.click();
                })
            "#;

            let result = document::eval(js).await;
            let base64_data = match result {
                Ok(val) => {
                    let s: String = serde_json::from_value(val)
                        .unwrap_or_default();
                    if s.is_empty() {
                        return;
                    }
                    s
                }
                Err(e) => {
                    error_msg.set(Some(format!("JS error: {e}")));
                    return;
                }
            };

            loading.set(true);
            error_msg.set(None);

            match ocr_shopee_code(base64_data).await {
                Ok(code) => {
                    loading.set(false);
                    on_code_extracted.call(code);
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
                class: "flex items-center justify-center w-10 h-10 bg-orange-100 dark:bg-orange-900/50 text-orange-600 dark:text-orange-400 rounded-xl hover:bg-orange-200 dark:hover:bg-orange-800 transition-colors disabled:opacity-50",
                r#type: "button",
                disabled: loading(),
                onclick: handle_file,
                if loading() {
                    // Spinner
                    svg {
                        class: "w-5 h-5 animate-spin",
                        view_box: "0 0 24 24",
                        fill: "none",
                        circle {
                            cx: "12",
                            cy: "12",
                            r: "10",
                            stroke: "currentColor",
                            stroke_width: "4",
                            class: "opacity-25",
                        }
                        path {
                            d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z",
                            fill: "currentColor",
                            class: "opacity-75",
                        }
                    }
                } else {
                    // Camera icon
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
                p { class: "text-xs text-red-500 max-w-[120px] text-center", "{err}" }
            }
        }
    }
}

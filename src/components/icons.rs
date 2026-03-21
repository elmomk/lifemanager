use dioxus::prelude::*;

#[component]
pub fn CheckSquareIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m9 11 3 3L22 4" }
            path { d: "M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" }
        }
    }
}

#[component]
pub fn ShoppingCartIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "8", cy: "21", r: "1" }
            circle { cx: "19", cy: "21", r: "1" }
            path { d: "M2.05 2.05h2l2.66 12.42a2 2 0 0 0 2 1.58h9.78a2 2 0 0 0 1.95-1.57l1.65-7.43H5.12" }
        }
    }
}

#[component]
pub fn PackageIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m7.5 4.27 9 5.15" }
            path { d: "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" }
            path { d: "m3.3 7 8.7 5 8.7-5" }
            path { d: "M12 22V12" }
        }
    }
}

#[component]
pub fn TvIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "2", y: "7", width: "20", height: "15", rx: "2", ry: "2" }
            polyline { points: "17 2 12 7 7 2" }
        }
    }
}

#[component]
pub fn HeartIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" }
        }
    }
}

#[component]
pub fn PlusIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M5 12h14" }
            path { d: "M12 5v14" }
        }
    }
}

#[component]
pub fn TrashIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M3 6h18" }
            path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
            path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
        }
    }
}

#[component]
pub fn CheckIcon(class: Option<String>) -> Element {
    let class = class.unwrap_or_default();
    rsx! {
        svg {
            class: "{class}",
            xmlns: "http://www.w3.org/2000/svg",
            width: "24", height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M20 6 9 17l-5-5" }
        }
    }
}

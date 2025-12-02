use dioxus::prelude::*;

#[component]
pub fn Page(class: Option<&'static str>, children: Element) -> Element {
    let class: &str = class.unwrap_or_default();

    rsx!(
        div {
            class: "min-h-screen pt-[64px] p-4 {class}",
            {children}
        }
    )
}

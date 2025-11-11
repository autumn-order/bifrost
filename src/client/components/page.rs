use dioxus::prelude::*;

#[component]
pub fn Page(class: Option<&'static str>, children: Element) -> Element {
    let class: &str = if let Some(class) = class { class } else { "" };

    rsx!(
        div {
            class: "min-h-screen pt-[64px] p-4 {class}",
            {children}
        }
    )
}

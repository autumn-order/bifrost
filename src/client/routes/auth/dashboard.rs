use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    rsx!(
        Title { "Dashboard | Bifrost" }
        Meta {
            name: "description",
            content: "EVE Online authentication platform for coalitions, alliances, and corporations."
        }
        div {
            "Auth"
        }
    )
}

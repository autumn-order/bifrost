use dioxus::prelude::*;

use crate::client::components::BifrostTitleButton;

#[component]
pub fn AuthNavbar() -> Element {
    rsx! {
        div {
            class: "navbar bg-base-200 fixed",
            div {
                class: "navbar-start",
                BifrostTitleButton {}

            }
            div {
                class: "navbar-end",
                div { class: "h-10",
                    a { href: "/api/auth/logout",
                        button {
                            class: "btn btn-outline",
                            "Logout"
                        }
                    }
                }
            }
        }
    }
}

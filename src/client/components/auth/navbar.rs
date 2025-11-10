use dioxus::prelude::*;

use crate::client::{components::BifrostTitleButton, router::Route};

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
                div {
                    a { href: "/api/auth/logout",
                        "Logout"
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}

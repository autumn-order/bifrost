use dioxus::prelude::*;

use crate::client::{
    components::{BifrostTitleButton, EveLogin},
    router::Route,
};

#[component]
pub fn Navbar() -> Element {
    rsx! {
        div {
            class: "navbar bg-base-200 fixed",
            div {
                class: "navbar-start",
                BifrostTitleButton {}
            }
            div {
                class: "navbar-end",
                EveLogin {  }
            }
        }

        Outlet::<Route> {}
    }
}

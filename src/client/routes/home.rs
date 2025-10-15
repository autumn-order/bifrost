use dioxus::prelude::*;

use crate::client::components::Hero;

/// Home page
#[component]
pub fn Home() -> Element {
    rsx! {
        Hero {}
    }
}

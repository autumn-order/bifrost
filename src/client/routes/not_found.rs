use dioxus::document::{Meta, Title};
use dioxus::prelude::*;

use crate::client::components::Page;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    rsx!(
        Title { "404 | Not Found" }
        Meta {
            name: "description",
            content: "The page you are looking for does not exist"
        }
        Page { class: "flex items-center justify-center",
            div { class: "flex flex-col items-center gap-2",
                h1 { class: "text-3xl md:text-4xl", "404 Not Found" }
                p { class:"text-xl", "The page you are looking for does not exist" }
            }
        }
    )
}

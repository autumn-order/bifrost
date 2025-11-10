use dioxus::prelude::*;

use crate::client::router::Route;

#[component]
pub fn BifrostTitleButton() -> Element {
    rsx!(
        Link {
            to: Route::Home {},
            div { class: "flex items-center gap-2",
                p { class: "text-xl",
                    "Bifrost"
                }
                p { class: "text-xs",
                    "v0.1.0.Alpha-1"
                }
            }
        }
    )
}

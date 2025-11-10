use dioxus::prelude::*;

pub use crate::client::router::Route;

/// Shared navbar component.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
            }

        }

        Outlet::<Route> {}
    }
}

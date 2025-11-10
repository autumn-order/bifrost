use dioxus::prelude::*;

pub use crate::client::router::Route;

#[component]
pub fn AuthNavbar() -> Element {
    rsx! {
        div {
            class: "navbar bg-base-200 fixed",
            div {
                class: "navbar-start",
                div { class: "flex items-center gap-2",
                    p { class: "text-xl",
                        "Bifrost"
                    }
                    p { class: "text-xs",
                        "v0.1.0.Alpha-1"
                    }
                }
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

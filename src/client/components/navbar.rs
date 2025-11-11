use dioxus::prelude::*;

use crate::client::{
    components::{BifrostTitleButton, EveLogin},
    router::Route,
    store::user::UserState,
};

#[component]
pub fn Navbar() -> Element {
    let user_store = use_context::<Store<UserState>>();

    rsx! {
        div {
            class: "navbar bg-base-200 fixed",
            div {
                class: "navbar-start",
                BifrostTitleButton {}
            }
            div {
                class: "navbar-end",
                ul { class: "flex gap-2 h-10",
                    // Conditionally render based on whether user is logged in
                    if user_store.read().user.is_some() {
                        // User is logged in, show link to auth page
                        li {
                            Link {
                                to: Route::Dashboard {},
                                class: "btn btn-primary w-28",
                                "Go to Auth"
                            }
                        }
                        li {
                            a { href: "/api/auth/logout",
                                button {
                                    class: "btn btn-outline w-28",
                                    "Logout"
                                }
                            }
                        }
                    } else if user_store.read().fetched {
                        // User is not logged in, show login button
                        li {
                            EveLogin {  }
                        }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}

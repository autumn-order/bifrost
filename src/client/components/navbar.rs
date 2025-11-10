use dioxus::prelude::*;

pub use crate::client::router::Route;

const LOGIN_BUTTON_IMG: Asset = asset!(
    "/assets/eve-sso-login-black-large.png",
    ImageAssetOptions::new()
        .with_avif()
        .with_size(ImageSize::Automatic)
);

#[component]
pub fn Navbar() -> Element {
    rsx! {
        div {
            class: "navbar bg-base-200",
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
                    a { href: "/api/auth/login",
                        img {
                            src: LOGIN_BUTTON_IMG,
                            height: 38.33,
                            width: 230
                        }
                    }
                }
            }
        }

        Outlet::<Route> {}
    }
}

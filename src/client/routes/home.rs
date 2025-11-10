use dioxus::prelude::*;

const LOGIN_BUTTON_IMG: Asset = asset!(
    "/assets/eve-sso-login-black-large.png",
    ImageAssetOptions::new()
        .with_avif()
        .with_size(ImageSize::Automatic)
);

#[component]
pub fn Home() -> Element {
    rsx!(div { class: "w-screen h-screen flex items-center justify-center pt-[-64px]",
        div { class: "flex flex-col items-center gap-4",
            div { class: "flex items-center gap-2",
                p { class: "text-2xl",
                    "Bifrost"
                }
                p {
                    "v0.1.0.Alpha-1"
                }
            }
            div {
                a { href: "/api/auth/login",
                    img {
                        src: LOGIN_BUTTON_IMG,
                    }
                }
            }
        }
    })
}

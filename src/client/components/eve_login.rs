use dioxus::prelude::*;

const LOGIN_BUTTON_IMG: Asset = asset!(
    "/assets/eve-sso-login-black-large.png",
    ImageAssetOptions::new()
        .with_avif()
        .with_size(ImageSize::Automatic)
);

#[component]
pub fn EveLogin() -> Element {
    rsx!(
        div {
            a { href: "/api/auth/login",
                img {
                    src: LOGIN_BUTTON_IMG,
                    height: 38.33,
                    width: 230
                }
            }
        }
    )
}

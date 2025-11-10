use dioxus::prelude::*;

use crate::client::{router::Route, store::user::UserState};

const LOGIN_BUTTON_IMG: Asset = asset!(
    "/assets/eve-sso-login-black-large.png",
    ImageAssetOptions::new()
        .with_avif()
        .with_size(ImageSize::Automatic)
);

#[component]
pub fn EveLogin() -> Element {
    // Get the user store from context
    let user_store = use_context::<Store<UserState>>();

    rsx!(
        div {
            // Conditionally render based on whether user is logged in
            if user_store.read().user.is_some() {
                // User is logged in, show link to auth page
                Link {
                    to: Route::Dashboard {},
                    class: "btn btn-primary",
                    "Go to Auth"
                }
            } else if user_store.read().fetched {
                // User is not logged in, show login button
                a { href: "/api/auth/login",
                    img {
                        src: LOGIN_BUTTON_IMG,
                        height: 38.33,
                        width: 230
                    }
                }
            }
        }
    )
}

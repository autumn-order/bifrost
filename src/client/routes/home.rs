use dioxus::document::{Meta, Title};
use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_brands_icons::{FaDiscord, FaGithub};
use dioxus_free_icons::Icon;

use crate::client::components::{EveLogin, Page};
use crate::client::router::Route;
use crate::client::store::user::UserState;

const AUTUMN_LOGO: Asset = asset!(
    "/assets/autumn-logo-dark.png",
    ImageAssetOptions::new()
        .with_avif()
        .with_size(ImageSize::Automatic)
);

#[component]
pub fn Home() -> Element {
    rsx!(
        Title { "Bifrost" }
        Meta {
            name: "description",
            content: "EVE Online authentication platform for coalitions, alliances, and corporations."
        }
        Page { class: "flex items-center justify-center",
            div { class: "flex flex-col items-center gap-4",
                div { class: "pt-6",
                    img {
                        width: 256,
                        height: 256,
                        src: AUTUMN_LOGO
                    }
                }
                div { class: "flex items-center gap-2",
                    p { class: "text-2xl",
                        "Bifrost"
                    }
                    p {
                        {env!("CARGO_PKG_VERSION")}
                    }
                }
                div {
                    LoginButton { }
                }
                div { class: "flex flex-col gap-2 px-4 max-w-256",
                    p { class: "font-bold text-center",
                        "This is a test instance of Bifrost, see the latest " a {
                            class: "link",
                            href: "https://github.com/autumn-order/bifrost/releases",
                            "release notes"
                        }
                        " for details."
                    }
                    p {
                        "To participate in the test, simply login with EVE Online and play around with the character linking system. You can link characters to an account, then logout, login with a character
                        not yet linked and then try to link your previous logged in characters to the new account to see how well transferring characters between accounts behaves. Try to break it."
                    }
                    p {
                        "Keep an eye on the metadata on the auth page that shows when your character information was last updated, your data should update at a rate of:"
                    }
                    ul { class: "list-disc pl-6",
                        li { "Faction Information: 24 hours" }
                        li { "Alliance Information: 24 hours" }
                        li { "Corporation Information: 24 hours" }
                        li { "Character Information: 30 days (ESI has a really long cache for this)"}
                        li { "Affiliations: No longer than 1 hour and 10 minutes " }
                    }
                    p {
                        "There is much more work to be done, this is a very basic implementation of the foundations built so far. Keep an eye on the Autumn Discord for details!"
                    }
                }
                ul { class: "flex flex-wrap justify-center gap-2",
                    li {
                        a { href: "https://discord.gg/HjaGsBBtFg",
                            button {
                                class: "btn btn-outline w-48 flex gap-2",
                                Icon {
                                    width: 24,
                                    height: 24,
                                    icon: FaDiscord
                                }
                                p {
                                    "Autumn Discord"

                                }
                            }
                        }

                    }
                    li {
                        a { href: "https://github.com/autumn-order/bifrost",
                            button {
                                class: "btn btn-outline w-48 flex gap-2",
                                Icon {
                                    width: 24,
                                    height: 24,
                                    icon: FaGithub
                                }
                                p {
                                    "Bifrost GitHub"
                                }
                            }
                        }
                    }
                }
            }
        }
    )
}

#[component]
pub fn LoginButton() -> Element {
    let user_store = use_context::<Store<UserState>>();

    rsx!(
        ul { class: "flex gap-2 h-10",
            if user_store.read().user.is_some() {
                li {
                    Link {
                        to: Route::Dashboard {},
                        class: "btn btn-primary w-28",
                        "Go to Auth"
                    }
                }
                li {
                    a { href: "/api/docs",
                        button {
                            class: "btn btn-secondary w-28",
                            "API Docs"
                        }
                    }
                }
            } else if user_store.read().fetched {
                li {
                    EveLogin {  }
                }
            }
        }
    )
}

use dioxus::document::{Meta, Title};
use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_brands_icons::{FaDiscord, FaGithub};
use dioxus_free_icons::Icon;

use crate::client::components::{EveLogin, Page};
use crate::client::router::Route;
use crate::client::store::user::UserState;

#[component]
pub fn LoginButton() -> Element {
    let user_store = use_context::<Store<UserState>>();

    rsx!(
        ul { class: "flex gap-2",
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

#[component]
pub fn Home() -> Element {
    rsx!(
        Title { "Bifrost Home" }
        Meta {
            name: "description",
            content: "EVE Online authentication platform for coalitions, alliances, and corporations."
        }
        Page { class: "flex items-center justify-center",
            div { class: "flex flex-col items-center gap-4",
                div { class: "flex items-center gap-2",
                    p { class: "text-2xl",
                        "Bifrost"
                    }
                    p {
                        "v0.1.0-Alpha.1"
                    }
                }
                div {
                    LoginButton { }
                }
                div { class: "flex flex-col gap-2 px-4 max-w-256",
                    p { class: "font-bold text-center",
                        "This is a test instance of Bifrost"
                    }
                    p {
                        "Currently we are testing authentication with EVE Online, character linking, and the update job scheduler & worker for updating character, corporation, alliance, & faction information as well as affiliations.
                        This is a very basic implementation of a frontend for the purposes of testing."
                    }
                    p {
                        "To participate in the test, simply login with EVE Online and play around with the character linking system. You can link characters to an account, then logout, login with a character not yet linked and then try to link your previous logged in characters to the new account to see how well transferring characters between accounts behaves. Try to break it."
                    }
                    p {
                        "Additionally, keep an eye on the metadata on the auth page that shows when your character information was last updated, your data should update at a rate of:"
                    }
                    ul { class: "list-disc pl-6",
                        li { "Faction Information: 24 hours" }
                        li { "Alliance Information: 24 hours" }
                        li { "Corporation Information: 24 hours" }
                        li { "Character Information: 30 days (ESI has a really long cache for this)"}
                        li { "Affiliations: No longer than 1 hour and 10 minutes " }
                    }
                    p {
                        "There is much more work to be done, this is a very basic implementation of the foundations built so far. The next test will involve groups, particularly group ownership by alliances, corporations, and even other groups. Keep an eye on the Autumn Discord for details!"
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

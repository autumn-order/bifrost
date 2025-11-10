use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_solid_icons::{FaLink, FaShuffle};
use dioxus_free_icons::Icon;

use crate::{client::store::user::UserState, model::user::CharacterDto};

#[component]
pub fn DashboardCharacterCard(characters: Signal<Vec<CharacterDto>>) -> Element {
    let user_store = use_context::<Store<UserState>>();

    let user = user_store.read();
    let user_data = user.user.as_ref();

    rsx!(
        div {
            class: "card shadow-sm w-full max-w-196",
            div {
                class: "card-body",
                h2 {
                    class: "card-title",
                    "Main Character"
                }
                div { class: "flex flex-col justify-center items-center p-2",
                    if let Some(user) = user_data {
                        div { class: "avatar",
                            div {
                                class: "w-24 h-24 rounded-full",
                                img {
                                    src: format!("https://images.evetech.net/characters/{}/portrait?size=128", user.character_id),
                                    alt: "{user.character_name}",
                                }
                            }
                        }
                        p {
                            class: "text-lg font-semibold mt-2",
                            "{user.character_name}"
                        }
                    } else {
                        div {
                            class: "skeleton h-24 w-24 rounded"
                        }
                        div {
                            class: "skeleton h-6 w-40 mt-2"
                        }
                    }
                }
                div {
                    ul { class: "flex flex-wrap gap-2 justify-center",
                        li {
                            a {
                                href: "/api/auth/login",
                                button { class: "btn btn-outline w-42 flex gap-2",
                                    Icon {
                                        width: 24,
                                        height: 24,
                                        icon: FaLink
                                    }
                                    p {
                                        "Link Character"
                                    }
                                }
                            }
                        }
                        li {
                            a {
                                href: "/api/auth/login?change_main=true",
                                button { class: "btn btn-outline w-42 flex gap-2",
                                    Icon {
                                        width: 24,
                                        height: 24,
                                        icon: FaShuffle
                                    }
                                    p {
                                        "Change Main"
                                    }
                                }
                            }
                        }
                    }
                }
                CharacterTable { characters: characters }
            }
        }
    )
}

#[component]
fn CharacterTable(characters: Signal<Vec<CharacterDto>>) -> Element {
    rsx!(
        div {
            class: "overflow-x-auto",
            table {
                class: "table table-md",
                thead {
                    tr {
                        th { "Character" }
                        th { "Corporation" }
                        th { "Alliance" }
                    }
                }
                tbody {
                    {characters.iter().map(|c| rsx! {
                        tr {
                            td {
                                div {
                                    class: "flex gap-2 items-center",
                                    div { class: "avatar",
                                        div {
                                            class: "w-10 h-10 rounded-full",
                                            img {
                                                src: format!("https://images.evetech.net/characters/{}/portrait?size=64", c.id),
                                                alt: "{c.name}",
                                            }
                                        }
                                    }
                                    p {
                                        "{c.name}"
                                    }
                                }
                            }
                            td {
                                div {
                                    class: "flex gap-2 items-center",
                                    div { class: "avatar",
                                        div {
                                            class: "w-10 h-10",
                                            img {
                                                src: format!("https://images.evetech.net/corporations/{}/logo?size=64", c.corporation.id),
                                                alt: "{c.corporation.name}",
                                            }
                                        }
                                    }
                                    p {
                                        "{c.corporation.name}"
                                    }
                                }
                            }
                            {if let Some(alliance) = &c.alliance {
                                rsx!(
                                    td {
                                        div {
                                            class: "flex gap-2 items-center",
                                            div { class: "avatar",
                                                div {
                                                    class: "w-10 h-10",
                                                    img {
                                                        src: format!("https://images.evetech.net/alliances/{}/logo?size=64", alliance.id),
                                                        alt: "{alliance.name}",
                                                    }
                                                }
                                            }
                                            p {
                                                "{alliance.name}"
                                            }
                                        }
                                    }
                                )
                            } else {
                                rsx!(
                                    td {

                                    }
                                )
                            }
                            }
                        }
                    })}
                }
            }
        }
    )
}

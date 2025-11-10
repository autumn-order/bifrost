use dioxus::prelude::*;
use dioxus_free_icons::icons::fa_solid_icons::FaLink;
use dioxus_free_icons::Icon;
use dioxus_logger::tracing;

use crate::{
    client::{components::Page, store::user::UserState},
    model::user::CharacterDto,
};

#[component]
pub fn Dashboard() -> Element {
    rsx!(
        Title { "Dashboard | Bifrost" }
        Meta {
            name: "description",
            content: "EVE Online authentication platform for coalitions, alliances, and corporations."
        }
        Page { class: "flex flex-col items-center",
            div { class: "w-full h-full max-w-[1440px] p-6 flex justify-center gap-2",
                CharacterCard { }
            }
        }
    )
}

#[component]
pub fn CharacterCard() -> Element {
    let user_store = use_context::<Store<UserState>>();

    let user = user_store.read();
    let user_data = user.user.as_ref();

    rsx!(
        div {
            class: "card shadow-sm w-full max-w-96",
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
                                class: "w-24 rounded-full",
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
                            class: "skeleton h-32 w-32 rounded"
                        }
                        div {
                            class: "skeleton h-6 w-40 mt-2"
                        }
                    }
                }
                div { class: "flex justify-center",
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
                CharacterTable { }
            }
        }
    )
}

#[component]
pub fn CharacterTable() -> Element {
    let mut characters = use_signal(|| Vec::<CharacterDto>::new());
    let user_store = use_context::<Store<UserState>>();

    // Retrieve user characters on component load
    #[cfg(feature = "web")]
    {
        let future = use_resource(|| async move { get_user_characters().await });

        match &*future.read_unchecked() {
            Some(Ok(chars)) => {
                let user = user_store.read();
                let main_character_id = user.user.as_ref().map(|u| u.character_id);

                // Filter out the main character from the list
                let filtered_chars: Vec<CharacterDto> = chars
                    .iter()
                    .filter(|c| Some(c.id) != main_character_id)
                    .cloned()
                    .collect();

                characters.set(filtered_chars);
            }
            Some(Err(err)) => {
                tracing::error!(err);
            }
            None => (),
        }
    }

    rsx!(
        div {
            class: "overflow-x-auto",
            table {
                class: "table table-md",
                thead {
                    tr {
                        th { "Character" }
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
                        }
                    })}
                }
            }
        }
    )
}

/// Retrieve user characters from API
#[cfg(feature = "web")]
pub async fn get_user_characters() -> Result<Vec<CharacterDto>, String> {
    use reqwasm::http::Request;

    let response = Request::get("/api/user/characters")
        .credentials(reqwasm::http::RequestCredentials::Include)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    match response.status() {
        200 => {
            let chars = response
                .json::<Vec<CharacterDto>>()
                .await
                .map_err(|e| format!("Failed to parse user character data: {}", e))?;
            Ok(chars)
        }
        404 => Ok(Vec::new()),
        _ => {
            use crate::model::api::ErrorDto;

            if let Ok(error_dto) = response.json::<ErrorDto>().await {
                Err(format!(
                    "Request failed with status {}: {}",
                    response.status(),
                    error_dto.error
                ))
            } else {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(format!(
                    "Request failed with status {}: {}",
                    response.status(),
                    error_text
                ))
            }
        }
    }
}

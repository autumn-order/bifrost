use dioxus::prelude::*;
use dioxus_logger::tracing;

use crate::{
    client::components::{
        auth::dashboard::{DashboardCharacterCard, DashboardUpdateCard},
        Page,
    },
    model::user::CharacterDto,
};

#[component]
pub fn Dashboard() -> Element {
    let mut characters = use_signal(|| Vec::<CharacterDto>::new());

    // Retrieve user characters on component load
    #[cfg(feature = "web")]
    {
        use crate::client::util::get_user_character::get_user_characters;

        let future = use_resource(|| async move { get_user_characters().await });

        match &*future.read_unchecked() {
            Some(Ok(chars)) => {
                characters.set(chars.clone());
            }
            Some(Err(err)) => {
                tracing::error!(err);
            }
            None => (),
        }
    }

    rsx!(
        Title { "Dashboard | Bifrost" }
        Meta {
            name: "description",
            content: "EVE Online authentication platform for coalitions, alliances, and corporations."
        }
        Page { class: "flex flex-col items-center",
            div { class: "w-full h-full max-w-[1440px] pt-4 flex flex-wrap justify-center gap-4 px-4",
                DashboardCharacterCard { characters: characters }
                DashboardUpdateCard { characters: characters }
            }
        }
    )
}

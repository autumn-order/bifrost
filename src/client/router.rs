use dioxus::prelude::*;

use crate::client::{
    components::Navbar,
    routes::{auth::AuthHome, Home, NotFound},
};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]

        #[route("/")]
        Home {},

        #[route("/:..segments")]
        NotFound { segments: Vec<String> },

    #[end_layout]

    #[nest("/auth")]

        #[route("/")]
        AuthHome {},
}

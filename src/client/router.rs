use dioxus::prelude::*;

use crate::client::{
    components::{auth::AuthLayout, Navbar},
    routes::{auth::Dashboard, Home, NotFound},
};

use crate::client::routes::NotFound as AuthNotFound;

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

        #[layout(AuthLayout)]

        #[route("/")]
        Dashboard {},

        #[route("/:..segments")]
        AuthNotFound { segments: Vec<String> },
}

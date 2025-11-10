use dioxus::prelude::*;

use crate::client::{components::Navbar, routes::Home};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
}

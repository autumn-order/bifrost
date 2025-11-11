use dioxus::prelude::*;

use crate::client::{
    components::{auth::AuthNavbar, Page},
    router::Route,
    store::user::UserState,
};

#[component]
pub fn AuthLayout() -> Element {
    let user_store = use_context::<Store<UserState>>();
    let nav = navigator();

    let user_logged_in = user_store.read().user.is_some();
    let fetch_completed = user_store.read().fetched;

    // Redirect unauthenticated user to login after fetch completes
    use_effect(use_reactive!(|(user_logged_in, fetch_completed)| {
        if !user_logged_in && fetch_completed {
            nav.push(Route::Home {});
        }
    }));

    rsx! {
        // Show loading spinner while user is being fetched
        if !fetch_completed {
            Page { class: "flex items-center justify-center",
                span {
                    class: "loading loading-spinner loading-xl"
                }
            }
        // Render auth pages if user is fetched and logged in
        } else if user_logged_in {
            AuthNavbar {}
            Outlet::<Route> {}
        }
        // If fetched and not logged in, render nothing while redirecting
        // via the use_effect
    }
}

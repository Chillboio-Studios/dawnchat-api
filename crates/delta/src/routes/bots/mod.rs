use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::Route;

mod create;
mod delete;
mod edit;
mod fetch;
mod fetch_owned;
mod fetch_public;
mod invite;
mod reset_oauth2_secret;

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![
        create::create_bot,
        invite::invite_bot,
        fetch_public::fetch_public_bot,
        fetch::fetch_bot,
        fetch_owned::fetch_owned_bots,
        edit::edit_bot,
        reset_oauth2_secret::reset_oauth2_secret,
        delete::delete_bot,
    ]
}

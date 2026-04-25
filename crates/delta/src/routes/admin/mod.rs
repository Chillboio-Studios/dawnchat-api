use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use rocket::{routes, Route};

mod bootstrap;

pub fn routes() -> (Vec<Route>, OpenApi) {
    (routes![bootstrap::panel_bootstrap], OpenApi::new())
}

use revolt_config::config;
use revolt_database::Database;
use revolt_result::{create_error, Result};
use rocket::http::Status;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::{serde::json::Json, State};
use rocket::{delete, get, routes, Route};
use rocket_empty::EmptyResponse;
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use serde::{Deserialize, Serialize};

struct AdminTokenGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminTokenGuard {
    type Error = revolt_result::Error;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let provided = request
            .headers()
            .get("x-admin-token")
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let settings = config().await;
        let expected = settings.admin.api_token.trim();

        if expected.is_empty() {
            return Outcome::Error((Status::Unauthorized, create_error!(NotAuthenticated)));
        }

        if provided == Some(expected) {
            Outcome::Success(AdminTokenGuard)
        } else {
            Outcome::Error((Status::Forbidden, create_error!(NotAuthenticated)))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AdminStats {
    total_users: i64,
    total_servers: i64,
    total_files: i64,
    total_messages: i64,
    online_users: i64,
}

#[get("/stats")]
pub async fn admin_stats(_admin: AdminTokenGuard, _db: &State<Database>) -> Result<Json<AdminStats>> {
    Ok(Json(AdminStats {
        total_users: 0,
        total_servers: 0,
        total_files: 0,
        total_messages: 0,
        online_users: 0,
    }))
}

#[get("/users/<user_id>")]
pub async fn admin_get_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<Json<revolt_database::User>> {
    let user = db.fetch_user(&user_id).await?;
    Ok(Json(user))
}

#[delete("/users/<user_id>")]
pub async fn admin_delete_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<EmptyResponse> {
    db.delete_user(&user_id).await?;
    Ok(EmptyResponse)
}

#[get("/servers/<server_id>")]
pub async fn admin_get_server(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
) -> Result<Json<revolt_database::Server>> {
    let server = db.fetch_server(&server_id).await?;
    Ok(Json(server))
}

#[delete("/servers/<server_id>")]
pub async fn admin_delete_server(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
) -> Result<EmptyResponse> {
    db.delete_server(&server_id).await?;
    Ok(EmptyResponse)
}

#[get("/files/<file_id>")]
pub async fn admin_get_file(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    file_id: String,
) -> Result<Json<revolt_database::File>> {
    let file = db.fetch_attachment("File", &file_id).await?;
    Ok(Json(file))
}

#[delete("/files/<file_id>")]
pub async fn admin_delete_file(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    file_id: String,
) -> Result<EmptyResponse> {
    db.delete_attachment(&file_id).await?;
    Ok(EmptyResponse)
}

pub fn routes() -> (Vec<Route>, OpenApi) {
    (
        routes![
            admin_stats,
            admin_get_user,
            admin_delete_user,
            admin_get_server,
            admin_delete_server,
            admin_get_file,
            admin_delete_file,
        ],
        OpenApi::new(),
    )
}
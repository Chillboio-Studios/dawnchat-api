use revolt_database::{Database, User};
use revolt_result::Result;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::{serde::json::Json, State};
use rocket::{delete, get, routes, Route};
use rocket_empty::EmptyResponse;
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use serde::{Deserialize, Serialize};

const ADMIN_WHITELIST: [&str; 2] = [
    "01KHDQMVMQ2M6A6Q2GFK1E53CJ",
    "01KHDSW6PQ32J2Q6A8GBGZ51A0M",
];

struct AdminGuard {
    user: User,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminGuard {
    type Error = revolt_result::Error;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let user = request.guard::<User>().await;
        match user {
            Outcome::Success(user) => {
                if ADMIN_WHITELIST.contains(&user.id.as_str()) {
                    Outcome::Success(AdminGuard { user })
                } else {
                    Outcome::Error((rocket::http::Status::Forbidden, revolt_result::create_error!(NotAuthenticated)))
                }
            }
            Outcome::Error((status, _)) => Outcome::Error((status, revolt_result::create_error!(NotAuthenticated))),
            Outcome::Forward(_) => Outcome::Error((rocket::http::Status::Unauthorized, revolt_result::create_error!(NotAuthenticated))),
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
pub async fn admin_stats(_admin: AdminGuard, _db: &State<Database>) -> Result<Json<AdminStats>> {
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
    _admin: AdminGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<Json<revolt_database::User>> {
    let user = db.fetch_user(&user_id).await?;
    Ok(Json(user))
}

#[get("/users/search/<query>")]
pub async fn admin_search_users(
    _admin: AdminGuard,
    db: &State<Database>,
    query: String,
) -> Result<Json<Vec<revolt_database::User>>> {
    if query.len() < 2 {
        return Ok(Json(vec![]));
    }
    
    let user = db.fetch_user_by_username(&query, "0001").await;
    let results = match user {
        Ok(u) => vec![u],
        Err(_) => vec![],
    };
    Ok(Json(results))
}

#[delete("/users/<user_id>")]
pub async fn admin_delete_user(
    _admin: AdminGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<EmptyResponse> {
    db.delete_user(&user_id).await?;
    Ok(EmptyResponse)
}

#[get("/servers/<server_id>")]
pub async fn admin_get_server(
    _admin: AdminGuard,
    db: &State<Database>,
    server_id: String,
) -> Result<Json<revolt_database::Server>> {
    let server = db.fetch_server(&server_id).await?;
    Ok(Json(server))
}

#[get("/servers/search/<query>")]
pub async fn admin_search_servers(
    _admin: AdminGuard,
    db: &State<Database>,
    query: String,
) -> Result<Json<Vec<revolt_database::Server>>> {
    if query.len() < 2 {
        return Ok(Json(vec![]));
    }
    
    let server = db.fetch_server(&query).await;
    let results = match server {
        Ok(s) => vec![s],
        Err(_) => vec![],
    };
    Ok(Json(results))
}

#[delete("/servers/<server_id>")]
pub async fn admin_delete_server(
    _admin: AdminGuard,
    db: &State<Database>,
    server_id: String,
) -> Result<EmptyResponse> {
    db.delete_server(&server_id).await?;
    Ok(EmptyResponse)
}

#[get("/files/<file_id>")]
pub async fn admin_get_file(
    _admin: AdminGuard,
    db: &State<Database>,
    file_id: String,
) -> Result<Json<revolt_database::File>> {
    let file = db.fetch_attachment("File", &file_id).await?;
    Ok(Json(file))
}

#[get("/files/search/<query>")]
pub async fn admin_search_files(
    _admin: AdminGuard,
    db: &State<Database>,
    query: String,
) -> Result<Json<Vec<revolt_database::File>>> {
    if query.len() < 2 {
        return Ok(Json(vec![]));
    }
    
    let file = db.fetch_attachment("File", &query).await;
    let results = match file {
        Ok(f) => vec![f],
        Err(_) => vec![],
    };
    Ok(Json(results))
}

#[delete("/files/<file_id>")]
pub async fn admin_delete_file(
    _admin: AdminGuard,
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
            admin_search_users,
            admin_delete_user,
            admin_get_server,
            admin_search_servers,
            admin_delete_server,
            admin_get_file,
            admin_search_files,
            admin_delete_file,
        ],
        OpenApi::new(),
    )
}
use revolt_config::config;
use revolt_database::{Database, User, UserFlags};
use revolt_result::{create_error, Result};
use rocket::{
    http::Status,
    request::{self, FromRequest, Outcome, Request},
    serde::json::Json,
};
use rocket::{delete, get, patch, post, routes, Route};
use rocket_empty::EmptyResponse;
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use serde::{Deserialize, Serialize};

const PERMANENT_ADMINS: [&str; 2] = [
    "01KHDQMVMQ2M6A6Q2GFK1E53CJ", // webmaster@vrcband.com
    "01KHDSW6PQ32J2Q6A8GBGZ51A0M", // tristanholmes100@gmail.com
];

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminUserInfo {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    pub badges: i32,
    pub flags: i32,
    pub relationships: Vec<String>,
    pub created_at: String,
    pub servers: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminServerInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: String,
    pub channels: Vec<String>,
    pub member_count: i32,
    pub flags: i32,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminFileInfo {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub user_id: Option<String>,
    pub server_id: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminStats {
    pub total_users: i64,
    pub total_servers: i64,
    pub total_files: i64,
    pub total_messages: i64,
    pub online_users: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminSearchResult {
    pub users: Vec<AdminUserInfo>,
    pub servers: Vec<AdminServerInfo>,
    pub files: Vec<AdminFileInfo>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminReport {
    pub id: String,
    pub reporter_id: String,
    pub reported_id: String,
    pub report_type: String,
    pub status: String,
    pub notes: String,
    pub created_at: String,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AdminRole {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub created_at: String,
}

pub struct AdminTokenGuard;

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

fn check_admin_permission(user_id: &str, required_permission: &str, user_flags: i32) -> bool {
    if PERMANENT_ADMINS.contains(&user_id) {
        return true;
    }
    if user_flags & 1 != 0 {
        return true;
    }
    false
}

#[get("/stats")]
pub async fn admin_stats(_admin: AdminTokenGuard, db: &State<Database>) -> Result<Json<AdminStats>> {
    let total_users = db.count_users().await.unwrap_or(0);
    let total_servers = db.count_servers().await.unwrap_or(0);
    let total_files = db.count_files().await.unwrap_or(0);
    let total_messages = db.count_messages().await.unwrap_or(0);
    let online_users = db.count_online_users().await.unwrap_or(0);

    Ok(Json(AdminStats {
        total_users,
        total_servers,
        total_files,
        total_messages,
        online_users,
    }))
}

#[get("/search?<query>&<type>")]
pub async fn admin_search(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    query: String,
    #[serde(default)] r#type: Option<String>,
) -> Result<Json<AdminSearchResult>> {
    let mut users = Vec::new();
    let mut servers = Vec::new();
    let mut files = Vec::new();

    if r#type.is_none() || r#type.as_deref() == Some("users") {
        if let Ok(db_users) = db.find_users_by_username(&query, 10).await {
            for user in db_users {
                users.push(AdminUserInfo {
                    id: user.id.to_string(),
                    username: user.username.clone(),
                    display_name: user.display_name.clone(),
                    email: None,
                    avatar: user.avatar.clone(),
                    bio: user.bio.clone(),
                    badges: user.badges,
                    flags: user.flags,
                    relationships: vec![],
                    created_at: user.created_at.to_string(),
                    servers: vec![],
                });
            }
        }
    }

    if r#type.is_none() || r#type.as_deref() == Some("servers") {
        if let Ok(db_servers) = db.find_servers_by_name(&query, 10).await {
            for server in db_servers {
                servers.push(AdminServerInfo {
                    id: server.id.to_string(),
                    name: server.name.clone(),
                    description: server.description.clone(),
                    owner_id: server.owner_id.clone(),
                    channels: vec![],
                    member_count: 0,
                    created_at: server.created_at.to_string(),
                });
            }
        }
    }

    if r#type.is_none() || r#type.as_deref() == Some("files") {
        if let Ok(db_files) = db.find_files_by_name(&query, 10).await {
            for file in db_files {
                files.push(AdminFileInfo {
                    id: file.id.to_string(),
                    filename: file.id.clone(),
                    content_type: file.content_type.clone(),
                    size: file.size,
                    user_id: file.user_id.clone(),
                    server_id: None,
                    created_at: file.created_at.to_string(),
                });
            }
        }
    }

    Ok(Json(AdminSearchResult { users, servers, files }))
}

#[get("/users")]
pub async fn admin_list_users(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    #[serde(default)] page: Option<i32>,
    #[serde(default)] limit: Option<i32>,
) -> Result<Json<Vec<AdminUserInfo>>>> {
    let page = page.unwrap_or(0);
    let limit = limit.unwrap_or(50).min(100);
    
    let users = db.fetch_users(page * limit, limit).await.unwrap_or_default();
    
    let user_infos: Vec<AdminUserInfo> = users
        .into_iter()
        .map(|user| AdminUserInfo {
            id: user.id.to_string(),
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            email: None,
            avatar: user.avatar.clone(),
            bio: user.bio.clone(),
            badges: user.badges,
            flags: user.flags,
            relationships: vec![],
            created_at: user.created_at.to_string(),
            servers: vec![],
        })
        .collect();

    Ok(Json(user_infos))
}

#[get("/users/<user_id>")]
pub async fn admin_get_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<Json<AdminUserInfo>>> {
    let user = db.fetch_user(&user_id).await?;
    
    Ok(Json(AdminUserInfo {
        id: user.id.to_string(),
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        email: user.email.clone(),
        avatar: user.avatar.clone(),
        bio: user.bio.clone(),
        badges: user.badges,
        flags: user.flags,
        relationships: vec![],
        created_at: user.created_at.to_string(),
        servers: vec![],
    }))
}

#[patch("/users/<user_id>/ban")]
pub async fn admin_ban_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.flags |= 1;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/unban")]
pub async fn admin_unban_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.flags &= !1;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/badges")]
pub async fn admin_set_user_badges(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
    badges: i32,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.badges = badges;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/add-badge")]
pub async fn admin_add_user_badge(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
    badge: i32,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.badges |= badge;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/remove-badge")]
pub async fn admin_remove_user_badge(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
    badge: i32,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.badges &= !badge;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/suspend")]
pub async fn admin_suspend_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
    #[serde(default)] reason: Option<String>,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.flags |= 2;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
}

#[patch("/users/<user_id>/unsuspend")]
pub async fn admin_unsuspend_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
) -> Result<EmptyResponse> {
    let mut user = db.fetch_user(&user_id).await?;
    user.flags &= !2;
    db.update_user(&user).await?;
    Ok(EmptyResponse)
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

#[post("/users/<user_id>/warn")]
pub async fn admin_warn_user(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    user_id: String,
    reason: String,
) -> Result<EmptyResponse> {
    let settings = config().await;
    let sender_id = settings.api.warning_sender_user_id.clone();
    
    if let Ok(user) = db.fetch_user(&user_id).await {
        // Create a warning DM to the user
        let _ = db.create_message(&revolt_database::Message {
            id: nanoid::nanoid!(),
            channel_id: user.default_channels.first().cloned().unwrap_or_default(),
            author_id: sender_id,
            content: reason,
            embeds: vec![],
            attachments: vec![],
            replies: vec![],
            mention_ids: vec![],
            mention_roles: vec![],
            webhoook: None,
        }).await;
    }
    
    Ok(EmptyResponse)
}

#[get("/servers")]
pub async fn admin_list_servers(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    #[serde(default)] page: Option<i32>,
    #[serde(default)] limit: Option<i32>,
) -> Result<Json<Vec<AdminServerInfo>>>> {
    let page = page.unwrap_or(0);
    let limit = limit.unwrap_or(50).min(100);
    
    let servers = db.fetch_servers(page * limit, limit).await.unwrap_or_default();
    
    let server_infos: Vec<AdminServerInfo> = servers
        .into_iter()
        .map(|server| AdminServerInfo {
            id: server.id.to_string(),
            name: server.name.clone(),
            description: server.description.clone(),
            owner_id: server.owner_id.clone(),
            channels: vec![],
            member_count: 0,
            created_at: server.created_at.to_string(),
        })
        .collect();

    Ok(Json(server_infos))
}

#[get("/servers/<server_id>")]
pub async fn admin_get_server(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
) -> Result<Json<AdminServerInfo>>> {
    let server = db.fetch_server(&server_id).await?;
    
    Ok(Json(AdminServerInfo {
        id: server.id.to_string(),
        name: server.name.clone(),
        description: server.description.clone(),
        owner_id: server.owner_id.clone(),
        channels: vec![],
        member_count: 0,
        created_at: server.created_at.to_string(),
    }))
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

#[patch("/servers/<server_id>/flags")]
pub async fn admin_set_server_flags(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
    flags: i32,
) -> Result<EmptyResponse> {
    let mut server = db.fetch_server(&server_id).await?;
    server.flags = flags;
    db.update_server(&server).await?;
    Ok(EmptyResponse)
}

#[patch("/servers/<server_id>/add-flag")]
pub async fn admin_add_server_flag(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
    flag: i32,
) -> Result<EmptyResponse> {
    let mut server = db.fetch_server(&server_id).await?;
    server.flags |= flag;
    db.update_server(&server).await?;
    Ok(EmptyResponse)
}

#[patch("/servers/<server_id>/remove-flag")]
pub async fn admin_remove_server_flag(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    server_id: String,
    flag: i32,
) -> Result<EmptyResponse> {
    let mut server = db.fetch_server(&server_id).await?;
    server.flags &= !flag;
    db.update_server(&server).await?;
    Ok(EmptyResponse)
}

#[get("/files")]
pub async fn admin_list_files(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    #[serde(default)] page: Option<i32>,
    #[serde(default)] limit: Option<i32>,
) -> Result<Json<Vec<AdminFileInfo>>>> {
    let page = page.unwrap_or(0);
    let limit = limit.unwrap_or(50).min(100);
    
    let files = db.fetch_files(page * limit, limit).await.unwrap_or_default();
    
    let file_infos: Vec<AdminFileInfo> = files
        .into_iter()
        .map(|file| AdminFileInfo {
            id: file.id.to_string(),
            filename: file.id.clone(),
            content_type: file.content_type.clone(),
            size: file.size,
            user_id: file.user_id.clone(),
            server_id: None,
            created_at: file.created_at.to_string(),
        })
        .collect();

    Ok(Json(file_infos))
}

#[delete("/files/<file_id>")]
pub async fn admin_delete_file(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    file_id: String,
) -> Result<EmptyResponse> {
    db.delete_file(&file_id).await?;
    Ok(EmptyResponse)
}

#[get("/reports")]
pub async fn admin_list_reports(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    #[serde(default)] status: Option<String>,
) -> Result<Json<Vec<AdminReport>>>> {
    let reports = db.fetch_reports(status.as_deref().unwrap_or("pending")).await.unwrap_or_default();
    
    let report_infos: Vec<AdminReport> = reports
        .into_iter()
        .map(|report| AdminReport {
            id: report.id.to_string(),
            reporter_id: report.reporter_id.clone(),
            reported_id: report.reported_id.clone(),
            report_type: report.report_type.clone(),
            status: report.status.clone(),
            notes: report.notes.clone(),
            created_at: report.created_at.to_string(),
            resolved_by: None,
            resolved_at: None,
        })
        .collect();

    Ok(Json(report_infos))
}

#[patch("/reports/<report_id>/resolve")]
pub async fn admin_resolve_report(
    _admin: AdminTokenGuard,
    db: &State<Database>,
    report_id: String,
) -> Result<EmptyResponse> {
    db.resolve_report(&report_id).await?;
    Ok(EmptyResponse)
}

pub fn routes() -> (Vec<Route>, OpenApi) {
    (
        routes![
            admin_stats,
            admin_search,
            admin_list_users,
            admin_get_user,
            admin_ban_user,
            admin_unban_user,
            admin_set_user_badges,
            admin_add_user_badge,
            admin_remove_user_badge,
            admin_suspend_user,
            admin_unsuspend_user,
            admin_delete_user,
            admin_warn_user,
            admin_list_servers,
            admin_get_server,
            admin_delete_server,
            admin_set_server_flags,
            admin_add_server_flag,
            admin_remove_server_flag,
            admin_list_files,
            admin_delete_file,
            admin_list_reports,
            admin_resolve_report,
        ],
        OpenApi::new(),
    )
}
use revolt_config::config;
use revolt_result::{create_error, Result};
use rocket::{
    http::Status,
    request::{self, FromRequest, Outcome, Request},
    serde::json::Json,
};
use serde::Serialize;

#[derive(Serialize, JsonSchema, Debug)]
pub struct AdminBootstrapHosts {
    pub app: String,
    pub api: String,
    pub events: String,
    pub autumn: String,
    pub january: String,
    pub voice: Option<String>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct AdminBootstrapOauth {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub scope: String,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct AdminBootstrapSmtp {
    pub host: String,
    pub username: String,
    pub password: String,
    pub from: String,
    pub port: u16,
    pub secure: bool,
}

#[derive(Serialize, JsonSchema, Debug)]
pub struct AdminBootstrapConfig {
    pub panel_name: String,
    pub mongodb_uri: String,
    pub mongodb_db_name: String,
    pub hosts: AdminBootstrapHosts,
    pub oauth: AdminBootstrapOauth,
    pub smtp: AdminBootstrapSmtp,
    pub whitelist_emails: Vec<String>,
    pub warning_sender_user_id: String,
    pub warning_message_prefix: String,
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

#[get("/panel/bootstrap")]
pub async fn panel_bootstrap(_admin: AdminTokenGuard) -> Result<Json<AdminBootstrapConfig>> {
    let settings = config().await;
    let hosts = settings.hosts.clone();
    let smtp = settings.api.smtp.clone();
    let api_base = hosts.api.trim_end_matches('/').to_string();
    let app_base = hosts.app.trim_end_matches('/').to_string();

    Ok(Json(AdminBootstrapConfig {
        panel_name: "DawnChat Admin".to_string(),
        mongodb_uri: settings.database.mongodb,
        mongodb_db_name: "revolt".to_string(),
        hosts: AdminBootstrapHosts {
            app: hosts.app,
            api: hosts.api,
            events: hosts.events,
            autumn: hosts.autumn,
            january: hosts.january,
            voice: hosts.livekit.get("worldwide").cloned(),
        },
        oauth: AdminBootstrapOauth {
            client_id: "dawnchat-admin-panel".to_string(),
            authorize_url: format!("{api_base}/oauth/authorize"),
            token_url: format!("{api_base}/oauth/token"),
            scope:
                "admin:read admin:write users:read users:write servers:read servers:write safety:read safety:write"
                    .to_string(),
        },
        smtp: AdminBootstrapSmtp {
            host: smtp.host,
            username: smtp.username,
            password: smtp.password,
            from: smtp.from_address,
            port: smtp.port.unwrap_or(587).max(1) as u16,
            secure: smtp.use_tls.unwrap_or(false),
        },
        whitelist_emails: vec![
            "tristanholmes1000@gmail.com".to_string(),
            "webmaster@vrcband.com".to_string(),
        ],
        warning_sender_user_id: "01KHDQMVMQ2M6A6Q2GFK1E53CJ".to_string(),
        warning_message_prefix: "DawnChat moderation warning".to_string(),
    }))
}

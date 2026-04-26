use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use authifier::models::Session;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use revolt_config::config;
use revolt_database::{Bot, Database};
use revolt_result::{create_error, Result};
use rocket::form::Form;
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::State;
use rocket::{get, post, Route};
use rocket::FromForm;
use rocket_empty::EmptyResponse;
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use serde::{Deserialize, Serialize};

const AUTH_CODE_TTL_SECONDS: u64 = 10 * 60;

static AUTH_CODES: Lazy<DashMap<String, AuthorizationCodeEntry>> = Lazy::new(DashMap::new);
static ACCESS_TOKENS: Lazy<DashMap<String, AccessTokenEntry>> = Lazy::new(DashMap::new);
static REFRESH_TOKENS: Lazy<DashMap<String, String>> = Lazy::new(DashMap::new);

#[derive(Clone)]
struct AuthorizationCodeEntry {
    client_id: String,
    user_id: String,
    session: Session,
    redirect_uri: String,
    scopes: Vec<String>,
    expires_at: u64,
}

#[derive(Clone)]
struct AccessTokenEntry {
    client_id: String,
    user_id: String,
    scopes: Vec<String>,
    expires_at: u64,
    refresh_token: String,
    refresh_expires_at: u64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OAuth2StatusResponse {
    pub enabled: bool,
    pub issuer: String,
}

#[derive(Clone, FromForm, Serialize, Deserialize, JsonSchema)]
pub struct OAuth2LoginQuery {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
}

#[derive(FromForm, Serialize, Deserialize, JsonSchema)]
pub struct OAuth2TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OAuth2TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Session>,
}

#[derive(FromForm, Serialize, Deserialize, JsonSchema)]
pub struct OAuth2RevokeRequest {
    pub token: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(FromForm, Serialize, Deserialize, JsonSchema)]
pub struct OAuth2MeQuery {
    pub access_token: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OAuth2MeResponse {
    pub user_id: String,
    pub client_id: String,
    pub scope: String,
    pub expires_at: u64,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn cleanup_oauth2_cache() {
    let now = now_unix();

    AUTH_CODES.retain(|_, entry| entry.expires_at > now);

    ACCESS_TOKENS.retain(|_, entry| {
        let keep = entry.expires_at > now || entry.refresh_expires_at > now;
        if !keep {
            REFRESH_TOKENS.remove(&entry.refresh_token);
        }
        keep
    });

    REFRESH_TOKENS.retain(|_, access_token| ACCESS_TOKENS.contains_key(access_token));
}

fn parse_scopes(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn is_valid_redirect_uri(uri: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(uri) {
        parsed.scheme() == "https" || parsed.scheme() == "http"
    } else {
        false
    }
}

fn validate_scopes(
    requested: Vec<String>,
    client_allowed: &HashSet<String>,
    provider_supported: &HashSet<String>,
) -> Result<Vec<String>> {
    if requested.is_empty() {
        return Err(create_error!(InvalidOperation));
    }

    let mut validated = Vec::with_capacity(requested.len());
    for scope in requested {
        if !provider_supported.contains(&scope) || !client_allowed.contains(&scope) {
            return Err(create_error!(InvalidOperation));
        }
        validated.push(scope);
    }

    Ok(validated)
}

fn validate_client_secret(bot: &Bot, provided_secret: Option<&str>, allow_public_clients: bool) -> bool {
    match bot.oauth2_client_secret.as_deref() {
        Some(secret) => provided_secret.is_some_and(|provided| provided == secret),
        None => allow_public_clients,
    }
}

async fn fetch_oauth_client(db: &Database, client_id: &str) -> Result<Bot> {
    db.fetch_bot_by_oauth2_client_id(client_id)
        .await
        .map_err(|_| create_error!(InvalidCredentials))
}

async fn issue_tokens(
    client_id: String,
    user_id: String,
    scopes: Vec<String>,
    session: Option<Session>,
) -> Result<OAuth2TokenResponse> {
    let settings = config().await.oauth2;
    let now = now_unix();

    let access_token = session
        .as_ref()
        .map(|value| value.token.to_string())
        .unwrap_or_else(|| nanoid::nanoid!(64));
    let refresh_token = nanoid::nanoid!(64);

    ACCESS_TOKENS.insert(
        access_token.clone(),
        AccessTokenEntry {
            client_id,
            user_id,
            scopes: scopes.clone(),
            expires_at: now + settings.access_token_lifetime,
            refresh_token: refresh_token.clone(),
            refresh_expires_at: now + settings.refresh_token_lifetime,
        },
    );
    REFRESH_TOKENS.insert(refresh_token.clone(), access_token.clone());

    Ok(OAuth2TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: settings.access_token_lifetime,
        refresh_token,
        scope: scopes.join(" "),
        session,
    })
}

async fn issue_authorization_code(
    db: &Database,
    session: Session,
    query: OAuth2LoginQuery,
) -> Result<Redirect> {
    cleanup_oauth2_cache();

    let response_type = query.response_type.as_deref().unwrap_or("code");
    if response_type != "code" {
        return Err(create_error!(InvalidOperation));
    }

    if !is_valid_redirect_uri(&query.redirect_uri) {
        return Err(create_error!(InvalidOperation));
    }

    let client = fetch_oauth_client(db, &query.client_id).await?;

    let allowed_redirect_uris = client
        .oauth2_redirect_uris
        .clone()
        .ok_or_else(|| create_error!(InvalidOperation))?;
    if !allowed_redirect_uris.iter().any(|uri| uri == &query.redirect_uri) {
        return Err(create_error!(InvalidOperation));
    }

    let supported_scopes: HashSet<String> = parse_scopes(Some(&config().await.oauth2.supported_scopes))
        .into_iter()
        .collect();
    let client_scopes: HashSet<String> = client
        .oauth2_scopes
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();

    if supported_scopes.is_empty() || client_scopes.is_empty() {
        return Err(create_error!(FeatureDisabled {
            feature: "oauth2".into()
        }));
    }

    let requested_scopes = {
        let scopes = parse_scopes(query.scope.as_deref());
        if scopes.is_empty() {
            client_scopes.iter().cloned().collect::<Vec<_>>()
        } else {
            scopes
        }
    };

    let scopes = validate_scopes(requested_scopes, &client_scopes, &supported_scopes)?;

    let user_id = session.user_id.to_string();

    let code = nanoid::nanoid!(48);
    AUTH_CODES.insert(
        code.clone(),
        AuthorizationCodeEntry {
            client_id: query.client_id,
            user_id,
            session,
            redirect_uri: query.redirect_uri.clone(),
            scopes,
            expires_at: now_unix() + AUTH_CODE_TTL_SECONDS,
        },
    );

    let mut redirect_url =
        url::Url::parse(&query.redirect_uri).map_err(|_| create_error!(InvalidOperation))?;
    {
        let mut qp = redirect_url.query_pairs_mut();
        qp.append_pair("code", &code);
        if let Some(state) = query.state {
            qp.append_pair("state", &state);
        }
    }

    Ok(Redirect::to(redirect_url.to_string()))
}

#[openapi(tag = "Session")]
#[get("/enabled")]
pub async fn enabled() -> Json<OAuth2StatusResponse> {
    let oauth2 = config().await.oauth2;
    let enabled = !oauth2.issuer.trim().is_empty() && !oauth2.supported_scopes.trim().is_empty();

    Json(OAuth2StatusResponse {
        enabled,
        issuer: oauth2.issuer,
    })
}

#[openapi(tag = "Session")]
#[get("/login?<query..>")]
pub async fn login(
    db: &State<Database>,
    session: Session,
    query: OAuth2LoginQuery,
) -> Result<Redirect> {
    issue_authorization_code(db, session, query).await
}

#[openapi(tag = "Session")]
#[get("/authorize?<query..>")]
pub async fn authorize(
    db: &State<Database>,
    session: Session,
    query: OAuth2LoginQuery,
) -> Result<Redirect> {
    issue_authorization_code(db, session, query).await
}

#[openapi(tag = "Session")]
#[post("/token", data = "<form>")]
pub async fn token(db: &State<Database>, form: Form<OAuth2TokenRequest>) -> Result<Json<OAuth2TokenResponse>> {
    cleanup_oauth2_cache();

    let form = form.into_inner();
    let settings = config().await.oauth2;

    match form.grant_type.as_str() {
        "authorization_code" => {
            let code = form.code.ok_or_else(|| create_error!(InvalidOperation))?;
            let entry = AUTH_CODES
                .remove(&code)
                .map(|(_, value)| value)
                .ok_or_else(|| create_error!(InvalidCredentials))?;

            if entry.expires_at <= now_unix() {
                return Err(create_error!(InvalidCredentials));
            }

            let client_id = form.client_id.unwrap_or_else(|| entry.client_id.clone());
            if client_id != entry.client_id {
                return Err(create_error!(InvalidCredentials));
            }

            if let Some(redirect_uri) = form.redirect_uri.as_deref() {
                if redirect_uri != entry.redirect_uri {
                    return Err(create_error!(InvalidCredentials));
                }
            }

            let bot = fetch_oauth_client(db, &client_id).await?;
            if !validate_client_secret(
                &bot,
                form.client_secret.as_deref(),
                settings.allow_public_clients,
            ) {
                return Err(create_error!(InvalidCredentials));
            }

            let response = issue_tokens(client_id, entry.user_id, entry.scopes, Some(entry.session)).await?;
            Ok(Json(response))
        }
        "refresh_token" => {
            let refresh_token = form
                .refresh_token
                .ok_or_else(|| create_error!(InvalidOperation))?;
            let client_id = form.client_id.ok_or_else(|| create_error!(InvalidOperation))?;

            let access_token = REFRESH_TOKENS
                .get(&refresh_token)
                .map(|entry| entry.value().clone())
                .ok_or_else(|| create_error!(InvalidCredentials))?;

            let entry = ACCESS_TOKENS
                .remove(&access_token)
                .map(|(_, value)| value)
                .ok_or_else(|| create_error!(InvalidCredentials))?;

            if entry.refresh_token != refresh_token || entry.refresh_expires_at <= now_unix() {
                REFRESH_TOKENS.remove(&refresh_token);
                return Err(create_error!(InvalidCredentials));
            }

            if entry.client_id != client_id {
                return Err(create_error!(InvalidCredentials));
            }

            let bot = fetch_oauth_client(db, &client_id).await?;
            if !validate_client_secret(
                &bot,
                form.client_secret.as_deref(),
                settings.allow_public_clients,
            ) {
                return Err(create_error!(InvalidCredentials));
            }

            REFRESH_TOKENS.remove(&refresh_token);
            let response = issue_tokens(client_id, entry.user_id, entry.scopes, None).await?;
            Ok(Json(response))
        }
        _ => Err(create_error!(InvalidOperation)),
    }
}

#[openapi(tag = "Session")]
#[post("/revoke", data = "<form>")]
pub async fn revoke(db: &State<Database>, form: Form<OAuth2RevokeRequest>) -> Result<EmptyResponse> {
    cleanup_oauth2_cache();

    let form = form.into_inner();
    let settings = config().await.oauth2;

    let bot = fetch_oauth_client(db, &form.client_id).await?;
    if !validate_client_secret(
        &bot,
        form.client_secret.as_deref(),
        settings.allow_public_clients,
    ) {
        return Err(create_error!(InvalidCredentials));
    }

    if let Some((_, access)) = ACCESS_TOKENS.remove(&form.token) {
        if access.client_id != form.client_id {
            return Err(create_error!(InvalidCredentials));
        }

        REFRESH_TOKENS.remove(&access.refresh_token);
        return Ok(EmptyResponse);
    }

    if let Some((_, access_token)) = REFRESH_TOKENS.remove(&form.token) {
        if let Some((_, access)) = ACCESS_TOKENS.remove(&access_token) {
            if access.client_id != form.client_id {
                return Err(create_error!(InvalidCredentials));
            }
        }
    }

    Ok(EmptyResponse)
}

#[openapi(tag = "Session")]
#[get("/@me?<query..>")]
pub async fn me(query: OAuth2MeQuery) -> Result<Json<OAuth2MeResponse>> {
    cleanup_oauth2_cache();

    let entry = ACCESS_TOKENS
        .get(&query.access_token)
        .map(|item| item.value().clone())
        .ok_or_else(|| create_error!(InvalidCredentials))?;

    if entry.expires_at <= now_unix() {
        return Err(create_error!(InvalidCredentials));
    }

    Ok(Json(OAuth2MeResponse {
        user_id: entry.user_id,
        client_id: entry.client_id,
        scope: entry.scopes.join(" "),
        expires_at: entry.expires_at,
    }))
}

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![enabled, login, authorize, token, revoke, me]
}

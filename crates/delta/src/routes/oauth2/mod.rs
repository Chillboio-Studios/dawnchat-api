use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use authifier::models::{Account, Session};
use authifier::util::normalise_email;
use authifier::Authifier;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use reqwest::header;
use reqwest::redirect::Policy;
use revolt_config::config;
use revolt_result::{create_error, Result};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::State;
use rocket::{get, post, Route};
use rocket::FromForm;
use revolt_rocket_okapi::revolt_okapi::openapi3::OpenApi;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const STATE_TTL_SECONDS: u64 = 10 * 60;
const TICKET_TTL_SECONDS: u64 = 60;

static OAUTH_TICKETS: Lazy<DashMap<String, TicketEntry>> = Lazy::new(DashMap::new);

#[derive(Clone)]
struct OAuth2Config {
    provider_name: String,
    client_id: String,
    client_secret: Option<String>,
    authorize_url: String,
    token_url: String,
    userinfo_url: String,
    scope: String,
    callback_url: String,
    state_secret: String,
    email_field: String,
}

#[derive(Clone)]
struct TicketEntry {
    session: Session,
    expires_at: u64,
}

#[derive(Serialize, Deserialize)]
struct OAuth2State {
    nonce: String,
    exp: u64,
    redirect_uri: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OAuth2StatusResponse {
    pub enabled: bool,
    pub provider: Option<String>,
}

#[derive(FromForm, Serialize, Deserialize, JsonSchema)]
pub struct OAuth2AuthorizeQuery {
    pub redirect_uri: Option<String>,
}

#[derive(FromForm, Deserialize, JsonSchema)]
pub struct OAuth2CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OAuth2ExchangeRequest {
    pub ticket: String,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn cleanup_tickets() {
    let now = now_unix();
    OAUTH_TICKETS.retain(|_, entry| entry.expires_at > now);
}

fn is_valid_redirect_uri(uri: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(uri) {
        parsed.scheme() == "https" || parsed.scheme() == "http"
    } else {
        false
    }
}

fn state_signature(payload: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn encode_state(state: &OAuth2State, secret: &str) -> std::result::Result<String, String> {
    let payload = serde_json::to_vec(state).map_err(|_| "state-encode")?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    let signature = state_signature(&payload_b64, secret);
    Ok(format!("{}.{}", payload_b64, signature))
}

fn decode_state(value: &str, secret: &str) -> std::result::Result<OAuth2State, String> {
    let mut split = value.splitn(2, '.');
    let payload_b64 = split.next().ok_or("state-missing")?;
    let signature = split.next().ok_or("state-missing")?;

    if state_signature(payload_b64, secret) != signature {
        return Err("state-signature".into());
    }

    let payload = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| "state-decode")?;
    let state: OAuth2State = serde_json::from_slice(&payload).map_err(|_| "state-json")?;

    if state.exp < now_unix() {
        return Err("state-expired".into());
    }

    if !is_valid_redirect_uri(&state.redirect_uri) {
        return Err("state-redirect".into());
    }

    Ok(state)
}

async fn oauth2_config() -> Option<OAuth2Config> {
    let settings = config().await;

    let provider_name = env::var("DAWNCHAT_OAUTH2_PROVIDER_NAME").unwrap_or_else(|_| "OAuth2".into());
    let client_id = env::var("DAWNCHAT_OAUTH2_CLIENT_ID").ok()?;
    let authorize_url = env::var("DAWNCHAT_OAUTH2_AUTHORIZE_URL").ok()?;
    let token_url = env::var("DAWNCHAT_OAUTH2_TOKEN_URL").ok()?;
    let userinfo_url = env::var("DAWNCHAT_OAUTH2_USERINFO_URL").ok()?;

    let callback_url = env::var("DAWNCHAT_OAUTH2_CALLBACK_URL").unwrap_or_else(|_| {
        format!(
            "{}/auth/oauth2/callback",
            settings.hosts.api.trim_end_matches('/'),
        )
    });

    if !is_valid_redirect_uri(&callback_url) {
        return None;
    }

    let fallback_secret = settings.api.security.authifier_shield_key;
    let state_secret = env::var("DAWNCHAT_OAUTH2_STATE_SECRET").unwrap_or(fallback_secret);
    if state_secret.trim().is_empty() {
        return None;
    }

    Some(OAuth2Config {
        provider_name,
        client_id,
        client_secret: env::var("DAWNCHAT_OAUTH2_CLIENT_SECRET").ok(),
        authorize_url,
        token_url,
        userinfo_url,
        scope: env::var("DAWNCHAT_OAUTH2_SCOPE").unwrap_or_else(|_| "openid profile email".into()),
        callback_url,
        state_secret,
        email_field: env::var("DAWNCHAT_OAUTH2_EMAIL_FIELD").unwrap_or_else(|_| "email".into()),
    })
}

fn pick_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .map(|field| field.to_string())
}

fn resolve_email(profile: &Value, email_field: &str) -> Option<String> {
    pick_string_field(profile, email_field)
        .or_else(|| pick_string_field(profile, "email"))
        .or_else(|| pick_string_field(profile, "preferred_username"))
        .or_else(|| pick_string_field(profile, "upn"))
}

#[openapi(tag = "Session")]
#[get("/enabled")]
pub async fn enabled() -> Json<OAuth2StatusResponse> {
    let cfg = oauth2_config().await;
    Json(OAuth2StatusResponse {
        enabled: cfg.is_some(),
        provider: cfg.map(|config| config.provider_name),
    })
}

#[openapi(tag = "Session")]
#[get("/authorize?<query..>")]
pub async fn authorize(query: OAuth2AuthorizeQuery) -> Result<Redirect> {
    let cfg = oauth2_config()
        .await
        .ok_or_else(|| create_error!(FeatureDisabled { feature: "oauth2".into() }))?;

    let default_redirect = format!(
        "{}/login/oauth2/callback",
        config().await.hosts.app.trim_end_matches('/'),
    );
    let redirect_uri = query.redirect_uri.unwrap_or(default_redirect);

    if !is_valid_redirect_uri(&redirect_uri) {
        return Err(create_error!(InvalidOperation));
    }

    let state = encode_state(
        &OAuth2State {
            nonce: nanoid::nanoid!(24),
            exp: now_unix() + STATE_TTL_SECONDS,
            redirect_uri,
        },
        &cfg.state_secret,
    )
    .map_err(|_| create_error!(InternalError))?;

    let mut url = url::Url::parse(&cfg.authorize_url).map_err(|_| create_error!(InvalidOperation))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", &cfg.callback_url)
        .append_pair("scope", &cfg.scope)
        .append_pair("state", &state);

    Ok(Redirect::to(url.to_string()))
}

#[openapi(tag = "Session")]
#[get("/callback?<query..>")]
pub async fn callback(authifier: &State<Authifier>, query: OAuth2CallbackQuery) -> Result<Redirect> {
    let cfg = oauth2_config()
        .await
        .ok_or_else(|| create_error!(FeatureDisabled { feature: "oauth2".into() }))?;

    if let Some(error) = query.error {
        let encoded_error: String = url::form_urlencoded::byte_serialize(error.as_bytes()).collect();
        let fallback = format!(
            "{}/login/oauth2/callback?error={}",
            config().await.hosts.app.trim_end_matches('/'),
            encoded_error,
        );
        return Ok(Redirect::to(fallback));
    }

    let code = query.code.ok_or_else(|| create_error!(InvalidOperation))?;
    let state_raw = query.state.ok_or_else(|| create_error!(InvalidOperation))?;

    let state = decode_state(&state_raw, &cfg.state_secret).map_err(|_| create_error!(InvalidCredentials))?;

    let mut token_form = std::collections::HashMap::<String, String>::new();
    token_form.insert("grant_type".into(), "authorization_code".into());
    token_form.insert("code".into(), code);
    token_form.insert("client_id".into(), cfg.client_id.clone());
    token_form.insert("redirect_uri".into(), cfg.callback_url.clone());
    if let Some(secret) = cfg.client_secret.as_ref() {
        token_form.insert("client_secret".into(), secret.clone());
    }

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|_| create_error!(InternalError))?;

    let form_body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in token_form {
            serializer.append_pair(&key, &value);
        }
        serializer.finish()
    };

    let token_response = client
        .post(&cfg.token_url)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .await
        .map_err(|_| create_error!(InternalError))?;

    if !token_response.status().is_success() {
        return Err(create_error!(InvalidCredentials));
    }

    let token_json: Value = token_response
        .json()
        .await
        .map_err(|_| create_error!(InternalError))?;

    let access_token = pick_string_field(&token_json, "access_token")
        .ok_or_else(|| create_error!(InvalidCredentials))?;

    let userinfo_response = client
        .get(&cfg.userinfo_url)
        .header(header::AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|_| create_error!(InternalError))?;

    if !userinfo_response.status().is_success() {
        return Err(create_error!(InvalidCredentials));
    }

    let profile: Value = userinfo_response
        .json()
        .await
        .map_err(|_| create_error!(InternalError))?;

    let email = resolve_email(&profile, &cfg.email_field)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| create_error!(InvalidCredentials))?;

    let normalised = normalise_email(email.clone());
    let account = if let Some(found) = authifier
        .database
        .find_account_by_normalised_email(&normalised)
        .await
        .map_err(|_| create_error!(InternalError))?
    {
        found
    } else {
        Account::new(authifier, email, nanoid::nanoid!(32), false)
            .await
            .map_err(|_| create_error!(InternalError))?
    };

    if account.disabled {
        let redirect = format!(
            "{}?error=disabled",
            state.redirect_uri,
        );
        return Ok(Redirect::to(redirect));
    }

    let session = account
        .create_session(authifier, "DawnChat for Web (OAuth2)".into())
        .await
        .map_err(|_| create_error!(InternalError))?;

    cleanup_tickets();
    let ticket = nanoid::nanoid!(48);
    OAUTH_TICKETS.insert(
        ticket.clone(),
        TicketEntry {
            session,
            expires_at: now_unix() + TICKET_TTL_SECONDS,
        },
    );

    let redirect = format!("{}?ticket={}", state.redirect_uri, ticket);
    Ok(Redirect::to(redirect))
}

#[openapi(tag = "Session")]
#[post("/exchange", data = "<data>")]
pub async fn exchange(data: Json<OAuth2ExchangeRequest>) -> Result<Json<Session>> {
    cleanup_tickets();

    let ticket = data.into_inner().ticket;
    let entry = OAUTH_TICKETS
        .remove(&ticket)
        .map(|(_, value)| value)
        .ok_or_else(|| create_error!(InvalidSession))?;

    if entry.expires_at <= now_unix() {
        return Err(create_error!(InvalidSession));
    }

    Ok(Json(entry.session))
}

pub fn routes() -> (Vec<Route>, OpenApi) {
    openapi_get_routes_spec![enabled, authorize, callback, exchange]
}

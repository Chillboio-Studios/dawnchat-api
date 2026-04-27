use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use revolt_result::{create_error, Error, Result};

use crate::{Database, User};

fn restriction_error(user: &User) -> Option<Error> {
    let flags = user.flags.unwrap_or_default();

    if flags & 4 == 4 {
        return Some(create_error!(AccountBanned {
            error: "User is banned".to_string(),
            reason: user.ban_reason.clone(),
            until: Some("permanent".to_string()),
        }));
    }

    if let Some(suspended_until) = user.suspended_until {
        if suspended_until > iso8601_timestamp::Timestamp::now_utc() {
            return Some(create_error!(AccountBanned {
                error: "User is banned".to_string(),
                reason: user.suspension_reason.clone(),
                until: Some(suspended_until.format().to_string()),
            }));
        }
    }

    None
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for User
where
    Database: FromRef<S>,
    S: Send + Sync
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<User> {
        let db = Database::from_ref(state);

        if let Some(Ok(bot_token)) = parts.headers.get("x-bot-token").map(|v| v.to_str()) {
            let bot = db.fetch_bot_by_token(bot_token).await?;
            let user = db.fetch_user(&bot.id).await?;
            if let Some(error) = restriction_error(&user) {
                Err(error)
            } else {
                Ok(user)
            }
        } else if let Some(Ok(session_token)) =
            parts.headers.get("x-session-token").map(|v| v.to_str())
        {
            let session = db.fetch_session_by_token(session_token).await?;
            let user = db.fetch_user(&session.user_id).await?;
            if let Some(error) = restriction_error(&user) {
                Err(error)
            } else {
                Ok(user)
            }
        } else {
            Err(create_error!(NotAuthenticated))
        }
    }
}

fn check_user_banned(user: &User) -> bool {
    if let Some(flags) = user.flags {
        // UserFlags::Banned = 4, check if the flag is set
        flags & 4 == 4
    } else {
        false
    }
}

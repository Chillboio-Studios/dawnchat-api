use authifier::models::Session;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Outcome, Request};
use revolt_result::{create_error, Error};

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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = Error;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let user: &Option<User> = request
            .local_cache_async(async {
                let db = request.rocket().state::<Database>().expect("`Database`");

                let header_bot_token = request
                    .headers()
                    .get("x-bot-token")
                    .next()
                    .map(|x| x.to_string());

                if let Some(bot_token) = header_bot_token {
                    if let Ok(bot) = db.fetch_bot_by_token(&bot_token).await {
                        if let Ok(user) = db.fetch_user(&bot.id).await {
                            return Some(user);
                        }
                    }
                } else if let Outcome::Success(session) = request.guard::<Session>().await {
                    if let Ok(user) = db.fetch_user(&session.user_id).await {
                        if restriction_error(&user).is_none() {
                            return Some(user);
                        }
                    }
                }

                None
            })
            .await;

        if let Some(user) = user {
            Outcome::Success(user.clone())
        } else {
            let db = request.rocket().state::<Database>().expect("`Database`");

            if let Some(Ok(session_token)) = request
                .headers()
                .get("x-session-token")
                .next()
                .map(|v| Ok::<&str, std::convert::Infallible>(v))
            {
                if let Ok(session) = db.fetch_session_by_token(session_token).await {
                    if let Ok(user) = db.fetch_user(&session.user_id).await {
                        if let Some(error) = restriction_error(&user) {
                            return Outcome::Error((Status::Forbidden, error));
                        }
                    }
                }
            }

            Outcome::Error((Status::Unauthorized, create_error!(InvalidSession)))
        }
    }
}

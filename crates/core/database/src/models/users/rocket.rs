use authifier::models::Session;
use iso8601_timestamp::Timestamp;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};

use crate::{Database, User};

fn user_is_banned(user: &User) -> bool {
    user.flags.unwrap_or_default() & 4 == 4
}

fn user_is_suspended(user: &User) -> bool {
    user.suspended_until
        .is_some_and(|suspended_until| suspended_until > Timestamp::now_utc())
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let Outcome::Success(session) = request.guard::<Session>().await else {
            return Outcome::Forward(Status::Unauthorized);
        };

        let Some(db) = request.rocket().state::<Database>() else {
            return Outcome::Error((Status::InternalServerError, ())); 
        };

        match db.fetch_user(&session.user_id).await {
            Ok(user) => {
                if user_is_banned(&user) || user_is_suspended(&user) {
                    Outcome::Error((Status::Forbidden, ()))
                } else {
                    Outcome::Success(user)
                }
            }
            Err(_) => Outcome::Forward(Status::Unauthorized),
        }
    }
}

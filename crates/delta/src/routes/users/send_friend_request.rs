// use revolt_database::util::reference::Reference;
use revolt_database::{Database, User, AMQP};
use revolt_models::v0;
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;

/// # Send Friend Request
///
/// Send a friend request to another user.
#[openapi(tag = "Relationships")]
#[post("/<target>/friend")]
pub async fn send_friend_request(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<v0::User>> {
    if let Some(muted_until) = user.muted_until {
        if muted_until > iso8601_timestamp::Timestamp::now_utc() {
            return Err(create_error!(Muted));
        }
    }

    if target.id == user.id {
        return Err(create_error!(InvalidOperation));
    }

    user.add_friend(db, amqp, &mut target).await?;
    Ok(Json(target.into(db, &user).await))
}

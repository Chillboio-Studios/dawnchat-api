use revolt_database::User;
use revolt_models::v0::{self, UserFlags};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;

/// # Fetch Self
///
/// Retrieve your user information.
#[openapi(tag = "User Information")]
#[get("/@me")]
pub async fn fetch(user: User) -> Result<Json<v0::User>> {
    if let Some(flags) = user.flags {
        if UserFlags::from_bits_truncate(flags as u32).contains(UserFlags::BANNED) {
            return Err(create_error!(Banned));
        }
    }

    Ok(Json(user.into_self(false).await))
}

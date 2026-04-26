use revolt_database::{util::reference::Reference, Database, PartialBot, User};
use revolt_models::v0;
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};

/// # Reset Bot OAuth2 Secret
///
/// Regenerate OAuth2 credentials for a bot you own.
#[openapi(tag = "Bots")]
#[post("/<bot_id>/oauth2/reset-secret")]
pub async fn reset_oauth2_secret(
    db: &State<Database>,
    user: User,
    bot_id: Reference<'_>,
) -> Result<Json<v0::BotWithUserResponse>> {
    if user.bot.is_some() {
        return Err(create_error!(IsBot));
    }

    let mut bot = bot_id.as_bot(db).await?;
    if bot.owner != user.id {
        return Err(create_error!(NotFound));
    }

    if bot.oauth2_client_id.is_none() {
        bot.oauth2_client_id = Some(nanoid::nanoid!(32));
    }

    let partial = PartialBot {
        oauth2_client_id: bot.oauth2_client_id.clone(),
        oauth2_client_secret: Some(nanoid::nanoid!(64)),
        ..Default::default()
    };

    bot.update(db, partial, vec![]).await?;
    let bot_user = db.fetch_user(&bot.id).await?;

    Ok(Json(v0::BotWithUserResponse {
        bot: bot.into(),
        user: bot_user.into_self(false).await,
    }))
}

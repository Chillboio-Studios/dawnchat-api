use revolt_database::{Bot, Database, User};
use revolt_models::v0;
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use validator::Validate;

/// # Create Bot
///
/// Create a new Revolt bot.
#[openapi(tag = "Bots")]
#[post("/create", data = "<info>")]
pub async fn create_bot(
    db: &State<Database>,
    user: User,
    info: Json<v0::DataCreateBot>,
) -> Result<Json<v0::BotWithUserResponse>> {
    let info = info.into_inner();
    info.validate().map_err(|error| {
        create_error!(FailedValidation {
            error: error.to_string()
        })
    })?;

    // Generate client_id and client_secret if any OAuth2 fields are present
    let (oauth2_client_id, oauth2_client_secret) = if info.oauth2_redirect_uris.is_some() || info.oauth2_scopes.is_some() {
        (Some(nanoid::nanoid!(32)), Some(nanoid::nanoid!(64)))
    } else {
        (None, None)
    };

    let mut partial = revolt_database::PartialBot {
        oauth2_client_id,
        oauth2_client_secret,
        oauth2_redirect_uris: info.oauth2_redirect_uris,
        oauth2_scopes: info.oauth2_scopes,
        ..Default::default()
    };

    let (mut bot, user) = Bot::create(db, info.name, &user, Some(partial)).await?;
    Ok(Json(v0::BotWithUserResponse {
        bot: bot.into(),
        user: user.into_self(false).await,
    }))
}

#[cfg(test)]
mod test {
    use crate::{rocket, util::test::TestHarness};
    use revolt_models::v0;
    use rocket::http::{ContentType, Header, Status};

    #[rocket::async_test]
    async fn create_bot() {
        let harness = TestHarness::new().await;
        let (_, session, _) = harness.new_user().await;

        let response = harness
            .client
            .post("/bots/create")
            .header(Header::new("x-session-token", session.token.to_string()))
            .header(ContentType::JSON)
            .body(
                json!(v0::DataCreateBot {
                    name: TestHarness::rand_string(),
                })
                .to_string(),
            )
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);

        let bot: v0::Bot = response.into_json().await.expect("`Bot`");
        assert!(harness.db.fetch_bot(&bot.id).await.is_ok());
    }
}

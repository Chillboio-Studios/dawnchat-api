use std::collections::HashSet;

use revolt_config::config;
use revolt_database::{util::reference::Reference, Database, PartialBot, User};
use revolt_models::v0::{self, DataEditBot};
use revolt_result::{create_error, Result};
use rocket::State;

use rocket::serde::json::Json;
use validator::Validate;

fn parse_scopes(value: &str) -> Vec<String> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn valid_redirect_uri(uri: &str) -> bool {
    url::Url::parse(uri)
        .map(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// # Edit Bot
///
/// Edit bot details by its id.
#[openapi(tag = "Bots")]
#[patch("/<bot_id>", data = "<data>")]
pub async fn edit_bot(
    db: &State<Database>,
    user: User,
    bot_id: Reference<'_>,
    data: Json<DataEditBot>,
) -> Result<Json<v0::BotWithUserResponse>> {
    let data = data.into_inner();
    data.validate().map_err(|error| {
        create_error!(FailedValidation {
            error: error.to_string()
        })
    })?;

    let mut bot = bot_id.as_bot(db).await?;
    if bot.owner != user.id {
        return Err(create_error!(NotFound));
    }

    let mut user = db.fetch_user(&bot.id).await?;
    if let Some(name) = data.name {
        user.update_username(db, name).await?;
    }

    if let Some(ref redirect_uris) = data.oauth2_redirect_uris {
        if !redirect_uris.iter().all(|uri| valid_redirect_uri(uri)) {
            return Err(create_error!(InvalidOperation));
        }
    }

    if let Some(ref requested_scopes) = data.oauth2_scopes {
        let supported_scopes: HashSet<String> =
            parse_scopes(&config().await.oauth2.supported_scopes)
                .into_iter()
                .collect();

        if requested_scopes.is_empty()
            || requested_scopes
                .iter()
                .any(|scope| !supported_scopes.contains(scope))
        {
            return Err(create_error!(InvalidOperation));
        }
    }


    if data.public.is_none()
        && data.analytics.is_none()
        && data.interactions_url.is_none()
        && data.oauth2_redirect_uris.is_none()
        && data.oauth2_scopes.is_none()
        && data.remove.is_empty()
    {
        return Ok(Json(v0::BotWithUserResponse {
            bot: bot.into(),
            user: user.into_self(false).await,
        }));
    }

    let DataEditBot {
        public,
        analytics,
        interactions_url,
        oauth2_redirect_uris,
        oauth2_scopes,
        remove,
        ..
    } = data;

    // If any OAuth2 field is being set, ensure client_id/secret exist
    let (oauth2_client_id, oauth2_client_secret) = if oauth2_redirect_uris.is_some() || oauth2_scopes.is_some() {
        (
            bot.oauth2_client_id.clone().or(Some(nanoid::nanoid!(32))),
            bot.oauth2_client_secret.clone().or(Some(nanoid::nanoid!(64)))
        )
    } else {
        (bot.oauth2_client_id.clone(), bot.oauth2_client_secret.clone())
    };

    let partial = PartialBot {
        public,
        analytics,
        interactions_url,
        oauth2_client_id,
        oauth2_client_secret,
        oauth2_redirect_uris,
        oauth2_scopes,
        ..Default::default()
    };

    bot.update(
        db,
        partial,
        remove
            .into_iter()
            .map(|v| v.into())
            .collect(),
    )
    .await?;

    Ok(Json(v0::BotWithUserResponse {
        bot: bot.into(),
        user: user.into_self(false).await,
    }))
}

#[cfg(test)]
mod test {
    use crate::{rocket, util::test::TestHarness};
    use revolt_database::Bot;
    use revolt_models::v0::{self, FieldsBot};
    use rocket::http::{ContentType, Header, Status};

    #[rocket::async_test]
    async fn edit_bot() {
        let harness = TestHarness::new().await;
        let (_, session, user) = harness.new_user().await;

        let (bot, _) = Bot::create(&harness.db, TestHarness::rand_string(), &user, None)
            .await
            .expect("`Bot`");

        let response = harness
            .client
            .patch(format!("/bots/{}", bot.id))
            .header(ContentType::JSON)
            .body(
                json!(v0::DataEditBot {
                    public: Some(true),
                    remove: vec![FieldsBot::Token],
                    ..Default::default()
                })
                .to_string(),
            )
            .header(Header::new("x-session-token", session.token.to_string()))
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);

        let updated_bot: v0::Bot = response.into_json().await.expect("`Bot`");
        assert!(!bot.public);
        assert!(updated_bot.public);
    }
}

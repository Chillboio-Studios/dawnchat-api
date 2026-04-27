use rocket::request::{FromRequest, Outcome, Request};
use rocket::http::Status;

use crate::User;

// Note: This implementation is a stub that relies on the Session from authifier.
// The full User object should be fetched by route handlers when needed using the
// session's user_id. Ban checking should be done at the handler or middleware level
// with proper database access.
#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();

    async fn from_request(_req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // This would require access to the database and session parsing,
        // which should be handled at the route handler level instead
        Outcome::Forward(Status::Unauthorized)
    }
}

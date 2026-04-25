use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, StatusClass};
use rocket::{Request, Response};
use url::Url;

pub struct ProxyAwareRedirectFairing;

fn parse_forwarded_proto(forwarded: &str) -> Option<&str> {
    for segment in forwarded.split(',') {
        for pair in segment.split(';') {
            let mut parts = pair.trim().splitn(2, '=');
            let key = parts.next()?.trim();
            let value = parts.next()?.trim().trim_matches('"');

            if key.eq_ignore_ascii_case("proto") {
                return Some(value);
            }
        }
    }

    None
}

#[rocket::async_trait]
impl Fairing for ProxyAwareRedirectFairing {
    fn info(&self) -> Info {
        Info {
            name: "Proxy-aware redirect scheme rewrite",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if response.status().class() != StatusClass::Redirection {
            return;
        }

        let location = match response.headers().get_one("Location") {
            Some(location) => location,
            None => return,
        };

        // Always prefer origin-relative redirect locations for this API when
        // the framework emitted an absolute URL. This keeps reverse proxies in
        // charge of the public scheme/host and avoids redirect loops when the
        // upstream app is contacted over plain HTTP.
        if let Ok(parsed) = Url::parse(location) {
            if matches!(parsed.scheme(), "http" | "https") {
                let path = parsed.path();
                let query = parsed
                    .query()
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("?{value}"))
                    .unwrap_or_default();
                let fragment = parsed
                    .fragment()
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("#{value}"))
                    .unwrap_or_default();
                response.set_header(Header::new(
                    "Location",
                    format!("{path}{query}{fragment}"),
                ));
                return;
            }
        }

        let target_scheme = request
            .headers()
            .get_one("X-Forwarded-Proto")
            .or_else(|| request.headers().get_one("Forwarded").and_then(parse_forwarded_proto))
            .unwrap_or("http");

        if target_scheme.eq_ignore_ascii_case("http") {
            if let Some(stripped) = location.strip_prefix("https://") {
                response.set_header(Header::new("Location", format!("http://{stripped}")));
            }
        }
    }
}

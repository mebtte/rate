use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct BasicAuth([u8; 32]);

impl BasicAuth {
    pub fn new(username: &str, password: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(username);
        digest.update(b":");
        digest.update(password);
        Self(digest.finalize().into())
    }

    fn accepts(&self, authorization: &str) -> bool {
        let mut parts = authorization.split_ascii_whitespace();
        let (Some(scheme), Some(token), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        if !scheme.eq_ignore_ascii_case("Basic") {
            return false;
        }
        let Ok(bytes) = STANDARD.decode(token) else {
            return false;
        };
        // Compare fixed-size digests so the comparison does not reveal the secret's length.
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        bool::from(self.0.ct_eq(&actual))
    }
}

pub async fn require(State(auth): State<BasicAuth>, request: Request, next: Next) -> Response {
    let mut authorizations = request.headers().get_all(header::AUTHORIZATION).iter();
    let valid = match (authorizations.next(), authorizations.next()) {
        (Some(value), None) => value.to_str().is_ok_and(|value| auth.accepts(value)),
        _ => false,
    };

    if valid {
        let mut response = next.run(request).await;
        // Shared caches must not retain assets served to an authenticated request.
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store, no-transform"),
        );
        return response;
    }
    (
        StatusCode::UNAUTHORIZED,
        [
            (
                header::WWW_AUTHENTICATE,
                r#"Basic realm="rate", charset="UTF-8""#,
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
    )
        .into_response()
}

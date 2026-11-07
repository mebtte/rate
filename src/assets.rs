#[cfg(rate_has_web_assets)]
use axum::http::header;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[cfg(rate_has_web_assets)]
static ASSETS: include_dir::Dir<'_> = include_dir::include_dir!("$RATE_WEB_DIST");

#[cfg(rate_has_web_assets)]
const CONTENT_SECURITY_POLICY: &str = concat!(
    "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; ",
    "connect-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; ",
    "frame-ancestors 'none'"
);

pub async fn index() -> Response {
    if cfg!(rate_has_web_assets) {
        serve("index.html")
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Build web/dist and rebuild rate to serve the web interface.",
        )
            .into_response()
    }
}

pub async fn file(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    serve(&path)
}

#[cfg(rate_has_web_assets)]
fn serve(path: &str) -> Response {
    if path
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".."))
        || path.contains('\\')
    {
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some(file) = ASSETS.get_file(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = mime_guess::from_path(path).first_or_octet_stream();
    let cache_control = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    (
        [
            (header::CONTENT_TYPE, content_type.essence_str()),
            (header::CACHE_CONTROL, cache_control),
            // A Worker has its own policy, so send these headers with the assets too.
            (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::X_FRAME_OPTIONS, "DENY"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        file.contents(),
    )
        .into_response()
}

#[cfg(not(rate_has_web_assets))]
fn serve(_: &str) -> Response {
    StatusCode::NOT_FOUND.into_response()
}

#[cfg(all(test, rate_has_web_assets))]
mod tests {
    use super::*;

    #[test]
    fn page_and_worker_have_browser_security_headers() {
        let worker = ASSETS
            .get_dir("assets")
            .unwrap()
            .files()
            .find(|file| {
                file.path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("speed.worker-") && name.ends_with(".js"))
            })
            .expect("the frontend build includes a speed test worker");

        for path in ["index.html", worker.path().to_str().unwrap()] {
            let response = serve(path);
            assert_eq!(response.status(), StatusCode::OK);
            let headers = response.headers();
            let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
            assert!(policy.contains("frame-ancestors 'none'"));
            assert!(policy.contains("connect-src 'self'"));
            assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        }
    }
}

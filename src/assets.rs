#[cfg(rate_has_web_assets)]
use axum::http::header;
use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

#[cfg(rate_has_web_assets)]
static ASSETS: include_dir::Dir<'_> = include_dir::include_dir!("$RATE_WEB_DIST");

#[cfg(rate_has_web_assets)]
const CONTENT_SECURITY_POLICY: &str = concat!(
    "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; ",
    "connect-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; ",
    "frame-ancestors 'none'"
);

pub fn router(title: Option<&str>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/{*path}", get(file))
        .with_state(customize_index(title))
}

async fn index(State(custom_index): State<Option<Bytes>>) -> Response {
    if cfg!(rate_has_web_assets) {
        serve("index.html", custom_index)
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Build web/dist and rebuild rate to serve the web interface.",
        )
            .into_response()
    }
}

async fn file(State(custom_index): State<Option<Bytes>>, Path(path): Path<String>) -> Response {
    serve(&path, custom_index)
}

#[cfg(rate_has_web_assets)]
fn customize_index(title: Option<&str>) -> Option<Bytes> {
    let title = title.filter(|title| !title.trim().is_empty())?;
    let html = ASSETS.get_file("index.html")?.contents_utf8()?;
    // Escape the title before inserting it into the built HTML template.
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    Some(Bytes::from(html.replacen(
        "<title>rate</title>",
        &format!("<title>{title} - rate</title>"),
        1,
    )))
}

#[cfg(not(rate_has_web_assets))]
fn customize_index(_: Option<&str>) -> Option<Bytes> {
    None
}

#[cfg(rate_has_web_assets)]
fn serve(path: &str, custom_index: Option<Bytes>) -> Response {
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
    let contents = custom_index
        .filter(|_| path == "index.html")
        .unwrap_or_else(|| Bytes::from_static(file.contents()));
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
        contents,
    )
        .into_response()
}

#[cfg(not(rate_has_web_assets))]
fn serve(_: &str, _: Option<Bytes>) -> Response {
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
            let response = serve(path, None);
            assert_eq!(response.status(), StatusCode::OK);
            let headers = response.headers();
            let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
            assert!(policy.contains("frame-ancestors 'none'"));
            assert!(policy.contains("connect-src 'self'"));
            assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        }
    }
}

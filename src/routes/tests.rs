use std::{convert::Infallible, net::SocketAddr};

use axum::{
    body::{Body, to_bytes},
    extract::connect_info::MockConnectInfo,
    http::Method,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::Parser;
use tower::ServiceExt;

use super::*;

async fn send(
    app: &Router,
    method: Method,
    path: &str,
    body: Body,
    authorization: Option<&str>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}

fn router(arguments: &[&str]) -> Router {
    build(&Config::try_parse_from(arguments).unwrap()).unwrap()
}

fn peer(address: &str) -> MockConnectInfo<PeerAddress> {
    MockConnectInfo(PeerAddress(address.parse::<SocketAddr>().unwrap()))
}

fn transfer_router() -> Router {
    router(&["rate"]).layer(peer("127.0.0.1:1234"))
}

#[tokio::test]
async fn api_streams_exact_download_bytes_and_acknowledges_upload() {
    let app = transfer_router();
    let ping = send(&app, Method::GET, "/api/v1/ping", Body::empty(), None).await;
    assert_eq!(ping.status(), StatusCode::NO_CONTENT);

    let download = send(
        &app,
        Method::GET,
        "/api/v1/download?bytes=1050000",
        Body::empty(),
        None,
    )
    .await;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers()[header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert_eq!(download.headers()[header::CONTENT_LENGTH], "1050000");
    assert_eq!(
        download.headers()[header::CACHE_CONTROL],
        "no-store, no-transform"
    );
    assert!(!download.headers().contains_key(header::CONTENT_ENCODING));
    let bytes = to_bytes(download.into_body(), 1050000).await.unwrap();
    assert_eq!(bytes.len(), 1050000);
    assert!(bytes.iter().any(|byte| *byte != 0));

    let upload = send(
        &app,
        Method::POST,
        "/api/v1/upload",
        Body::from(bytes),
        None,
    )
    .await;
    assert_eq!(upload.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn transfers_validate_request_size_and_stream_errors() {
    let app = transfer_router();
    for size in ["0", "67108865", "bad"] {
        let path = format!("/api/v1/download?bytes={size}");
        let response = send(&app, Method::GET, &path, Body::empty(), None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let frame = Bytes::from_static(&[0; 1024 * 1024]);
    let body = Body::from_stream(futures_util::stream::iter(std::iter::repeat_n(
        Ok::<_, Infallible>(frame),
        65,
    )));
    let oversized = send(&app, Method::POST, "/api/v1/upload", body, None).await;
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let error = std::io::Error::other("interrupted upload");
    let body = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(error)]));
    let broken = send(&app, Method::POST, "/api/v1/upload", body, None).await;
    assert_eq!(broken.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unfinished_downloads_keep_slots_until_the_body_is_dropped() {
    let app = transfer_router();
    let path = "/api/v1/download?bytes=1";
    let mut outstanding = Vec::new();
    for _ in 0..16 {
        let response = send(&app, Method::GET, path, Body::empty(), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        outstanding.push(response);
    }
    let limited_download = send(&app, Method::GET, path, Body::empty(), None).await;
    assert_eq!(limited_download.status(), StatusCode::TOO_MANY_REQUESTS);
    let limited_upload = send(&app, Method::POST, "/api/v1/upload", Body::empty(), None).await;
    assert_eq!(limited_upload.status(), StatusCode::TOO_MANY_REQUESTS);

    drop(outstanding.pop());
    let response = send(&app, Method::GET, path, Body::empty(), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(to_bytes(response.into_body(), 1).await.unwrap().len(), 1);
    let upload = send(&app, Method::POST, "/api/v1/upload", Body::empty(), None).await;
    assert_eq!(upload.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn api_blocks_cross_origin_browser_requests() {
    let app = transfer_router();
    for (site, expected) in [
        ("cross-site", StatusCode::FORBIDDEN),
        ("same-site", StatusCode::FORBIDDEN),
        ("same-origin", StatusCode::OK),
        ("none", StatusCode::OK),
    ] {
        let request = Request::builder()
            .uri("/api/v1/download?bytes=1")
            .header("Sec-Fetch-Site", site)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), expected);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store, no-transform"
        );
    }
}

#[tokio::test]
async fn client_serializes_peer_ip_without_guessing_local_geo() {
    let app = router(&["rate"]).layer(peer("[::ffff:127.0.0.1]:1234"));
    let response = send(&app, Method::GET, "/api/v1/client", Body::empty(), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
        serde_json::json!({"ip": "127.0.0.1"})
    );
}

#[tokio::test]
async fn client_only_reads_forwarded_for_from_a_trusted_peer() {
    for (address, expected_ip) in [
        ("127.0.0.2:1234", "127.0.0.2"),
        ("127.0.0.1:1234", "192.0.2.3"),
    ] {
        let app = router(&["rate", "--trusted-proxy", "127.0.0.1,10.0.0.2"]).layer(peer(address));
        let request = Request::builder()
            .uri("/api/v1/client")
            .header("X-Forwarded-For", "203.0.113.1, 192.0.2.3")
            .header("X-Forwarded-For", "10.0.0.2")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let json = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
            serde_json::json!({"ip": expected_ip})
        );
    }
}

#[tokio::test]
async fn client_resolves_forwarded_chains_across_trusted_ipv4_and_ipv6_subnets() {
    let app = router(&["rate", "--trusted-proxy", "172.20.0.0/16,fd00:1234::/64"]);
    for (address, forwarded, nearest_proxy, expected_ip) in [
        (
            "[::ffff:172.20.3.7]:1234",
            "203.0.113.1, 192.0.2.3",
            "fd00:1234::8",
            "192.0.2.3",
        ),
        (
            "172.21.3.7:1234",
            "203.0.113.1, 192.0.2.3",
            "fd00:1234::8",
            "172.21.3.7",
        ),
        (
            "[fd00:1234::9]:1234",
            "2001:db8::1, 2001:db8::2",
            "172.20.250.8",
            "2001:db8::2",
        ),
        (
            "[fd00:1235::9]:1234",
            "2001:db8::1, 2001:db8::2",
            "172.20.250.8",
            "fd00:1235::9",
        ),
    ] {
        let request = Request::builder()
            .uri("/api/v1/client")
            .header("X-Forwarded-For", forwarded)
            .header("X-Forwarded-For", nearest_proxy)
            .body(Body::empty())
            .unwrap();
        let response = app
            .clone()
            .layer(peer(address))
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json = to_bytes(response.into_body(), 1024).await.unwrap();
        let result = serde_json::from_slice::<serde_json::Value>(&json).unwrap();
        assert_eq!(result["ip"], expected_ip, "socket peer {address}");
    }
}

#[cfg(rate_has_geoip_db)]
#[tokio::test]
async fn embedded_database_returns_public_ip_details() {
    let app = router(&["rate"]).layer(peer("8.8.8.8:1234"));
    let response = send(&app, Method::GET, "/api/v1/client", Body::empty(), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = to_bytes(response.into_body(), 1024).await.unwrap();
    let result = serde_json::from_slice::<serde_json::Value>(&json).unwrap();
    assert_eq!(result["ip"], "8.8.8.8");
    assert_eq!(result["countryCode"], "US");
    assert_eq!(result["asn"], "AS15169");
    assert!(
        result["networkName"]
            .as_str()
            .is_some_and(|name| !name.is_empty())
    );
}

#[tokio::test]
async fn prefix_mount_preserves_query_in_redirect_and_hides_other_paths() {
    let app = router(&["rate", "--base-path", "/tools/speedtest/"]);
    let redirect = send(
        &app,
        Method::GET,
        "/tools/speedtest?a=1",
        Body::empty(),
        None,
    )
    .await;
    assert_eq!(redirect.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        redirect.headers()[header::LOCATION],
        "/tools/speedtest/?a=1"
    );

    let index = send(&app, Method::GET, "/tools/speedtest/", Body::empty(), None).await;
    let expected = if cfg!(rate_has_web_assets) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    assert_eq!(index.status(), expected);

    let ping = send(
        &app,
        Method::GET,
        "/tools/speedtest/api/v1/ping",
        Body::empty(),
        None,
    )
    .await;
    assert_eq!(ping.status(), StatusCode::NO_CONTENT);

    for path in [
        "/",
        "/api/v1/ping",
        "/tools/speedtester/",
        "/tools/speedtest/assets/missing.js",
    ] {
        let response = send(&app, Method::GET, path, Body::empty(), None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn basic_auth_protects_page_assets_api_health_and_redirect() {
    let credentials = r#"rate-user:sp ace: "中文"'$!"#;
    let app = router(&["rate", "--base-path", "/speed", "--basic-auth", credentials]);
    for (method, path) in [
        (Method::GET, "/speed"),
        (Method::GET, "/speed/"),
        (Method::GET, "/speed/assets/file.js"),
        (Method::GET, "/speed/api/v1/client"),
        (Method::POST, "/speed/api/v1/upload"),
        (Method::GET, "/speed/healthz"),
    ] {
        let response = send(&app, method, path, Body::empty(), None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert!(
            response.headers()[header::WWW_AUTHENTICATE]
                .to_str()
                .unwrap()
                .starts_with("Basic ")
        );
    }

    let bad_auth = format!("Basic {}", STANDARD.encode("rate-user:wrong"));
    let response = send(
        &app,
        Method::GET,
        "/speed/healthz",
        Body::empty(),
        Some(&bad_auth),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let good_auth = format!("basic {}", STANDARD.encode(credentials));
    let ambiguous = Request::builder()
        .uri("/speed/healthz")
        .header(header::AUTHORIZATION, &good_auth)
        .header(header::AUTHORIZATION, &bad_auth)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(ambiguous).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = send(
        &app,
        Method::GET,
        "/speed/healthz",
        Body::empty(),
        Some(&good_auth),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    for path in [
        "/speed/",
        "/speed/assets/file.js",
        "/speed/api/v1/ping",
        "/speed/healthz",
    ] {
        let response = send(&app, Method::GET, path, Body::empty(), Some(&good_auth)).await;
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store, no-transform",
            "{path}"
        );
    }
}

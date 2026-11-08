use std::{error::Error, sync::Arc};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{ConnectInfo, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use ipnet::IpNet;
use serde::Deserialize;

use crate::{
    assets,
    auth::{self, BasicAuth},
    config::Config,
    geo::{ClientInfo, GeoLookup},
    limits::TransferLimits,
    listener::PeerAddress,
    proxy, transfer,
};

struct AppState {
    random_block: Bytes,
    geo: GeoLookup,
    trusted_proxies: Vec<IpNet>,
    transfers: TransferLimits,
}

#[derive(Deserialize)]
struct DownloadQuery {
    bytes: Option<usize>,
}

pub fn build(config: &Config) -> Result<Router, Box<dyn Error>> {
    config.validate_ipinfo_key()?;
    let auth = config
        .basic_auth()?
        .map(|(username, password)| BasicAuth::new(username, password));
    let state = Arc::new(AppState {
        random_block: transfer::random_block()
            .map_err(|error| format!("unable to generate download data: {error}"))?,
        geo: GeoLookup::new(config.ipinfo_api_key.as_deref())?,
        trusted_proxies: config.trusted_proxy.clone(),
        transfers: TransferLimits::new(64, 16),
    });

    let api = Router::new()
        .route("/ping", get(|| async { StatusCode::NO_CONTENT }))
        .route("/download", get(download))
        .route("/upload", post(upload))
        .route("/client", get(client))
        .layer(middleware::from_fn(same_origin_api))
        .layer(middleware::from_fn(no_cache));

    let mut app = Router::new()
        .route(
            "/healthz",
            get(|| async { "ok" }).layer(middleware::from_fn(no_cache)),
        )
        .nest("/api/v1", api)
        .with_state(state)
        .merge(assets::router(config.title.as_deref()));

    if let Some(auth) = &auth {
        app = app.layer(middleware::from_fn_with_state(auth.clone(), auth::require));
    }

    if config.base_path == "/" {
        return Ok(app);
    }

    // Mount the homepage at the slash-suffixed path so relative Vite asset URLs resolve.
    let mount_path = format!("{}/", config.base_path);
    let destination = mount_path.clone();
    let mut redirect = get(move |uri: Uri| async move {
        let location = match uri.query() {
            Some(query) => format!("{destination}?{query}"),
            None => destination,
        };
        Redirect::permanent(&location).into_response()
    })
    .layer(middleware::from_fn(no_cache));
    if let Some(auth) = auth {
        redirect = redirect.layer(middleware::from_fn_with_state(auth, auth::require));
    }

    Ok(Router::new()
        .route(&config.base_path, redirect)
        .nest(&mount_path, app))
}

async fn download(
    State(state): State<Arc<AppState>>,
    ConnectInfo(PeerAddress(peer)): ConnectInfo<PeerAddress>,
    headers: HeaderMap,
    Query(query): Query<DownloadQuery>,
) -> Response {
    let ip = proxy::client_ip(peer.ip(), &headers, &state.trusted_proxies);
    transfer::download(
        state.random_block.clone(),
        query.bytes.unwrap_or(transfer::DEFAULT_DOWNLOAD_BYTES),
        &state.transfers,
        ip,
    )
}

async fn upload(
    State(state): State<Arc<AppState>>,
    ConnectInfo(PeerAddress(peer)): ConnectInfo<PeerAddress>,
    headers: HeaderMap,
    body: Body,
) -> StatusCode {
    let ip = proxy::client_ip(peer.ip(), &headers, &state.trusted_proxies);
    transfer::upload(headers, body, &state.transfers, ip).await
}

async fn client(
    State(state): State<Arc<AppState>>,
    ConnectInfo(PeerAddress(peer)): ConnectInfo<PeerAddress>,
    headers: HeaderMap,
) -> Json<ClientInfo> {
    let ip = proxy::client_ip(peer.ip(), &headers, &state.trusted_proxies);
    Json(state.geo.lookup(ip).await)
}

async fn no_cache(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-transform"),
    );
    response
}

async fn same_origin_api(request: Request, next: Next) -> Response {
    // A cross-origin page can trigger a transfer even if CORS hides its response.
    let cross_origin = request
        .headers()
        .get_all("sec-fetch-site")
        .iter()
        .any(|value| !matches!(value.as_bytes(), b"same-origin" | b"none"));
    if cross_origin {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests;

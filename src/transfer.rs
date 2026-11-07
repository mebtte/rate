use std::{future, io, net::IpAddr, time::Duration};

use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::{StreamExt, stream};
use tokio::time::{Instant, timeout_at};

use crate::limits::TransferLimits;

const BLOCK_SIZE: usize = 1024 * 1024;
pub const TRANSFER_LIMIT: usize = 64 * BLOCK_SIZE;
pub const DEFAULT_DOWNLOAD_BYTES: usize = 8 * BLOCK_SIZE;
const UPLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(120);

pub fn random_block() -> Result<Bytes, getrandom::Error> {
    let mut block = vec![0; BLOCK_SIZE];
    getrandom::fill(&mut block)?;
    Ok(block.into())
}

pub fn download(block: Bytes, size: usize, limits: &TransferLimits, client: IpAddr) -> Response {
    if !(1..=TRANSFER_LIMIT).contains(&size) {
        return (
            StatusCode::BAD_REQUEST,
            "bytes must be in the range 1..=67108864",
        )
            .into_response();
    }

    let Some(permit) = limits.try_acquire(client) else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };

    // Keep the permit in the response body until Hyper finishes streaming or drops it.
    let deadline = Instant::now() + TRANSFER_TIMEOUT;
    let data = stream::unfold((block, size, permit), move |(block, remaining, permit)| {
        let chunk_size = remaining.min(block.len());
        future::ready((chunk_size > 0).then(|| {
            if Instant::now() >= deadline {
                return (
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "download timed out",
                    )),
                    (block, 0, permit),
                );
            }
            (
                Ok(block.slice(..chunk_size)),
                (block, remaining - chunk_size, permit),
            )
        }))
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, size.to_string())
        .body(Body::from_stream(data))
        .expect("valid download response")
}

pub async fn upload(
    headers: HeaderMap,
    body: Body,
    limits: &TransferLimits,
    client: IpAddr,
) -> StatusCode {
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|size| size > TRANSFER_LIMIT as u64) {
        return StatusCode::PAYLOAD_TOO_LARGE;
    }
    let Some(_permit) = limits.try_acquire(client) else {
        return StatusCode::TOO_MANY_REQUESTS;
    };

    drain_upload(body, UPLOAD_IDLE_TIMEOUT, TRANSFER_TIMEOUT).await
}

async fn drain_upload(body: Body, idle_timeout: Duration, total_timeout: Duration) -> StatusCode {
    // Cap streamed bytes independently of Content-Length and acknowledge only a complete body.
    let mut total = 0;
    let mut frames = body.into_data_stream();
    let deadline = Instant::now() + total_timeout;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return StatusCode::REQUEST_TIMEOUT;
        }
        let frame = match timeout_at(deadline.min(now + idle_timeout), frames.next()).await {
            Ok(Some(Ok(frame))) => frame,
            Ok(Some(Err(_))) => return StatusCode::BAD_REQUEST,
            Ok(None) => return StatusCode::NO_CONTENT,
            Err(_) => return StatusCode::REQUEST_TIMEOUT,
        };
        total += frame.len();
        if total > TRANSFER_LIMIT {
            return StatusCode::PAYLOAD_TOO_LARGE;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;

    #[tokio::test]
    async fn upload_still_times_out_when_small_chunks_keep_arriving() {
        let trickle = stream::unfold((), |()| async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            Some((Ok::<_, Infallible>(Bytes::from_static(b"x")), ()))
        });
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            drain_upload(
                Body::from_stream(trickle),
                Duration::from_secs(5),
                Duration::from_millis(30),
            ),
        )
        .await
        .unwrap();
        assert_eq!(result, StatusCode::REQUEST_TIMEOUT);
    }
}

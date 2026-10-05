//! Delivery for the fixed public illustrations loaded within the startup byte bounds.
//! Private/generated media still requires df-assets' current metadata/access authority;
//! these public immutable snapshots neither register nor authorize that media.

#[cfg(test)]
mod tests;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use bytes::Bytes;
use std::convert::Infallible;

const MAX_RANGE_HEADER_BYTES: usize = 128;
const MAX_BODY_CHUNK_BYTES: usize = 65536;
const ART_PATH: &str = "/assets/ui/scenes/mara-harbor-v4.png";
const CAMPFIRE_PATH: &str = "/assets/concept-art/scene-campfire-under-stars.webp";
const PORTRAIT_PATH: &str = "/assets/concept-art/vell-avatar.webp";
const INN_PATH: &str = "/assets/concept-art/scene-tavern-barkeep-talk-rain.webp";

#[derive(Clone)]
pub(super) struct PublicAssets {
    pub(super) art: Bytes,
    pub(super) campfire: Bytes,
    pub(super) portrait: Bytes,
    pub(super) inn: Option<Bytes>,
}

pub(super) fn router(assets: PublicAssets) -> Router {
    Router::new()
        .route(ART_PATH, get(art))
        .route(CAMPFIRE_PATH, get(campfire))
        .route(PORTRAIT_PATH, get(portrait))
        .route(INN_PATH, get(inn))
        .with_state(assets)
}

async fn art(State(assets): State<PublicAssets>, method: Method, headers: HeaderMap) -> Response {
    response(assets.art, "image/png", &method, &headers)
}

async fn campfire(
    State(assets): State<PublicAssets>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    response(assets.campfire, "image/webp", &method, &headers)
}

async fn portrait(
    State(assets): State<PublicAssets>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    response(assets.portrait, "image/webp", &method, &headers)
}

async fn inn(State(assets): State<PublicAssets>, method: Method, headers: HeaderMap) -> Response {
    match assets.inn {
        Some(bytes) => response(bytes, "image/webp", &method, &headers),
        None => Redirect::temporary(ART_PATH).into_response(),
    }
}

enum Selection {
    Full,
    Partial { start: usize, end: usize },
    Unsatisfiable,
}

fn selection(headers: &HeaderMap, length: usize) -> Selection {
    // Without a matching validator, If-Range requests receive the complete snapshot.
    if headers.contains_key(header::IF_RANGE) {
        return Selection::Full;
    }
    let mut values = headers.get_all(header::RANGE).iter();
    let Some(value) = values.next() else {
        return Selection::Full;
    };
    if values.next().is_some() || value.as_bytes().len() > MAX_RANGE_HEADER_BYTES {
        return Selection::Full;
    }
    let Some((unit, value)) = value
        .to_str()
        .ok()
        .and_then(|value| value.trim().split_once('='))
    else {
        return Selection::Full;
    };
    if !unit.eq_ignore_ascii_case("bytes") {
        return Selection::Full;
    }
    let Some((start, end)) = value.trim().split_once('-') else {
        return Selection::Full;
    };
    let length = length as u64;
    if start.is_empty() {
        let Some(suffix) = decimal(end) else {
            return Selection::Full;
        };
        return if suffix == 0 || length == 0 {
            Selection::Unsatisfiable
        } else {
            Selection::Partial {
                start: length.saturating_sub(suffix) as usize,
                end: length as usize,
            }
        };
    }
    let start_text = start;
    let Some(start) = decimal(start_text) else {
        return Selection::Full;
    };
    let end = if end.is_empty() {
        u64::MAX
    } else {
        let Some(end_value) = decimal(end) else {
            return Selection::Full;
        };
        let start_text = start_text.trim_start_matches('0');
        let end_text = end.trim_start_matches('0');
        if (start_text.len(), start_text) > (end_text.len(), end_text) {
            return Selection::Full;
        }
        end_value
    };
    if start >= length {
        return Selection::Unsatisfiable;
    }
    Selection::Partial {
        start: start as usize,
        end: end.saturating_add(1).min(length) as usize,
    }
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // A syntactically valid HTTP decimal can exceed u64. Saturation preserves clipping
    // and suffix semantics without overflow or allocating for an untrusted integer.
    Some(value.parse().unwrap_or(u64::MAX))
}

fn response(bytes: Bytes, mime: &'static str, method: &Method, headers: &HeaderMap) -> Response {
    let length = bytes.len();
    // HTTP Range applies to GET; HEAD describes the full representation.
    let selection = if method == Method::GET {
        selection(headers, length)
    } else {
        Selection::Full
    };
    let (status, bytes, content_range) = match selection {
        Selection::Full => (StatusCode::OK, bytes, None),
        Selection::Partial { start, end } => (
            StatusCode::PARTIAL_CONTENT,
            bytes.slice(start..end),
            Some(format!("bytes {start}-{}/{length}", end - 1)),
        ),
        Selection::Unsatisfiable => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            Bytes::new(),
            Some(format!("bytes */{length}")),
        ),
    };
    let content_length = bytes.len();
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        // Poll-driven zero-copy chunks own only the bounded startup snapshot. Dropping
        // the body cancels delivery immediately; there is no worker or read-ahead queue.
        Body::from_stream(futures::stream::unfold(bytes, |mut remaining| async move {
            if remaining.is_empty() {
                return None;
            }
            let chunk = remaining.split_to(remaining.len().min(MAX_BODY_CHUNK_BYTES));
            Some((Ok::<_, Infallible>(chunk), remaining))
        }))
    };
    let mut response = (
        status,
        [
            (header::CONTENT_TYPE, mime.to_owned()),
            (header::ACCEPT_RANGES, "bytes".to_owned()),
            (header::CONTENT_LENGTH, content_length.to_string()),
        ],
        body,
    )
        .into_response();
    if let Some(content_range) = content_range {
        let Ok(value) = content_range.parse() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

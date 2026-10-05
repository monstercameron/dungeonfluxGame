use super::*;
use axum::{body::to_bytes, http::Request};
use futures::StreamExt;
use tonic::codegen::Service;

fn fixture(inn: Option<Bytes>) -> Router {
    router(PublicAssets {
        art: Bytes::from_static(b"0123456789"),
        campfire: Bytes::from_static(b"campfire"),
        portrait: Bytes::from_static(b"portrait"),
        inn,
    })
}

async fn request(router: &mut Router, method: Method, path: &str, headers: HeaderMap) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    *request.headers_mut() = headers;
    Service::call(router, request).await.unwrap()
}

fn range(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, value.parse().unwrap());
    headers
}

#[tokio::test]
async fn allowlisted_routes_return_exact_public_bytes_mime_and_length() {
    let mut router = fixture(Some(Bytes::from_static(b"inn")));
    for (path, mime, expected) in [
        (ART_PATH, "image/png", b"0123456789".as_slice()),
        (CAMPFIRE_PATH, "image/webp", b"campfire".as_slice()),
        (PORTRAIT_PATH, "image/webp", b"portrait".as_slice()),
        (INN_PATH, "image/webp", b"inn".as_slice()),
    ] {
        let response = request(&mut router, Method::GET, path, HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
        assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.len().to_string()
        );
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(to_bytes(response.into_body(), 10).await.unwrap(), expected);
    }
}

#[tokio::test]
async fn get_ranges_deliver_only_selected_bytes_with_exact_interval_headers() {
    let mut router = fixture(Some(Bytes::from_static(b"inn")));
    for (range_value, expected, interval) in [
        ("bytes=0-0", "0", "bytes 0-0/10"),
        ("bytes=2-5", "2345", "bytes 2-5/10"),
        ("bytes=7-999", "789", "bytes 7-9/10"),
        ("bytes=7-", "789", "bytes 7-9/10"),
        ("bytes=-3", "789", "bytes 7-9/10"),
        ("bytes=-20", "0123456789", "bytes 0-9/10"),
        ("bytes=0002-0005", "2345", "bytes 2-5/10"),
        ("Bytes=2-5", "2345", "bytes 2-5/10"),
        (
            "bytes=2-99999999999999999999999999999",
            "23456789",
            "bytes 2-9/10",
        ),
        (
            "bytes=-99999999999999999999999999999",
            "0123456789",
            "bytes 0-9/10",
        ),
    ] {
        let response = request(&mut router, Method::GET, ART_PATH, range(range_value)).await;
        assert_eq!(
            response.status(),
            StatusCode::PARTIAL_CONTENT,
            "{range_value}"
        );
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(response.headers()[header::CONTENT_RANGE], interval);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.len().to_string()
        );
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap(),
            expected.as_bytes()
        );
    }
    // The actual concept-art handlers also accept ranges, including the optional inn.
    for path in [CAMPFIRE_PATH, PORTRAIT_PATH, INN_PATH] {
        let response = request(&mut router, Method::GET, path, range("bytes=1-1")).await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/webp");
        let expected = match path {
            INN_PATH => b"n",
            PORTRAIT_PATH => b"o",
            _ => b"a",
        };
        assert_eq!(
            to_bytes(response.into_body(), 1).await.unwrap(),
            expected.as_slice()
        );
    }
}

#[tokio::test]
async fn unsatisfiable_ranges_return_416_and_public_representation_length() {
    let mut router = fixture(Some(Bytes::new()));
    for range_value in [
        "bytes=10-",
        "bytes=11-30",
        "bytes=-0",
        "bytes=999999999999999999999999-",
        "bytes=0-0",
    ] {
        let path = if range_value == "bytes=0-0" {
            INN_PATH
        } else {
            ART_PATH
        };
        let length = if path == INN_PATH { 0 } else { 10 };
        let response = request(&mut router, Method::GET, path, range(range_value)).await;
        assert_eq!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "{range_value}"
        );
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes */{length}")
        );
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn invalid_unsupported_and_unvalidated_conditional_ranges_use_full_get() {
    let mut router = fixture(None);
    for range_value in [
        "bytes=5-2",
        "bytes=",
        "bytes=-",
        "bytes=+1-2",
        "bytes=1 -2",
        "bytes=0-a",
        "items=0-1",
        "bytes=0-1,3-4",
        "bytes=0-1,",
        "bytes=0-1-2",
        "bytes=999999999999999999999999-99999999999999999999999",
    ] {
        let response = request(&mut router, Method::GET, ART_PATH, range(range_value)).await;
        assert_eq!(response.status(), StatusCode::OK, "{range_value}");
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap(),
            b"0123456789".as_slice()
        );
    }
    let mut repeated = range("bytes=0-1");
    repeated.append(header::RANGE, "bytes=3-4".parse().unwrap());
    let mut conditional = range("bytes=0-1");
    conditional.insert(header::IF_RANGE, "\"unknown-version\"".parse().unwrap());
    for headers in [
        repeated,
        conditional,
        range(&format!("bytes=0-{}", "9".repeat(129))),
    ] {
        let response = request(&mut router, Method::GET, ART_PATH, headers).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap(),
            b"0123456789".as_slice()
        );
    }
}

#[tokio::test]
async fn head_ignores_range_and_describes_full_image_without_bytes() {
    let mut router = fixture(Some(Bytes::from_static(b"inn")));
    for (path, mime, length) in [
        (ART_PATH, "image/png", "10"),
        (CAMPFIRE_PATH, "image/webp", "8"),
        (PORTRAIT_PATH, "image/webp", "8"),
        (INN_PATH, "image/webp", "3"),
    ] {
        for headers in [HeaderMap::new(), range("bytes=0-0"), range("bytes=999-")] {
            let response = request(&mut router, Method::HEAD, path, headers).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
            assert_eq!(response.headers()[header::CONTENT_LENGTH], length);
            assert!(!response.headers().contains_key(header::CONTENT_RANGE));
            assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn missing_optional_inn_preserves_redirect_and_other_paths_cannot_select_bytes() {
    let mut router = fixture(None);
    for method in [Method::GET, Method::HEAD] {
        let response = request(&mut router, method, INN_PATH, range("bytes=0-0")).await;
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(response.headers()[header::LOCATION], ART_PATH);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert!(to_bytes(response.into_body(), 0).await.unwrap().is_empty());
    }
    for path in [
        "/assets/private.webp",
        "/assets/concept-art/../private.webp",
        "/assets/concept-art/other.webp",
    ] {
        let response = request(&mut router, Method::GET, path, range("bytes=0-0")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = request(&mut router, Method::POST, ART_PATH, HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn large_get_and_range_bodies_are_poll_driven_bounded_chunks() {
    let bytes = Bytes::from(vec![7; MAX_BODY_CHUNK_BYTES * 2 + 3]);
    let mut router = router(PublicAssets {
        art: bytes.clone(),
        campfire: Bytes::new(),
        portrait: Bytes::new(),
        inn: None,
    });
    for (headers, expected) in [
        (HeaderMap::new(), bytes.clone()),
        (range("bytes=1-"), bytes.slice(1..)),
    ] {
        let response = request(&mut router, Method::GET, ART_PATH, headers).await;
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.len().to_string()
        );
        let mut stream = response.into_body().into_data_stream();
        let mut received = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= MAX_BODY_CHUNK_BYTES);
            received.extend_from_slice(&chunk);
        }
        assert_eq!(received.as_slice(), expected.as_ref());
    }
    let response = request(&mut router, Method::GET, ART_PATH, HeaderMap::new()).await;
    let mut stream = response.into_body().into_data_stream();
    assert_eq!(
        stream.next().await.unwrap().unwrap().len(),
        MAX_BODY_CHUNK_BYTES
    );
    drop(stream);
    let response = request(&mut router, Method::GET, ART_PATH, range("bytes=-3")).await;
    assert_eq!(
        to_bytes(response.into_body(), 3).await.unwrap(),
        bytes.slice(bytes.len() - 3..)
    );
}

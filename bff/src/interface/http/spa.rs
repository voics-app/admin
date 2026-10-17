//! Serves the admin SPA, embedded into the binary at build time.

use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../web/dist"]
struct WebAssets;

const INDEX: &str = "index.html";

/// Serves an asset by path, falling back to `index.html` for client-side routes.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    match WebAssets::get(path).filter(|_| !path.is_empty()) {
        Some(file) => asset_response(path, file),
        None if looks_like_file(path) => StatusCode::NOT_FOUND.into_response(),
        None => match WebAssets::get(INDEX) {
            Some(file) => asset_response(INDEX, file),
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                "admin web build not embedded",
            )
                .into_response(),
        },
    }
}

fn looks_like_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
}

fn asset_response(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite fingerprints everything under assets/, so those can be cached forever.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut response = file.data.into_owned().into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

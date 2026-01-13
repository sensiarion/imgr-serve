use crate::config::Config;
use crate::filename_extractor::FileNameExtractor;
use crate::image_processing::ProcessingParams;
use crate::image_types::{Extensions, MimeType};
use crate::processing::ProcessingErrorType;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use http::response::Builder;
use log::{debug, info};
use sanitize_filename::sanitize;
use serde_json::json;
use std::fmt::format;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Specify caching headers for serving files
fn caching_headers(builder: Builder, cache_ttl: usize) -> Builder {
    // TODO: pass cache policy via config
    // For user content (profile pictures):
    //
    // Use max-age=86400 (24h) + strong ETag
    //
    // Enable If-None-Match checks

    // 1 year
    let duration = Duration::new(cache_ttl as u64, 0);
    builder
        .header(
            header::CACHE_CONTROL,
            format!("public, max-age={}, immutable", duration.as_secs()),
        )
        .header(
            header::EXPIRES,
            httpdate::fmt_http_date(SystemTime::now() + duration),
        )
}

fn content_disposition_header(filename: Option<String>, extensions: Extensions) -> HeaderValue {
    let full_filename = format!(
        "{}.{}",
        filename
            .unwrap_or("image".to_string())
            .replace("\"", "\\\""),
        extensions.name()
    );
    format!(
        "inline; filename=\"{}\"; filename*=UTF-8''{}",
        &full_filename,
        urlencoding::encode(full_filename.as_str())
    )
    .parse()
    .unwrap()
}

/// Serve images as static files
///
/// If image is not existing, it will be attempted to fetch on configured base api
pub async fn serve_file(
    Path(image_id): Path<String>,
    query: Query<ProcessingParams>,
    State(state): State<Arc<Config>>,
) -> impl IntoResponse {
    let image_id = sanitize(image_id);
    info!("Getting img {}", image_id);

    let result = state.processor.get(image_id.clone(), query.0.clone()).await;
    debug!("processed image {}. Generating response", &image_id);

    let response = match result {
        Ok(img) => caching_headers(Response::builder(), state.client_cache_ttl)
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, img.extension.mime_type())
            .header(
                header::CONTENT_DISPOSITION,
                content_disposition_header(img.filename.clone(), Extensions::Webp),
            )
            .body(Body::from(img.data.as_slice().to_owned())),
        Err(err) => {
            let status = match err.err_type {
                ProcessingErrorType::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST.into(),
            };
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"detail": err.detail}).to_string()))
        }
    };

    debug!("generated response");

    response.unwrap()
}

/// Pre fetch image into cache to prevent fetching on client image request
#[axum::debug_handler]
pub async fn preload_image(
    Path(image_id): Path<String>,
    State(state): State<Arc<Config>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let image_id = sanitize(image_id);
    info!("Preloading img {}", image_id);

    // Check API key without holding a lock
    let server_api_key = state.api_key.clone();
    let api_key = match headers.get("X-API-Key") {
        None => String::new(),
        Some(header) => header.to_str().unwrap_or("").into(),
    };
    if api_key != server_api_key {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "detail": "Mismatched api key"
            })),
        );
    }

    // Prefetch without holding a lock on the entire config
    let result = state
        .processor
        .prefetch(
            image_id.clone(),
            FileNameExtractor::extract(&headers).unwrap_or(image_id.to_string()),
            body.to_vec(),
        )
        .await;
    if let Err(err) = result {
        return (StatusCode::BAD_REQUEST, Json(json!({"detail": err.detail})));
    }

    (StatusCode::OK, Json(json!({"status": "Ok"})))
}

use std::sync::Arc;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use sanitize_filename::sanitize;
use log::{debug, info};
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::body::Body;
use serde_json::json;
use axum::Json;
use crate::config::Config;
use crate::filename_extractor::FileNameExtractor;
use crate::image_processing::ProcessingParams;
use crate::image_types::{Extensions, MimeType};
use crate::processing::ProcessingErrorType;

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
        Ok(img) => Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, img.extension.mime_type())
            .header(
                header::CONTENT_DISPOSITION,
                // TODO: rewrite to support utf-8 file names
                format!(
                    "attachment; filename=\"{}.{}\"",
                    img.filename.clone().unwrap_or("image".to_string()),
                    Extensions::Webp.name()
                ),
            )
            .body(Body::from(img.data.as_slice().to_owned())),
        Err(err) => {
            let status = match err.err_type {
                ProcessingErrorType::NotFound => 404,
                _ => 400,
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
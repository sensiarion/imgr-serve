mod config;
mod filename_extractor;
mod image_processing;
mod image_types;
mod processed_image_cache;
mod processing;
mod proxying_images;
mod storage;
mod types;

use crate::config::Config;
use crate::filename_extractor::FileNameExtractor;
use crate::processing::ProcessingErrorType;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::put;
use axum::{routing::get, Json, Router};
use image_processing::ProcessingParams;
use image_types::{Extensions, MimeType};
use log::{debug, info};
use serde_json::json;
use std::sync::Arc;
use sanitize_filename::sanitize;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

/// Serve images as static files
///
/// If image is not existing, it will be attempted to fetch on configured base api
async fn serve_file(
    Path(image_id): Path<String>,
    query: Query<ProcessingParams>,
    State(state): State<Arc<tokio::sync::Mutex<Config>>>,
) -> impl IntoResponse {
    let image_id = sanitize(image_id);
    info!("Getting img {}", image_id);

    let result = {
        let state = state.clone();
        let mut state_guard = state.lock().await;

        state_guard
            .processor
            .get(image_id.clone(), query.0.clone())
            .await
    };
    debug!("processed image {}. Generating response", &image_id);

    // TODO: fix double unwrap for extension (should to it here and then pass into handler)

    let response = match result {
        Ok(img) => Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, img.extension.mime_type())
            .header(
                header::CONTENT_DISPOSITION,
                // TODO: rewrite to support utf-8 file names
                format!(
                    "attachment; filename=\"{}.{}\"",
                    img.filename.unwrap_or("image".to_string()),
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
async fn preload_image(
    Path(image_id): Path<String>,
    State(state): State<Arc<tokio::sync::Mutex<Config>>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let image_id = sanitize(image_id);
    info!("Preloading img {}", image_id);

    // TODO move api key from main Config
    let server_api_key = {
        let state = state.clone();
        let state_guard = state.lock().await;
        state_guard.api_key.clone()
    };
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

    {
        let state = state.clone();
        let mut state_guard = state.lock().await;
        let result = state_guard
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
    }

    (StatusCode::OK, Json(json!({"status": "Ok"})))
}

#[tokio::main]
async fn main() {
    registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let (host, port) = (config.host.clone(), config.port.clone());

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/images/{id}", get(serve_file))
        .route("/images/{id}", put(preload_image))
        .with_state(Arc::new(tokio::sync::Mutex::new(config)));

    info!("Running server on http://{}:{}", host, port);
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

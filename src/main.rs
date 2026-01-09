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
use crate::types::{serve_background, BackgroundService};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::put;
use axum::{routing::get, Json, Router};
use image_processing::ProcessingParams;
use image_types::{Extensions, MimeType};
use log::{debug, info};
use sanitize_filename::sanitize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

/// Serve images as static files
///
/// If image is not existing, it will be attempted to fetch on configured base api
async fn serve_file(
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
async fn preload_image(
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

fn main() {
    registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Configure rayon's global thread pool to use all available CPU cores
    // This ensures fast_image_resize can utilize all cores for parallel processing
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get() + 2)
        .unwrap_or(8 + 2);
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .expect("Failed to initialize rayon thread pool");

    // Configure tokio runtime with a large blocking thread pool
    // This allows multiple concurrent image processing requests to run in parallel
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_threads)
        .max_blocking_threads(num_threads * 2) // Allow more blocking threads for CPU-intensive work
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    rt.block_on(async {
        let config = Config::from_env();
        let (host, port) = (config.host.clone(), config.port.clone());

        let background_services = config.processor.get_background_services();
        let background_tasks_runner = serve_background(background_services.clone()).await;

        let app = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .route("/images/{id}", get(serve_file))
            .route("/images/{id}", put(preload_image))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::GATEWAY_TIMEOUT,
                Duration::from_secs(10),
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(Arc::new(config));

        info!("Running server on http://{}:{}", host, port);
        // run our app with hyper, listening globally on port 3000
        let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
            .await
            .unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(
                background_services,
                background_tasks_runner,
            ))
            .await
            .unwrap();
    });
}

async fn shutdown_signal(
    background_services: Vec<Arc<RwLock<dyn BackgroundService + Send + Sync>>>,
    background_task_runner: JoinSet<()>,
) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let interrupt = async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    #[cfg(not(unix))]
    let interrupt = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = interrupt => {},
    }

    for s in background_services.iter() {
        let mut service = s.write().await;
        service.stop().await;
    }
    background_task_runner.join_all().await;
}

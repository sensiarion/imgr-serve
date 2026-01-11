mod config;
mod filename_extractor;
mod image_processing;
mod image_types;
mod processed_image_cache;
mod processing;
mod proxying_images;
mod routes;
mod storage;
mod types;
mod persistent_store;

use crate::config::Config;
use crate::types::{serve_background, BackgroundService};
use axum::http::{StatusCode};
use axum::routing::put;
use axum::{routing::get, Router};
use log::{info};
use routes::images;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

/// Configure async runtime and rayon cpu usage with optimal configuration
fn configure_runtime() -> Runtime {
    // Configure rayon's global thread pool to use all available CPU cores
    // This ensures fast_image_resize can utilize all cores for parallel processing
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    rayon::ThreadPoolBuilder::new()
        // We have little overhead on CPU bound tasks (few async locks)
        // so its better to use a little bit more workers to fully utilise CPU
        .num_threads(num_threads + 2)
        .build_global()
        .expect("Failed to initialize rayon thread pool");

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_threads)
        .max_blocking_threads(num_threads * 2) // Allow more blocking threads for CPU-intensive work
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
}

fn main() {
    registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rt = configure_runtime();

    rt.block_on(async {
        let config = Config::from_env();
        let (host, port) = (config.host.clone(), config.port.clone());

        let background_services = config.processor.get_background_services();
        let background_tasks_runner = serve_background(background_services.clone()).await;

        let app = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .route("/images/{id}", get(images::serve_file))
            .route("/images/{id}", put(images::preload_image))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::GATEWAY_TIMEOUT,
                Duration::from_secs(30),
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(Arc::new(config));

        info!("Running server on http://{}:{}", host, port);
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

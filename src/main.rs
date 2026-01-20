extern crate core;

mod config;
mod image_ops;
mod openapi;
mod proxying_images;
mod routes;
mod store;
mod utils;

use crate::config::Config;
use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, put_with};
use aide::openapi::{Info, OpenApi};
use aide::swagger::Swagger;
use axum::routing::get;
use axum::{Extension, Router};
use log::info;
use routes::images;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tower_http::trace::TraceLayer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;
use utils::background::{BackgroundService, serve_background};

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

fn openapi_spec() -> OpenApi {
    OpenApi {
        info: Info {
            title: env!("CARGO_BIN_NAME").to_string(),
            description: Some(
                "Image proxy and processing API with cache-backed resizing.".to_string(),
            ),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn app_init(state: Arc<Config>, enable_docs: bool) -> Router {
    let mut openapi = openapi_spec();

    let api = ApiRouter::new()
        .api_route(
            "/images/{id}",
            get_with(images::serve_file, images::serve_file_docs),
        )
        .api_route(
            "/images/{id}",
            put_with(images::preload_image, images::preload_image_docs),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let mut app = api.finish_api(&mut openapi);

    if enable_docs {
        let openapi = Arc::new(openapi);
        app = app
            .route("/openapi.json", get(routes::openapi::openapi_json))
            .route("/docs", get(Swagger::new("/openapi.json").axum_handler()))
            .layer(Extension(openapi));
    }

    #[cfg(not(debug_assertions))]
    {
        use axum::http::StatusCode;
        use std::time::Duration;
        use tower_http::timeout::TimeoutLayer;
        app = app.layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(30),
        ));
    }

    app
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
        let enable_docs = config.enable_docs;

        let shutdown_channel = tokio::sync::watch::channel(false);
        let background_services = config.processor.get_background_services();
        let background_tasks_runner =
            serve_background(background_services.clone(), shutdown_channel.1).await;

        let state = Arc::new(config);
        let app = app_init(state, enable_docs);

        info!("Running server on http://{}:{}", host, port);
        if enable_docs {
            info!("Docs available at http://{}:{}/docs", host, port);
        }
        let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
            .await
            .unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(
                background_services,
                background_tasks_runner,
                shutdown_channel.0,
            ))
            .await
            .unwrap();
    });
}

async fn shutdown_signal(
    background_services: Vec<Arc<RwLock<dyn BackgroundService + Send + Sync>>>,
    background_task_runner: JoinSet<()>,
    shutdown_channel: tokio::sync::watch::Sender<bool>,
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
    let _ = shutdown_channel.send(true);
    background_task_runner.join_all().await;
}

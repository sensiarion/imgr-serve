mod image_processing;
mod image_types;
mod proxying_images;

use axum::extract::{Path, Query};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::{routing::get, Router};

use crate::image_processing::cast_to_extension;
use image::{DynamicImage, GenericImageView, ImageBuffer, Pixel, Rgba};
use image_processing::{ProcessingParams, DEFAULT_COMPRESSION_QUALITY};
use image_types::{Extensions, IntoImageFormat, MimeType};
use imghdr;
use log::{debug, info};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::registry;
use tracing_subscriber::util::SubscriberInitExt;

const CACHING_IMAGE: &[u8] = include_bytes!("../docs/examples/big_cat.jpg");

async fn serve_file(
    Path(image_id): Path<String>,
    query: Query<ProcessingParams>,
) -> impl IntoResponse {
    info!("Getting img {}", image_id);

    // img processing
    let img_data = CACHING_IMAGE;
    let content_type = imghdr::from_bytes(&img_data);
    if content_type.is_none() || content_type.unwrap().image_format().is_none() {
        panic!("Not a supporting image");
    }

    debug!("determine type");

    // Safety: we processing here only images, passed IntoImageFormat.image_format
    // which is always correct data for image lib
    let img = image::load_from_memory_with_format(
        img_data,
        content_type.unwrap().image_format().unwrap(),
    )
    .unwrap();

    debug!("loaded into lib");

    // resizing
    let resized = image_processing::resize::<DynamicImage>(&img, query.width, query.height);

    debug!("resized");

    let response_data = cast_to_extension::<DynamicImage>(resized, Extensions::Webp, query.quality);
    debug!("encoded");

    // TODO: configure caches headers
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        header::CONTENT_TYPE,
        Extensions::Webp.mime_type().parse().unwrap(),
    );
    // TODO: rewrite to support utf-8 file names
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"image.{}\"", Extensions::Webp.name())
            .parse()
            .unwrap(),
    );

    debug!("generated response");

    (resp_headers, response_data)
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

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/image/{id}", get(serve_file));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

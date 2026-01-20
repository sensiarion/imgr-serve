use crate::config::Config;
use crate::image_ops::image_types::{Extensions, MimeType};
use crate::image_ops::operations::ProcessingParams;
use crate::image_ops::processing::ProcessingErrorType;
use crate::openapi::{ApiKeyHeader, BinaryBody, ImageIdParam};
use crate::routes::errors::{
    GetImageErrorResponse, GetImageErrorType, PreloadImageErrorResponse, PreloadImageErrorType,
};
use crate::routes::responses;
use crate::routes::responses::{ApiError, ImageResponse};
use crate::utils::filename_extractor::FileNameExtractor;
use aide::transform::{TransformOperation, TransformResponse};
use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode, header};
use http::response::Builder;
use log::{debug, info};
use sanitize_filename::sanitize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Specify caching headers for serving files
fn caching_headers(builder: Builder, cache_ttl: usize) -> Builder {
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

/// Filename header, supporting UTF-8 chars
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

/// Validate ProcessingParams
fn validate_processing_params(params: &ProcessingParams) -> Result<(), String> {
    if let Some(quality) = params.quality {
        if quality < 10 || quality > 100 {
            return Err("Quality must be between 10 and 100".to_string());
        }
    }
    Ok(())
}

/// Serve images as static files
///
/// If image is not existing, it will be attempted to fetch on configured base api
pub async fn serve_file(
    Path(image_id): Path<String>,
    query: Query<ProcessingParams>,
    State(state): State<Arc<Config>>,
) -> Result<ImageResponse, ApiError<GetImageErrorType>> {
    // Validate processing parameters
    if let Err(err) = validate_processing_params(&query.0) {
        return Err(responses::api_error(
            StatusCode::BAD_REQUEST,
            err,
            Some(GetImageErrorType::InvalidSize),
        ));
    }

    if !state
        .max_image_resize
        .is_allowed_size(&query.width, &query.height)
    {
        return Err(responses::api_error(
            StatusCode::BAD_REQUEST,
            "Extension too big".to_string(),
            Some(GetImageErrorType::InvalidSize),
        ));
    }

    let image_id = sanitize(image_id);
    info!("Getting img {}", image_id);

    let result = state.processor.get(image_id.clone(), query.0.clone()).await;
    debug!("processed image {}. Generating response", &image_id);

    let response = match result {
        Ok(img) => ImageResponse(
            caching_headers(Response::builder(), state.client_cache_ttl)
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, img.extension.mime_type())
                .header(
                    header::CONTENT_DISPOSITION,
                    content_disposition_header(img.filename.clone(), img.extension),
                )
                .body(Body::from(img.data.as_slice().to_owned()))
                .unwrap(),
        ),
        Err(err) => {
            let status = match err.err_type {
                ProcessingErrorType::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST.into(),
            };
            let error_type = match err.err_type {
                ProcessingErrorType::UnsupportingExtension => {
                    GetImageErrorType::UnsupportingExtension
                }
                ProcessingErrorType::NotFound => GetImageErrorType::NotFound,
                ProcessingErrorType::FileApiError => GetImageErrorType::FileApiError,
                ProcessingErrorType::ProcessedImagesLimit => {
                    GetImageErrorType::ProcessedImagesLimit
                }
            };
            return Err(responses::api_error(status, err.detail, Some(error_type)));
        }
    };

    debug!("generated response");

    Ok(response)
}

/// Pre fetch image into cache to prevent fetching on client image request
#[axum::debug_handler]
pub async fn preload_image(
    Path(image_id): Path<String>,
    State(state): State<Arc<Config>>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<PreloadImageErrorResponse>, ApiError<PreloadImageErrorType>> {
    let image_id = sanitize(image_id);
    info!("Preloading img {}", image_id);

    // Check API key without holding a lock
    let server_api_key = state.api_key.clone();
    let api_key = match headers.get("X-API-Key") {
        None => String::new(),
        Some(header) => header.to_str().unwrap_or("").into(),
    };
    if api_key != server_api_key {
        return Err(responses::api_error(
            StatusCode::UNAUTHORIZED,
            "Mismatched api key".to_string(),
            Some(PreloadImageErrorType::Unauthorized),
        ));
    }

    // Prefetch without holding a lock on the entire config
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(responses::api_error(
                StatusCode::BAD_REQUEST,
                format!("Invalid body: {}", err),
                Some(PreloadImageErrorType::InvalidBody),
            ));
        }
    };

    let result = state
        .processor
        .prefetch(
            image_id.clone(),
            FileNameExtractor::extract(&headers).unwrap_or(image_id.to_string()),
            body_bytes.to_vec(),
        )
        .await;
    if let Err(err) = result {
        let error_type = match err.err_type {
            ProcessingErrorType::UnsupportingExtension => {
                PreloadImageErrorType::UnsupportingExtension
            }
            _ => PreloadImageErrorType::UnsupportingExtension,
        };
        return Err(responses::api_error(
            StatusCode::BAD_REQUEST,
            err.detail,
            Some(error_type),
        ));
    }

    Ok(responses::ok_json::<PreloadImageErrorType>(
        "Ok".to_string(),
    ))
}

pub fn serve_file_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.description("Serve image by id with optional processing parameters.")
        .input::<ImageIdParam>()
        .response_with::<200, ImageResponse, _>(|res: TransformResponse<'_, ()>| {
            res.description("Binary image response.")
        })
        .response_with::<400, Json<GetImageErrorResponse>, _>(
            |res: TransformResponse<'_, GetImageErrorResponse>| {
                res.description("Invalid request or processing error.")
            },
        )
        .response_with::<404, Json<GetImageErrorResponse>, _>(
            |res: TransformResponse<'_, GetImageErrorResponse>| res.description("Image not found."),
        )
}

pub fn preload_image_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.description("Preload image into cache to avoid processing on request.")
        .input::<(ImageIdParam, ApiKeyHeader, BinaryBody)>()
        .response_with::<200, Json<PreloadImageErrorResponse>, _>(
            |res: TransformResponse<'_, PreloadImageErrorResponse>| {
                res.description("Preload request accepted.")
            },
        )
        .response_with::<400, Json<PreloadImageErrorResponse>, _>(
            |res: TransformResponse<'_, PreloadImageErrorResponse>| {
                res.description("Invalid image or payload.")
            },
        )
        .response_with::<401, Json<PreloadImageErrorResponse>, _>(
            |res: TransformResponse<'_, PreloadImageErrorResponse>| {
                res.description("Missing or invalid API key.")
            },
        )
}

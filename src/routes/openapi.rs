use aide::openapi::OpenApi;
use axum::Extension;
use std::sync::Arc;

pub async fn openapi_json(Extension(openapi): Extension<Arc<OpenApi>>) -> axum::response::Response {
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_vec(openapi.as_ref()).unwrap(),
        ))
        .unwrap()
}

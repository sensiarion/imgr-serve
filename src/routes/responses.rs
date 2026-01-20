use crate::routes::errors::ErrorResponse;
use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response as OpenApiResponse};
use axum::Json;
use axum::body::Body;
use axum::response::IntoResponse;
use http::{Response, StatusCode};
use indexmap::IndexMap;
use serde::Serialize;

pub(crate) struct ApiError<T> {
    status: StatusCode,
    detail: String,
    error_type: Option<T>,
}

impl<T: Serialize> IntoResponse for ApiError<T> {
    fn into_response(self) -> axum::response::Response {
        let payload = ErrorResponse {
            detail: self.detail,
            error_type: self.error_type,
        };
        (self.status, Json(payload)).into_response()
    }
}

impl<T> OperationOutput for ApiError<T> {
    type Inner = ();

    fn operation_response(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Option<OpenApiResponse> {
        None
    }

    fn inferred_responses(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Vec<(Option<u16>, OpenApiResponse)> {
        Vec::new()
    }
}

pub(crate) struct ImageResponse(pub Response<Body>);

impl IntoResponse for ImageResponse {
    fn into_response(self) -> axum::response::Response {
        self.0
    }
}

impl OperationOutput for ImageResponse {
    type Inner = ();

    fn operation_response(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Option<OpenApiResponse> {
        Some(OpenApiResponse {
            description: "Binary image response.".to_string(),
            content: IndexMap::from_iter([(
                "image/*".to_string(),
                MediaType {
                    schema: None,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        })
    }

    fn inferred_responses(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Vec<(Option<u16>, OpenApiResponse)> {
        Vec::new()
    }
}

pub fn api_error<T>(status: StatusCode, detail: String, error_type: Option<T>) -> ApiError<T> {
    ApiError {
        status,
        detail,
        error_type,
    }
}

pub fn ok_json<T>(detail: String) -> Json<ErrorResponse<T>> {
    Json(ErrorResponse {
        detail,
        error_type: None,
    })
}

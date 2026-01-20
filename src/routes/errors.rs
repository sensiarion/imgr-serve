use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum GetImageErrorType {
    InvalidSize,
    UnsupportingExtension,
    NotFound,
    FileApiError,
    ProcessedImagesLimit,
}

#[derive(Debug, Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum PreloadImageErrorType {
    InvalidBody,
    Unauthorized,
    UnsupportingExtension,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(bound = "T: JsonSchema")]
#[serde(bound = "T: Serialize")]
pub struct ErrorResponse<T> {
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<T>,
}

pub type GetImageErrorResponse = ErrorResponse<GetImageErrorType>;
pub type PreloadImageErrorResponse = ErrorResponse<PreloadImageErrorType>;

use crate::image_ops::image_types::Extensions;
use serde::{Deserialize, Serialize};
/// it may be uuid, or complex link with path, either will work as simple string
pub type ImageId = String;

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageContainer {
    pub data: Box<Vec<u8>>,
    pub filename: Option<String>,
    pub extension: Extensions,
}

impl ImageContainer {
    pub fn new(data: Box<Vec<u8>>, filename: Option<String>, extension: Extensions) -> Self {
        ImageContainer {
            data,
            filename,
            extension,
        }
    }
}

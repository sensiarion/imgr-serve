use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumString;

pub trait MimeType {
    fn mime_type(&self) -> &str;
}

#[derive(
    Deserialize,
    Serialize,
    JsonSchema,
    Debug,
    PartialEq,
    Hash,
    Eq,
    Copy,
    Clone,
    EnumString,
    Ord,
    PartialOrd,
)]
pub enum Extensions {
    Webp,
    Avif,
    PNG,
}

impl Extensions {
    pub fn name(&self) -> &str {
        match self {
            Extensions::Webp => "webp",
            Extensions::Avif => "avif",
            Extensions::PNG => "png",
        }
    }
}

impl Default for Extensions {
    fn default() -> Self {
        Extensions::Webp
    }
}

impl MimeType for Extensions {
    fn mime_type(&self) -> &str {
        match &self {
            Extensions::Webp => "image/webp",
            Extensions::Avif => "image/avif",
            Extensions::PNG => "image/png",
        }
    }
}

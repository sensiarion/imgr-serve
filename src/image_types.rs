use image::ImageFormat;
use imghdr::Type;
use serde::{Deserialize, Serialize};

pub trait MimeType {
    fn mime_type(&self) -> &str;
}

pub trait IntoImageFormat {
    fn image_format(&self) -> Option<ImageFormat>;
}

impl MimeType for imghdr::Type {
    fn mime_type(&self) -> &str {
        match &self {
            Type::Gif => "image/gif",
            Type::Tiff => "image/tiff",
            Type::Rast => "image/rast",
            Type::Xbm => "image/xbm",
            Type::Jpeg => "image/jpeg",
            Type::Bmp => "image/bmp",
            Type::Png => "image/png",
            Type::Webp => "image/webp",
            Type::Exr => "image/exr",
            Type::Bgp => "image/bgp",
            Type::Pbm => "image/pbm",
            Type::Pgm => "image/pgm",
            Type::Ppm => "image/ppm",
            Type::Rgb => "image/rgb",
            Type::Rgbe => "image/rgbe",
            Type::Flif => "image/flif",
            Type::Ico => "image/ico",
        }
    }
}

impl IntoImageFormat for imghdr::Type {
    fn image_format(&self) -> Option<ImageFormat> {
        match &self {
            Type::Gif => Some(ImageFormat::Gif),
            Type::Tiff => Some(ImageFormat::Tiff),
            Type::Jpeg => Some(ImageFormat::Jpeg),
            Type::Bmp => Some(ImageFormat::Bmp),
            Type::Png => Some(ImageFormat::Png),
            Type::Webp => Some(ImageFormat::WebP),
            Type::Exr => Some(ImageFormat::OpenExr),
            Type::Ico => Some(ImageFormat::Ico),
            _ => None,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Extensions {
    Webp,
}

impl Extensions {
    pub fn name(&self) -> &str {
        match self {
            Extensions::Webp => "webp",
        }
    }
}

impl MimeType for Extensions {
    fn mime_type(&self) -> &str {
        match &self {
            Extensions::Webp => "image/webp",
        }
    }
}
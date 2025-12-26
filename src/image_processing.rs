use crate::image_types::Extensions;
use image::imageops::FilterType;
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Pixel, Rgba};

pub const DEFAULT_COMPRESSION_QUALITY: u32 = 82;

#[derive(serde::Deserialize, PartialEq,Hash,Eq,Clone)]
pub struct ProcessingParams {
    pub width: Option<u32>,
    pub height: Option<u32>,
    // TODO: accept only certain extensions
    pub extension: Option<Extensions>,
    pub quality: Option<u32>,
}

pub fn resize<I: GenericImageView<Pixel = Rgba<u8>>>(
    img: &DynamicImage,
    width: Option<u32>,
    height: Option<u32>,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>> {
    let w = width.unwrap_or(img.width());
    let h = height.unwrap_or(img.height());

    imageops::resize(img, w, h, FilterType::Nearest)
}

pub fn cast_to_extension<I: GenericImageView<Pixel = Rgba<u8>>>(
    img: ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>,
    extension: Extensions,
    quality: Option<u32>,
) -> Vec<u8> {
    let new_width = img.width();
    let new_height = img.height();
    let new_data = img.into_vec();

    match extension {
        Extensions::Webp => {
            let web_encoder =
                webp::Encoder::new(&new_data, webp::PixelLayout::Rgba, new_width, new_height);

            let bytes_img = web_encoder
                .encode(quality.unwrap_or(DEFAULT_COMPRESSION_QUALITY) as f32)
                .as_ref()
                .to_owned();
            bytes_img
        }
    }
}

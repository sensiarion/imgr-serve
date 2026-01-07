use crate::image_types::Extensions;
use fast_image_resize::{Resizer};
use image::{DynamicImage, GenericImageView, ImageBuffer, Pixel, Rgba};

pub const DEFAULT_COMPRESSION_QUALITY: u32 = 82;

/// Behaviour on requesting images with different ratio, then source
#[derive(serde::Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum RatioPolicy {
    /// Just resize with changing ratio and shrinking or etc image
    Resize,
    /// Keep original ratio with cropping to center
    CropToCenter,
}

impl Default for RatioPolicy {
    fn default() -> Self {
        RatioPolicy::CropToCenter
    }
}

#[derive(serde::Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct ProcessingParams {
    pub width: Option<u32>,
    pub height: Option<u32>,
    // TODO: accept only certain extensions
    pub extension: Option<Extensions>,
    pub quality: Option<u32>,
    pub ratio_policy: Option<RatioPolicy>,
}

pub fn resize<I: GenericImageView<Pixel = Rgba<u8>>>(
    img: &DynamicImage,
    width: Option<u32>,
    height: Option<u32>,
    ratio_policy: Option<RatioPolicy>,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>> {
    let w = width.unwrap_or(img.width());
    let h = height.unwrap_or(img.height());

    let ratio_policy = ratio_policy.unwrap_or_default();

    let orig_ratio = img.width() as f64 / img.height() as f64;
    let target_ratio = w as f64 / h as f64;

    let mut resizer = Resizer::new();

    let resulting_image = match ratio_policy {
        RatioPolicy::Resize => {
            // Use fast_image_resize for parallel processing instead of image crate's resize_exact
            let mut dst_img = DynamicImage::new(w, h, img.color());
            let resize_res = resizer.resize(img, &mut dst_img, None);
            if let Err(resize_err) = resize_res {
                panic!("There should be no error on resize, got {}", resize_err)
            };
            dst_img
        }
        RatioPolicy::CropToCenter => {
            // Resize to cover target dimensions while maintaining aspect ratio,
            // then crop to exact target dimensions
            if (orig_ratio - target_ratio).abs() < f64::EPSILON {
                // Same ratio, just resize
                let mut dst_img = DynamicImage::new(w, h, img.color());
                let resize_res = resizer.resize(img, &mut dst_img, None);
                if let Err(resize_err) = resize_res {
                    panic!("There should be no error on resize, got {}", resize_err)
                };
                dst_img
            } else {
                // Different ratios: resize to cover, then crop
                let (resize_w, resize_h) = if orig_ratio > target_ratio {
                    // Original is wider than target
                    // Scale to match target height, width will be larger
                    let new_h = h;
                    let new_w = (h as f64 * orig_ratio).round() as u32;
                    (new_w, new_h)
                } else {
                    // Original is taller than target
                    // Scale to match target width, height will be larger
                    let new_w = w;
                    let new_h = (w as f64 / orig_ratio).round() as u32;
                    (new_w, new_h)
                };

                let mut resized = DynamicImage::new(resize_w, resize_h, img.color());
                // Resize to cover dimensions

                // Calculate crop coordinates (center)
                let offset_x = (resize_w.saturating_sub(w)) / 2;
                let offset_y = (resize_h.saturating_sub(h)) / 2;

                let resize_res = resizer.resize(img, &mut resized, None);
                if let Err(resize_err) = resize_res {
                    panic!("There should be no error on resize, got {}", resize_err)
                };
                resized.crop(offset_x, offset_y, w, h)
            }
        }
    };

    resulting_image.to()
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

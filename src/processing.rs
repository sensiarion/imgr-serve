use crate::image_processing;
use crate::image_processing::{cast_to_extension, ProcessingParams};
use crate::image_types::{Extensions, IntoImageFormat};
use crate::processed_image_cache::ProcessedImagesCache;
use crate::proxying_images::FileApiBackend;
use crate::storage::Storage;
use crate::types::{ImageContainer, ImageId};
use image::{DynamicImage, ImageFormat};
use log::{debug, warn};
use std::sync::Arc;

pub enum ProcessingErrorType {
    UnsupportingExtension,
    NotFound,
    FileApiError,
    // CorruptedCache
}

impl ProcessingErrorType {
    pub fn default_detail(&self) -> String {
        match &self {
            ProcessingErrorType::UnsupportingExtension => {
                "Current image extension is not supported or not an image".to_string()
            }
            ProcessingErrorType::NotFound => "Current image is not found".to_string(),
            ProcessingErrorType::FileApiError => "File not found".to_string(),
        }
    }
}

pub struct ProcessingError {
    pub err_type: ProcessingErrorType,
    pub detail: String,
}

impl ProcessingError {
    fn new(err_type: ProcessingErrorType, detail: Option<String>) -> Self {
        let detail = detail.unwrap_or(err_type.default_detail());
        ProcessingError { err_type, detail }
    }
}

pub struct Processor {
    storage: Arc<tokio::sync::Mutex<dyn Storage + Send + Sync>>,
    cache: Arc<tokio::sync::Mutex<dyn ProcessedImagesCache + Send + Sync>>,
    file_api: Option<Arc<dyn FileApiBackend + Send + Sync>>,
}

impl Processor {
    pub fn new(
        storage: Arc<tokio::sync::Mutex<dyn Storage + Send + Sync>>,
        cache: Arc<tokio::sync::Mutex<dyn ProcessedImagesCache + Send + Sync>>,
        file_api: Option<Arc<dyn FileApiBackend + Send + Sync>>,
    ) -> Self {
        Processor {
            storage,
            cache,
            file_api,
        }
    }

    /// Determine image format, from supporting by formatting lib
    fn get_image_format(&self, data: &Vec<u8>) -> Option<ImageFormat> {
        let img_type = imghdr::from_bytes(data.as_slice());
        if let Some(img_type) = img_type {
            return img_type.image_format();
        }
        None
    }
    fn ensure_correct_extension(&self, data: &Vec<u8>) -> Option<ProcessingError> {
        let img_format = self.get_image_format(data);
        if img_format.is_none() {
            return Some(ProcessingError::new(
                ProcessingErrorType::UnsupportingExtension,
                None,
            ));
        }
        None
    }

    pub async fn get(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Result<ImageContainer, ProcessingError> {
        let cache = self.cache.clone();
        {
            let mut cache_guard = cache.lock().await;
            let cached = cache_guard
                .get(image_id.clone(), params.clone())
                .await
                .cloned();
            if let Some(cached) = cached {
                debug!("Fetched image {} from cache", image_id);
                return Ok(cached);
            }
        }

        let processed_from_storage = {
            let storage = self.storage.clone();
            let mut storage_guard = storage.lock().await;
            let orig_image = storage_guard.get(image_id.clone()).await;
            match orig_image {
                None => None,
                Some(orig_image) => {
                    let img_format = self.get_image_format(&orig_image);
                    match img_format {
                        None => {
                            warn!(
                                "Cache is corrupted for image {}. Fetching from api",
                                image_id.clone()
                            );
                            None
                        }
                        Some(_) => {
                            debug!("Found image {} in storage, start processing", image_id);
                            return self._process_image(image_id, orig_image, params).await;
                        }
                    }
                }
            }
        };
        if let Some(processed_image) = processed_from_storage {
            return processed_image;
        }

        if self.file_api.is_none() {
            debug!("File api disabled. Image {} not found", image_id);
            return Err(ProcessingError::new(ProcessingErrorType::NotFound, None));
        }

        let response = self
            .file_api
            .clone()
            .unwrap()
            .fetch_img_from_base_api(&image_id)
            .await;
        match response {
            Err(err) => {
                if err.http_error_code.unwrap_or(0) == 404 {
                    return Err(ProcessingError::new(
                        ProcessingErrorType::NotFound,
                        Some(err.reason),
                    ));
                }
                Err(ProcessingError::new(
                    ProcessingErrorType::FileApiError,
                    Some(format!(
                        "err: {}; status: {:#?}",
                        err.reason, err.http_error_code
                    )),
                ))
            }
            // TODO: make setting into storage and cache parallel of main execution flow
            Ok(orig_image) => {
                debug!("Fetched from api, start processing image {}", image_id);
                {
                    let storage = self.storage.clone();
                    let mut storage_guard = storage.lock().await;
                    storage_guard.set(image_id.clone(), &orig_image).await;
                }

                self._process_image(image_id, &orig_image, params).await
            }
        }
    }

    /// Fully process image and puts it in all caches (storage + processing cache)
    ///
    /// * `image_id` - should be only the **original** image (cause it's passing into storage cache)
    pub async fn _process_image(
        &mut self,
        image_id: ImageId,
        original_image: &Vec<u8>,
        params: ProcessingParams,
    ) -> Result<ImageContainer, ProcessingError> {
        let img_format = self.get_image_format(original_image);
        if img_format.is_none() {
            return Err(ProcessingError::new(
                ProcessingErrorType::UnsupportingExtension,
                None,
            ));
        }

        let img = image::load_from_memory_with_format(original_image, img_format.unwrap()).unwrap();
        let resized = image_processing::resize::<DynamicImage>(&img, params.width, params.height);
        let extension = params.extension.unwrap_or(Extensions::Webp);
        let result_data =
            cast_to_extension::<DynamicImage>(resized, extension.clone(), params.quality);
        let result = ImageContainer::new(Box::new(result_data.clone()), None, extension);

        {
            let cache = self.cache.clone();
            let mut cache_guard = cache.lock().await;
            cache_guard
                .set(image_id.clone(), params, result.clone())
                .await;
        }

        Ok(result)
    }

    pub async fn prefetch(
        &mut self,
        image_id: ImageId,
        data: Vec<u8>,
    ) -> Result<(), ProcessingError> {
        if let Some(err) = self.ensure_correct_extension(&data) {
            return Err(err);
        }

        let _storage = self.storage.clone();
        let mut storage = _storage.lock().await;

        storage.set(image_id, &data).await;

        Ok(())

        // let storage = self
    }
    //     get with image params (size, ext)
    //       and fallback to storage if not found
    //     prefetch
}

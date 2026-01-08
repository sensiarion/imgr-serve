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
use std::time::Instant;
use tokio::task::spawn_blocking;
use tracing::instrument;

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
    storage: Arc<tokio::sync::RwLock<dyn Storage + Send + Sync>>,
    cache: Arc<tokio::sync::RwLock<dyn ProcessedImagesCache + Send + Sync>>,
    file_api: Option<Arc<dyn FileApiBackend + Send + Sync>>,
}

impl Processor {
    pub fn new(
        storage: Arc<tokio::sync::RwLock<dyn Storage + Send + Sync>>,
        cache: Arc<tokio::sync::RwLock<dyn ProcessedImagesCache + Send + Sync>>,
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

    #[instrument(skip(self), fields(image_id = %image_id))]
    pub async fn get(
        &self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Result<Arc<ImageContainer>, ProcessingError> {
        // Check processed image cache
        let cache_check_start = Instant::now();
        let cache = self.cache.clone();
        let cached = {
            let cache_guard = cache.read().await;
            let lock_wait = cache_check_start.elapsed();
            if lock_wait.as_millis() > 10 {
                debug!("Cache lock wait: {:?} for image {}", lock_wait, image_id);
            }
            cache_guard.get(image_id.clone(), params.clone()).await
        };
        let cache_check_time = cache_check_start.elapsed();
        if cache_check_time.as_millis() > 50 {
            debug!(
                "Cache check took {:?} for image {}",
                cache_check_time, image_id
            );
        }
        if let Some(cached) = cached {
            debug!("Fetched image {} from cache", image_id);
            return Ok(cached);
        }

        // Check storage for original image
        let processed_from_storage = {
            let orig_image = {
                let storage = self.storage.clone();
                let lock_start = Instant::now();
                let storage_guard = storage.read().await;
                let lock_wait = lock_start.elapsed();
                if lock_wait.as_millis() > 10 {
                    debug!("Storage lock wait: {:?} for image {}", lock_wait, image_id);
                }
                storage_guard.get(image_id.clone()).await.clone()
            };
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
                            return self._process_image(image_id, &orig_image, params).await;
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
                    let mut storage_guard = storage.write().await;
                    storage_guard.set(image_id.clone(), &orig_image).await;
                }

                self._process_image(image_id, &orig_image, params).await
            }
        }
    }

    /// Fully process image and puts it in all caches (storage + processing cache)
    ///
    /// * `image_id` - should be only the **original** image (cause it's passing into storage cache)
    #[instrument(skip(self, original_image), fields(image_id = %image_id))]
    pub async fn _process_image(
        &self,
        image_id: ImageId,
        original_image: &Vec<u8>,
        params: ProcessingParams,
    ) -> Result<Arc<ImageContainer>, ProcessingError> {
        let img_format = self.get_image_format(original_image);
        if img_format.is_none() {
            return Err(ProcessingError::new(
                ProcessingErrorType::UnsupportingExtension,
                None,
            ));
        }

        let img = image::load_from_memory_with_format(original_image, img_format.unwrap()).unwrap();

        let params_clone = params.clone();
        let resize_start = Instant::now();
        let result = spawn_blocking(move || {
            let params = params_clone;
            let resize_op_start = Instant::now();
            let resized = image_processing::resize::<DynamicImage>(
                &img,
                params.width,
                params.height,
                params.ratio_policy.clone(),
            );
            let resize_op_time = resize_op_start.elapsed();
            if resize_op_time.as_millis() > 200 {
                debug!("Resize operation took {:?}", resize_op_time);
            }

            let encode_start = Instant::now();
            let extension = params.extension.unwrap_or(Extensions::Webp);
            let result_data =
                cast_to_extension::<DynamicImage>(resized, extension.clone(), params.quality);
            let encode_time = encode_start.elapsed();
            if encode_time.as_millis() > 100 {
                debug!("Encode operation took {:?}ms", encode_time);
            }
            Arc::new(ImageContainer::new(Box::new(result_data), None, extension))
        })
        .await
        .unwrap();
        let resize_total_time = resize_start.elapsed();
        if resize_total_time.as_millis() > 500 {
            debug!(
                "Total resize+encode took {:?} for image {}",
                resize_total_time, image_id
            );
        }

        // Store in cache
        {
            let cache = self.cache.clone();
            let lock_start = Instant::now();
            let mut cache_guard = cache.write().await;
            let lock_wait = lock_start.elapsed();
            if lock_wait.as_millis() > 10 {
                debug!(
                    "Cache lock wait (store): {:?} for image {}",
                    lock_wait, image_id
                );
            }
            cache_guard
                .set(image_id.clone(), params, result.clone())
                .await;
        }

        Ok(result)
    }

    pub async fn prefetch(
        &self,
        image_id: ImageId,
        _filename: String,
        data: Vec<u8>,
    ) -> Result<(), ProcessingError> {
        if let Some(err) = self.ensure_correct_extension(&data) {
            return Err(err);
        }

        let _storage = self.storage.clone();
        let mut storage = _storage.write().await;

        storage.set(image_id, &data).await;

        Ok(())
    }
}

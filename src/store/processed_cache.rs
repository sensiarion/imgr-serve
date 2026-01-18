use crate::config::ImageOptionsOverflowPolicy;
use crate::image_ops::operations::ProcessingParams;
use crate::utils::background::BackgroundService;
use crate::utils::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ProcessingError<'a> {
    pub error: &'a str,
}

/// Cache for processed images with different params
#[async_trait]
pub trait ProcessedImagesCache: BackgroundService {
    async fn get(&self, image_id: ImageId, params: ProcessingParams)
    -> Option<Arc<ImageContainer>>;

    /// Max count of images, stored in the cache per ImageId, this option should be honored in impl
    fn max_options_per_image(&self) -> &NonZeroUsize;

    /// Policy on overflow of max_options_per_image, this option should be honored in impl
    fn max_options_per_image_overflow_policy(&self) -> &ImageOptionsOverflowPolicy;

    /// Insert record in internal storage
    ///
    /// Should also result in increasing `records_count` value
    async fn _insert(
        &self,
        image_id: &ImageId,
        params: &ProcessingParams,
        image: Arc<ImageContainer>,
        pop_last: bool,
    );

    /// Count of processed versions of specified image
    async fn records_count(&self, image_id: &ImageId) -> usize;

    async fn have_record(&self, image_id: &ImageId, params: &ProcessingParams) -> bool;

    /// Lock, using for setting value
    fn set_lock(&self) -> Arc<Mutex<()>>;

    async fn set(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
        image: Arc<ImageContainer>,
    ) -> Result<(), ProcessingError> {
        // without guard, there can be parallel insertions over limit
        let lock = self.set_lock();
        let _guard = lock.lock().await;

        let records_count = self.records_count(&image_id).await;
        match self.have_record(&image_id, &params).await {
            false => {
                if records_count >= self.max_options_per_image().get() {
                    match self.max_options_per_image_overflow_policy() {
                        // overflow case is more like a DOS scenario,
                        // so we neither probit it, or overwrite
                        ImageOptionsOverflowPolicy::Restrict => Err(ProcessingError {
                            error: "Limit exceed. No any new image formats allowed",
                        }),
                        ImageOptionsOverflowPolicy::Rewrite => {
                            // use lru cache internally can be better,
                            // but in DOS scenario there is no actual difference.
                            // If it's attempt to DOS after all usual extension for image is required,
                            // we, at least, keep actual using images in cache that way
                            self._insert(&image_id, &params, image, true).await;
                            Ok(())
                        }
                    }
                } else {
                    self._insert(&image_id, &params, image, false).await;
                    Ok(())
                }
            }
            // key is already there nothing to do.
            // invalidation for now is made via prefetch (fully invalidating all params options)
            // , so we don't have to reset the value
            true => Ok(()),
        }
    }

    /// Flushes all version of specified image id
    async fn remove(&mut self, image_id: ImageId);
}

use crate::image_processing::ProcessingParams;
use crate::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use std::num::NonZeroUsize;
use std::sync::Arc;

/// Cache for processed images with different params
#[async_trait]
pub trait ProcessedImagesCache {
    async fn get(& self, image_id: ImageId, params: ProcessingParams)
    -> Option<Arc<ImageContainer>>;

    async fn set(&mut self, image_id: ImageId, params: ProcessingParams, image: Arc<ImageContainer>);
}

/// Inmemory cache for processed images
pub struct MemoryProcessedImageCache {
    cache: quick_cache::sync::Cache<(ImageId, ProcessingParams), Arc<ImageContainer>>,
}

impl MemoryProcessedImageCache {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(1024).unwrap());

        MemoryProcessedImageCache {
            cache: quick_cache::sync::Cache::new(capacity.into()),
        }
    }
}

#[async_trait]
impl ProcessedImagesCache for MemoryProcessedImageCache {
    async fn get(
        & self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Option<Arc<ImageContainer>> {
        self.cache.get(&(image_id, params))
    }

    async fn set(&mut self, image_id: ImageId, params: ProcessingParams, image: Arc<ImageContainer>) {
        self.cache.insert((image_id, params), image);
    }
}

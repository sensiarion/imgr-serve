use crate::image_processing::ProcessingParams;
use crate::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Cache for processed images with different params
#[async_trait]
pub trait ProcessedImagesCache {
    async fn get(&mut self, image_id: ImageId, params: ProcessingParams)
    -> Option<&ImageContainer>;

    async fn set(&mut self, image_id: ImageId, params: ProcessingParams, image: ImageContainer);
}

/// Inmemory cache for processed images
pub struct MemoryProcessedImageCache {
    cache: LruCache<(ImageId, ProcessingParams), ImageContainer>,
}

impl MemoryProcessedImageCache {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(1024).unwrap());

        MemoryProcessedImageCache {
            cache: LruCache::new(capacity),
        }
    }
}

#[async_trait]
impl ProcessedImagesCache for MemoryProcessedImageCache {
    async fn get(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Option<&ImageContainer> {
        self.cache.get(&(image_id, params))
    }

    async fn set(&mut self, image_id: ImageId, params: ProcessingParams, image: ImageContainer) {
        self.cache.push((image_id, params), image);
    }
}

use crate::image_processing::ProcessingParams;
use crate::types::{BackgroundService, ImageContainer, ImageId};
use async_trait::async_trait;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

/// Cache for processed images with different params
#[async_trait]
pub trait ProcessedImagesCache: BackgroundService {
    async fn get(&self, image_id: ImageId, params: ProcessingParams)
    -> Option<Arc<ImageContainer>>;

    async fn set(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
        image: Arc<ImageContainer>,
    );
}

/// Inmemory cache for processed images
pub struct MemoryProcessedImageCache {
    cache: quick_cache::sync::Cache<(ImageId, ProcessingParams), Arc<ImageContainer>>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
}

impl MemoryProcessedImageCache {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(1024).unwrap());

        MemoryProcessedImageCache {
            cache: quick_cache::sync::Cache::new(capacity.into()),
            cancel_chan: tokio::sync::watch::channel(false),
        }
    }
}

#[async_trait]
impl ProcessedImagesCache for MemoryProcessedImageCache {
    async fn get(
        &self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Option<Arc<ImageContainer>> {
        self.cache.get(&(image_id, params))
    }

    async fn set(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
        image: Arc<ImageContainer>,
    ) {
        self.cache.insert((image_id, params), image);
    }
}

#[async_trait]
impl BackgroundService for MemoryProcessedImageCache {
    fn background_period(&self) -> Duration {
        Duration::new(3600, 0)
    }

    // Current cache impl is auto clearing, so we actually do not need background tasks
    async fn background(&mut self) {}

    fn cancel_token(&self) -> tokio::sync::watch::Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        let _ = self.cancel_chan.0.send(true);
    }
}

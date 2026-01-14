use crate::image_ops::operations::ProcessingParams;
use crate::store::persistent_store::{PersistSpace, PersistentStore};
use crate::utils::background::BackgroundService;
use crate::utils::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use image::EncodableLayout;
use serde::Serialize;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;

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

    /// Flushes all version of specified image id
    async fn remove(&mut self, image_id: ImageId);
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

    async fn remove(&mut self, image_id: ImageId) {
        let matched: Vec<(ImageId, ProcessingParams)> = self
            .cache
            .iter()
            .filter_map(|item| {
                let key = (item).0;
                if key.0 == image_id {
                    return Some(key);
                }
                None
            })
            .collect();

        for key in matched.iter() {
            self.cache.remove(key);
        }
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

/// Inmemory cache for processed images
pub struct PersistentProcessedImageCache {
    store: Arc<PersistentStore>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
}

impl PersistentProcessedImageCache {
    pub fn new(store: Arc<PersistentStore>, _capacity: Option<NonZeroUsize>) -> Self {
        PersistentProcessedImageCache {
            store,
            cancel_chan: tokio::sync::watch::channel(false),
        }
    }
}

#[derive(Serialize, Debug)]
struct CacheKey {
    image_id: ImageId,
    params: ProcessingParams,
}

impl CacheKey {
    fn as_key(&self) -> String {
        format!(
            "{}_{}",
            &self.image_id,
            serde_json::to_string(&self.params).unwrap()
        )
    }
}

#[async_trait]
impl ProcessedImagesCache for PersistentProcessedImageCache {
    async fn get(
        &self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Option<Arc<ImageContainer>> {
        let key = CacheKey { image_id, params }.as_key();

        let v = self.store.get(PersistSpace::Cache, &key).await;

        match v {
            None => None,
            Some(v) => Some(Arc::new(
                postcard::from_bytes::<ImageContainer>(v.as_bytes()).unwrap(),
            )),
        }
    }

    async fn set(
        &mut self,
        image_id: ImageId,
        params: ProcessingParams,
        image: Arc<ImageContainer>,
    ) {
        let key = CacheKey { image_id, params }.as_key();

        self.store
            .set(PersistSpace::Cache, &key, image.as_ref())
            .await;
    }

    async fn remove(&mut self, image_id: ImageId) {
        self.store
            .remove_by_prefix(PersistSpace::Cache, &image_id)
            .await;
    }
}

#[async_trait]
impl BackgroundService for PersistentProcessedImageCache {
    fn background_period(&self) -> Duration {
        Duration::new(60, 0)
    }

    // Persistent cache cleaning up by itself
    async fn background(&mut self) {}

    fn cancel_token(&self) -> Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        let _ = self.cancel_chan.0.send(true);
    }
}

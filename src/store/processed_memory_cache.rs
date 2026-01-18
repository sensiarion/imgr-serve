use crate::config::ImageOptionsOverflowPolicy;
use crate::image_ops::operations::ProcessingParams;
use crate::store::processed_cache::ProcessedImagesCache;
use crate::utils::background::BackgroundService;
use crate::utils::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Inmemory cache for processed images
pub struct MemoryProcessedImageCache {
    cache: quick_cache::sync::Cache<(ImageId, ProcessingParams), Arc<ImageContainer>>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
    max_options_per_image: NonZeroUsize,
    max_options_per_image_overflow_policy: ImageOptionsOverflowPolicy,
    cache_entries: quick_cache::sync::Cache<ImageId, BTreeSet<ProcessingParams>>,
    write_lock: Arc<Mutex<()>>,
}

impl MemoryProcessedImageCache {
    pub fn new(
        capacity: Option<NonZeroUsize>,
        max_options_per_image: NonZeroUsize,
        max_options_per_image_overflow_policy: ImageOptionsOverflowPolicy,
    ) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(1024).unwrap());

        MemoryProcessedImageCache {
            cache: quick_cache::sync::Cache::new(capacity.into()),
            cancel_chan: tokio::sync::watch::channel(false),
            max_options_per_image,
            max_options_per_image_overflow_policy,
            cache_entries: quick_cache::sync::Cache::new(capacity.into()),
            write_lock: Arc::new(Mutex::new(())),
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

    fn max_options_per_image(&self) -> &NonZeroUsize {
        &self.max_options_per_image
    }

    fn max_options_per_image_overflow_policy(&self) -> &ImageOptionsOverflowPolicy {
        &self.max_options_per_image_overflow_policy
    }

    async fn _insert(
        &self,
        image_id: &ImageId,
        params: &ProcessingParams,
        image: Arc<ImageContainer>,
        pop_last: bool,
    ) {
        let mut entries = self
            .cache_entries
            .get(image_id)
            .unwrap_or_else(|| BTreeSet::new());

        if pop_last && entries.len() > 0 {
            let last_param = entries.pop_last().unwrap();
            self.cache.remove(&(image_id.clone(), last_param));
        }
        entries.insert(params.clone());
        self.cache_entries.insert(image_id.clone(), entries);
        self.cache.insert((image_id.clone(), params.clone()), image);
    }

    async fn records_count(&self, image_id: &ImageId) -> usize {
        self.cache_entries
            .get(image_id)
            .unwrap_or_else(|| BTreeSet::new())
            .len()
    }

    async fn have_record(&self, image_id: &ImageId, params: &ProcessingParams) -> bool {
        self.cache.contains_key(&(image_id.clone(), params.clone()))
    }

    fn set_lock(&self) -> Arc<Mutex<()>> {
        self.write_lock.clone()
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
        self.cache_entries.remove(&image_id);
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

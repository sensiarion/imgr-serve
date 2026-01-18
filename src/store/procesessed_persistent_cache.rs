use crate::config::ImageOptionsOverflowPolicy;
use crate::image_ops::operations::ProcessingParams;
use crate::store::persistent_store::{PersistSpace, PersistentStore};
use crate::store::processed_cache::ProcessedImagesCache;
use crate::utils::background::BackgroundService;
use crate::utils::types::{ImageContainer, ImageId};
use async_trait::async_trait;
use image::EncodableLayout;
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::watch::Receiver;

/// Custom key serialization into memory to surely correct work over lsm-tree
///
/// Also, remove method is depends on current structure, so be careful on refactoring
fn cache_key(image_id: &ImageId, params: &ProcessingParams) -> String {
    format!("{}_{}", &image_id, serde_json::to_string(&params).unwrap())
}

/// Inmemory cache for processed images
pub struct PersistentProcessedImageCache {
    store: Arc<PersistentStore>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
    max_options_per_image: NonZeroUsize,
    max_options_per_image_overflow_policy: ImageOptionsOverflowPolicy,
    write_lock: Arc<Mutex<()>>,
}

impl PersistentProcessedImageCache {
    pub fn new(
        store: Arc<PersistentStore>,
        _capacity: Option<NonZeroUsize>,
        max_options_per_image: NonZeroUsize,
        max_options_per_image_overflow_policy: ImageOptionsOverflowPolicy,
    ) -> Self {
        PersistentProcessedImageCache {
            store,
            cancel_chan: tokio::sync::watch::channel(false),
            max_options_per_image,
            max_options_per_image_overflow_policy,
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait]
impl ProcessedImagesCache for PersistentProcessedImageCache {
    async fn get(
        &self,
        image_id: ImageId,
        params: ProcessingParams,
    ) -> Option<Arc<ImageContainer>> {
        let key = cache_key(&image_id, &params);

        let v = self.store.get(PersistSpace::Cache, &key).await;

        match v {
            None => None,
            Some(v) => Some(Arc::new(
                postcard::from_bytes::<ImageContainer>(v.as_bytes()).unwrap(),
            )),
        }
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
        let key = cache_key(&image_id, &params);

        let entries = self.store.get(PersistSpace::CacheEntries, image_id).await;
        let mut entries: BTreeSet<(ImageId, ProcessingParams)> = match entries {
            None => BTreeSet::new(),
            Some(slice) => postcard::from_bytes(slice.as_bytes()).unwrap(),
        };

        if pop_last && entries.len() > 0 {
            let last = entries.pop_last().unwrap();
            let last_key = cache_key(&last.0, &last.1);
            self.store.remove(PersistSpace::Cache, &last_key).await;
        }

        // TODO: prevent postcard parsing unwrap
        self.store
            .set(PersistSpace::Cache, &key, image.as_ref())
            .await;
        entries.insert((image_id.clone(), params.clone()));
        self.store
            .set(PersistSpace::CacheEntries, image_id, &entries)
            .await;
    }

    async fn records_count(&self, image_id: &ImageId) -> usize {
        let entries = self.store.get(PersistSpace::CacheEntries, image_id).await;
        match entries {
            None => 0,
            Some(slice) => {
                postcard::from_bytes::<BTreeSet<(ImageId, ProcessingParams)>>(slice.as_bytes())
                    .unwrap()
                    .len()
            }
        }
    }

    async fn have_record(&self, image_id: &ImageId, params: &ProcessingParams) -> bool {
        let key = cache_key(&image_id, &params);

        // TODO: refacator to entry check to prevent obj load from memory
        self.store.get(PersistSpace::Cache, &key).await.is_some()
    }

    fn set_lock(&self) -> Arc<Mutex<()>> {
        self.write_lock.clone()
    }

    async fn remove(&mut self, image_id: ImageId) {
        // we build key as {image_id}_{params}, where params - json object, so we can rely on
        // structure _{, which is pretty unique to use in prefix removal
        self.store
            .remove_by_prefix(PersistSpace::Cache, &format!("{}_{{", &image_id))
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

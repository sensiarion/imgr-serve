use crate::persistent_store::{PersistSpace, PersistentStore};
use crate::types::{BackgroundService, ImageId};
use async_trait::async_trait;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;

/// Storage to cache original image files, receiving from base api
#[async_trait]
pub trait Storage: BackgroundService {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>>;

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>);

    async fn remove(&mut self, image_id: ImageId);
}

/// Storage implementation with inmemory files caching
pub struct CachingStorage {
    cache: quick_cache::sync::Cache<String, Arc<Vec<u8>>>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
}

impl CachingStorage {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(256).unwrap());

        CachingStorage {
            cache: quick_cache::sync::Cache::new(capacity.into()),
            cancel_chan: tokio::sync::watch::channel(false),
        }
    }
}

#[async_trait]
impl Storage for CachingStorage {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>> {
        self.cache.get(&image_id)
    }

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>) {
        self.cache.insert(image_id, Arc::new(data.clone()));
    }

    async fn remove(&mut self, image_id: ImageId){
        self.cache.remove(&image_id);
    }
}

#[async_trait]
impl BackgroundService for CachingStorage {
    fn background_period(&self) -> Duration {
        Duration::new(3600, 0)
    }

    // Current cache impl is auto clearing, so we actually do not need background tasks
    async fn background(&mut self) {}

    fn cancel_token(&self) -> Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        let _ = self.cancel_chan.0.send(true);
    }
}

/// Storage implementation with disk files caching
pub struct PersistentStorage {
    store: Arc<PersistentStore>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
}

impl PersistentStorage {
    pub fn new(store: Arc<PersistentStore>, capacity: Option<NonZeroUsize>) -> Self {
        PersistentStorage {
            store,
            cancel_chan: tokio::sync::watch::channel(false),
        }
    }
}

#[async_trait]
impl Storage for PersistentStorage {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>> {
        let v = self.store.get(PersistSpace::Storage, &image_id).await;

        match v {
            None => return None,
            Some(v) => Some(Arc::new(v.to_vec())),
        }
    }

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>) {
        self.store.set(PersistSpace::Storage, &image_id, data).await;
    }

    async fn remove(&mut self, image_id: ImageId){
        self.store.remove(PersistSpace::Storage, &image_id).await;
    }
}

#[async_trait]
impl BackgroundService for PersistentStorage {
    fn background_period(&self) -> Duration {
        Duration::new(60, 0)
    }

    // Persistent storage cleaning up by itself
    async fn background(&mut self) {}

    fn cancel_token(&self) -> Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        let _ = self.cancel_chan.0.send(true);
    }
}

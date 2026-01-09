use crate::types::{BackgroundService, ImageId};
use async_trait::async_trait;
use std::num::NonZeroUsize;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{Receiver};

/// Storage to cache original image files, receiving from base api
#[async_trait]
pub trait Storage: BackgroundService {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>>;

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>);
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
}

#[async_trait]
impl BackgroundService for CachingStorage {
    fn background_period(&self) -> Duration {
        Duration::new(10, 0)
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

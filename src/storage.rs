use crate::types::ImageId;
use async_trait::async_trait;
use std::num::NonZeroUsize;

use std::sync::Arc;

/// Storage to cache original image files, receiving from base api

#[async_trait]
pub trait Storage {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>>;

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>);

    async fn background(&mut self);
}

/// Storage implementation with inmemory files caching
pub struct CachingStorage {
    cache: quick_cache::sync::Cache<String, Arc<Vec<u8>>>,
}

impl CachingStorage {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(256).unwrap());

        CachingStorage {
            cache: quick_cache::sync::Cache::new(capacity.into()),
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

    async fn background(&mut self) {
        // self.cache.
    }
}

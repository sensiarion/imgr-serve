use crate::types::{ImageId};
use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Storage to cache original image files, receiving from base api

#[async_trait]
pub trait Storage {
    async fn get(&mut self, image_id: ImageId) -> Option<&Vec<u8>>;

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>);
}

/// Storage implementation with inmemory files caching
pub struct CachingStorage {
    cache: LruCache<String, Vec<u8>>,
}

impl CachingStorage {
    pub fn new(capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(256).unwrap());

        CachingStorage {
            cache: LruCache::new(capacity),
        }
    }
}

#[async_trait]
impl Storage for CachingStorage {
    async fn get(&mut self, image_id: ImageId) -> Option<&Vec<u8>> {
        self.cache.get(&image_id)
    }

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>) {
        self.cache.put(image_id, data.clone());
    }
}

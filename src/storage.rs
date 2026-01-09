use crate::types::{BackgroundService, ImageId};
use async_trait::async_trait;
use fjall::{Keyspace, KeyspaceCreateOptions, PersistMode};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tokio::task::spawn_blocking;
use tracing::debug;

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

const PERSISTENT_STORAGE_KEYSPACE: &str = "storage";
/// Storage implementation with disk files caching
pub struct PersistentStorage {
    db: fjall::Database,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
    keyspace: Keyspace,
}

impl PersistentStorage {
    pub fn new(db_path: Box<Path>, capacity: Option<NonZeroUsize>) -> Self {
        let capacity = capacity.unwrap_or(NonZeroUsize::new(256).unwrap());
        // Db cache is configured by memory size, so assume that each image is about 2mb
        let img_size: u64 = 2048 * 1024;


        let db = fjall::Database::builder(db_path)
            .cache_size(capacity.get() as u64 * img_size)
            .open()
            .unwrap();

        let keyspace = db
            .keyspace(
                PERSISTENT_STORAGE_KEYSPACE,
                KeyspaceCreateOptions::default,
            )
            .unwrap();
        PersistentStorage {
            db,
            cancel_chan: tokio::sync::watch::channel(false),
            keyspace,
        }
    }
}

#[async_trait]
impl Storage for PersistentStorage {
    async fn get(&self, image_id: ImageId) -> Option<Arc<Vec<u8>>> {
        let v = self.keyspace.get(image_id.as_str()).ok().unwrap();

        match v {
            None => return None,
            Some(v) => Some(Arc::new(v.to_vec())),
        }
    }

    async fn set(&mut self, image_id: ImageId, data: &Vec<u8>) {
        let _ = self.keyspace.insert(image_id, data);
    }
}

#[async_trait]
impl BackgroundService for PersistentStorage {
    fn background_period(&self) -> Duration {
        Duration::new(60, 0)
    }

    // Current cache impl is auto clearing, so we actually do not need background tasks
    async fn background(&mut self) {
        let db = self.db.clone();
        spawn_blocking(move || {
            db.persist(PersistMode::SyncData).unwrap();
            debug!("flush PersistentStorage to disk");
        })
        .await
        .unwrap();
    }

    fn cancel_token(&self) -> Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        let _ = self.cancel_chan.0.send(true);
    }
}

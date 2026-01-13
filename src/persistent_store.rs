
use crate::types::BackgroundService;
use async_trait::async_trait;
use fjall::{Keyspace, KeyspaceCreateOptions, PersistMode, Slice};
use log::{debug, warn};
use postcard::to_stdvec;
use serde::Serialize;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use strum::{Display, EnumString};
use tokio::sync::watch::Receiver;
use tokio::task::spawn_blocking;

#[derive(Debug, EnumString, Display)]
pub enum PersistSpace {
    Storage,
    Cache,
}

const PERSISTENT_STORAGE_KEYSPACE: &str = "storage";
const PERSISTENT_CACHE_KEYSPACE: &str = "cache";

pub struct PersistentStore {
    db: fjall::Database,
    store_keyspace: Keyspace,
    cache_keyspace: Keyspace,
}

/// Expecting source image is about 2mb size
const SOURCE_IMAGE_SIZE: u64 = 2048 * 1024;

/// Expecting resized image is about 2mb size
const RESIZED_IMAGE_SIZE: u64 = 64 * 1024;

// TODO: rename (conflicts  with storage)
impl PersistentStore {
    pub fn new(
        db_path: Box<Path>,
        storage_capacity: NonZeroUsize,
        cache_capacity: NonZeroUsize,
    ) -> Self {
        let storage_size = SOURCE_IMAGE_SIZE * storage_capacity.get() as u64;
        let resized_size = RESIZED_IMAGE_SIZE * cache_capacity.get() as u64;
        let db_cache_size = storage_size + resized_size;

        let db = fjall::Database::builder(db_path)
            .cache_size(db_cache_size)
            .open()
            .unwrap();

        let store_keyspace = db
            .keyspace(PERSISTENT_STORAGE_KEYSPACE, KeyspaceCreateOptions::default)
            .unwrap();
        let cache_keyspace = db
            .keyspace(PERSISTENT_CACHE_KEYSPACE, KeyspaceCreateOptions::default)
            .unwrap();

        PersistentStore {
            db,
            store_keyspace,
            cache_keyspace,
        }
    }

    fn keyspace(&self, space: PersistSpace) -> Keyspace {
        match space {
            PersistSpace::Storage => self.store_keyspace.clone(),
            PersistSpace::Cache => self.cache_keyspace.clone(),
        }
    }
    pub async fn get<K>(&self, space: PersistSpace, key: &K) -> Option<Slice>
    where
        K: Serialize + Send + Sync + 'static,
    {
        let keyspace = self.keyspace(space);

        let key = to_stdvec(&key).unwrap();

        spawn_blocking(move || keyspace.get(key).unwrap())
            .await
            .unwrap()
    }

    pub async fn set<K, V>(&self, space: PersistSpace, key: &K, value: &V)
    where
        K: Serialize + Send + Sync + 'static,
        V: Serialize + Send + Sync + 'static,
    {
        let keyspace = self.keyspace(space);

        let key = to_stdvec(&key).unwrap();
        let value = to_stdvec(&value).unwrap();

        spawn_blocking(move || keyspace.insert(key, value).unwrap())
            .await
            .unwrap();
    }

    pub async fn remove_by_prefix<K>(&self, space: PersistSpace, prefix: &K)
    where
        K: Serialize + Send + Sync + 'static,
    {
        let keyspace = self.keyspace(space);

        let key = to_stdvec(&prefix).unwrap();

        spawn_blocking(move || {
            for key in keyspace.prefix(key) {
                let _ = keyspace.remove(key.key().unwrap());
            }
        })
        .await
        .unwrap();
    }

    pub async fn remove<K>(&self, space: PersistSpace, key: &K)
    where
        K: Serialize + Send + Sync + 'static,
    {
        let keyspace = self.keyspace(space);

        let key = to_stdvec(&key).unwrap();

        let _ = keyspace.remove(key);
    }
}

pub struct StorageBackgroundAdapter {
    store: Option<Arc<PersistentStore>>,
    cancel_chan: (
        tokio::sync::watch::Sender<bool>,
        tokio::sync::watch::Receiver<bool>,
    ),
}

impl StorageBackgroundAdapter {
    pub fn new(store: Option<Arc<PersistentStore>>) -> Self {
        StorageBackgroundAdapter {
            store,
            cancel_chan: tokio::sync::watch::channel(false),
        }
    }
}

#[async_trait]
impl BackgroundService for StorageBackgroundAdapter {
    fn background_period(&self) -> Duration {
        Duration::new(60, 0)
    }

    async fn background(&mut self) {
        if self.store.is_none() {
            return;
        }
        debug!("Flushing images to disk");
        let store = self.store.clone();
        let err = store.unwrap().db.persist(PersistMode::SyncAll);
        if let Err(err) = err {
            warn!(
                "Failed to flush data to disk, got error: {}",
                err.to_string()
            )
        }
    }

    fn cancel_token(&self) -> Receiver<bool> {
        self.cancel_chan.1.clone()
    }

    async fn stop(&mut self) {
        if self.store.is_none() {
            return;
        }
        debug!("Flushing images to disk");
        let store = self.store.clone();
        let err = store.unwrap().db.persist(PersistMode::SyncAll);
        if let Err(err) = err {
            warn!(
                "Failed to flush data to disk, got error: {}",
                err.to_string()
            )
        }
    }
}

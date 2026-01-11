use fjall::{Keyspace, KeyspaceCreateOptions, Slice};
use postcard::to_stdvec;
use serde::{Serialize};
use std::num::NonZeroUsize;
use std::path::Path;
use strum::{Display, EnumString};
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

}

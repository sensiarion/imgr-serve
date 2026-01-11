use crate::persistent_store::PersistentStore;
use crate::processed_image_cache::{
    MemoryProcessedImageCache, PersistentProcessedImageCache, ProcessedImagesCache,
};
use crate::processing::Processor;
use crate::proxying_images::{FileApiBackend, SimpleFileApiBackend};
use crate::storage::{CachingStorage, PersistentStorage, Storage};
use envconfig;
use envconfig::Envconfig;
use log::info;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use strum::{Display, EnumString};

#[derive(Clone, EnumString, Display, Eq, PartialEq)]
pub enum StorageImplementation {
    InMemory,
    Persistent,
}

#[derive(Clone, EnumString, Display, Eq, PartialEq)]
pub enum ProcessingCacheImplementation {
    InMemory,
    Persistent,
}

// TODO add prefixes before release
#[derive(Envconfig)]
struct EnvConfig {
    #[envconfig(from = "HOST", default = "0.0.0.0")]
    pub host: String,
    #[envconfig(from = "PORT", default = "3021")]
    pub port: u32,

    #[envconfig(from = "BASE_FILE_API_URL")]
    base_file_api_url: Option<String>,
    #[envconfig(from = "BASE_FILE_API_URL_TIMEOUT", default = "30")]
    base_file_api_timeout: u32,

    #[envconfig(from = "API_KEY", default = "")]
    pub api_key: String,

    #[envconfig(from = "STORAGE_IMPLEMENTATION", default = "InMemory")]
    pub storage_implementation: StorageImplementation,

    #[envconfig(from = "PROCESSING_CACHE_IMPLEMENTATION", default = "InMemory")]
    pub processing_cache_implementation: ProcessingCacheImplementation,

    /// Count of original images cached in memory
    #[envconfig(from = "STORAGE_CACHE_SIZE", default = "256")]
    pub storage_cache_size: usize,

    /// Count of processed images (after resize, crop and etc) stored in memory
    #[envconfig(from = "PROCESSING_CACHE_SIZE", default = "1024")]
    pub processing_cache_size: usize,

    /// Persistent db location (directory) for both processing and storage cache
    #[envconfig(from = "PERSISTENT_STORAGE_DIR", default = ".imgr-serve")]
    pub persistent_storage_dir: String,
}

pub struct Config {
    pub host: String,
    pub port: u32,
    pub api_key: String,
    pub processor: Processor,
}

impl Config {
    pub fn from_env() -> Config {
        let env_conf = EnvConfig::init_from_env().unwrap();
        let base_file_api = match env_conf.base_file_api_url {
            None => None,
            Some(url) => Some(Arc::new(SimpleFileApiBackend::new(
                url,
                Some(env_conf.base_file_api_timeout),
            )) as Arc<dyn FileApiBackend + Send + Sync>),
        };

        let storage_size = NonZeroUsize::new(env_conf.storage_cache_size)
            .unwrap_or(NonZeroUsize::new(256).unwrap());
        let cache_size = NonZeroUsize::new(env_conf.processing_cache_size)
            .unwrap_or(NonZeroUsize::new(1024).unwrap());
        let need_persist_store = env_conf.storage_implementation
            == StorageImplementation::Persistent
            || env_conf.processing_cache_implementation
                == ProcessingCacheImplementation::Persistent;
        let persistent_store = match need_persist_store {
            true => Some(Arc::new(PersistentStore::new(
                Box::from(Path::new(env_conf.persistent_storage_dir.as_str())),
                {
                    if env_conf.storage_implementation == StorageImplementation::Persistent {
                        storage_size
                    } else {
                        NonZeroUsize::new(1).unwrap()
                    }
                },
                {
                    if env_conf.processing_cache_implementation
                        == ProcessingCacheImplementation::Persistent
                    {
                        cache_size
                    } else {
                        NonZeroUsize::new(1).unwrap()
                    }
                },
            ))),
            false => None,
        };

        info!("Using {} storage", env_conf.storage_implementation);
        let storage: Arc<tokio::sync::RwLock<dyn Storage + Send + Sync>> = match env_conf
            .storage_implementation
        {
            StorageImplementation::InMemory => Arc::new(tokio::sync::RwLock::with_max_readers(
                CachingStorage::new(Some(storage_size)),
                1024,
            )),
            StorageImplementation::Persistent => Arc::new(tokio::sync::RwLock::with_max_readers(
                PersistentStorage::new(persistent_store.clone().unwrap(), Some(storage_size)),
                1024,
            )),
        };

        info!(
            "Using {} processing cache",
            env_conf.processing_cache_implementation
        );
        let cache: Arc<tokio::sync::RwLock<dyn ProcessedImagesCache + Send + Sync>> =
            match env_conf.processing_cache_implementation {
                ProcessingCacheImplementation::InMemory => {
                    Arc::new(tokio::sync::RwLock::with_max_readers(
                        MemoryProcessedImageCache::new(Some(storage_size)),
                        1024,
                    ))
                }
                ProcessingCacheImplementation::Persistent => {
                    Arc::new(tokio::sync::RwLock::with_max_readers(
                        PersistentProcessedImageCache::new(
                            persistent_store.unwrap(),
                            Some(storage_size),
                        ),
                        1024,
                    ))
                }
            };

        let processor = Processor::new(storage, cache, base_file_api);

        Config {
            host: env_conf.host,
            port: env_conf.port,
            api_key: env_conf.api_key,
            processor,
        }
    }
}

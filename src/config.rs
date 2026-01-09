use crate::processed_image_cache::MemoryProcessedImageCache;
use crate::processing::Processor;
use crate::proxying_images::{FileApiBackend, SimpleFileApiBackend};
use crate::storage::{CachingStorage, PersistentStorage, Storage};
use envconfig;
use envconfig::Envconfig;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use log::info;
use strum::{Display, EnumString};

#[derive(Clone, EnumString,Display)]
pub enum StorageImplementation {
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

    /// Count of original images cached in memory
    #[envconfig(from = "STORAGE_CACHE_SIZE", default = "256")]
    pub storage_cache_size: usize,

    /// Count of processed images (after resize, crop and etc) stored in memory
    #[envconfig(from = "PROCESSING_CACHE_SIZE", default = "1024")]
    pub processing_cache_size: usize,
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

        // TODO specify cache size via env
        // TODO choose storage/cache backends via env

        // TODO single persistentdb for storage/cache
        // TODO db path in env

        let storage_size = NonZeroUsize::new(env_conf.storage_cache_size)
            .unwrap_or(NonZeroUsize::new(256).unwrap());

        info!("Using {} storage",env_conf.storage_implementation);
        let storage: Arc<tokio::sync::RwLock<dyn Storage + Send + Sync>> =
                match env_conf.storage_implementation {
                    StorageImplementation::InMemory => {

                        Arc::new(tokio::sync::RwLock::with_max_readers(
                            CachingStorage::new(Some(storage_size)),
                            1024,
                        ))
                    }
                    StorageImplementation::Persistent => {
                        Arc::new(tokio::sync::RwLock::with_max_readers(
                            PersistentStorage::new(Box::from(Path::new("./db")), Some(storage_size)),
                            1024,
                        ))
                    }
                };

        let cache = MemoryProcessedImageCache::new(Some(
            NonZeroUsize::new(env_conf.processing_cache_size)
                .unwrap_or(NonZeroUsize::new(1024).unwrap()),
        ));

        let processor = Processor::new(
            storage,
            Arc::new(tokio::sync::RwLock::with_max_readers(cache, 1024)),
            base_file_api,
        );

        Config {
            host: env_conf.host,
            port: env_conf.port,
            api_key: env_conf.api_key,
            processor,
        }
    }
}

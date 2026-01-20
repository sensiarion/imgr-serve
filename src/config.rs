use crate::image_ops::image_types::Extensions;
use crate::image_ops::processing::Processor;
use crate::proxying_images::{FileApiBackend, SimpleFileApiBackend};
use crate::store::persistent_store::PersistentStore;
use crate::store::procesessed_persistent_cache::PersistentProcessedImageCache;
use crate::store::processed_cache::ProcessedImagesCache;
use crate::store::processed_memory_cache::MemoryProcessedImageCache;
use crate::store::source_image_storage::{CachingStorage, OriginalImageStorage, PersistentStorage};
use envconfig;
use envconfig::Envconfig;
use log::info;
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use strum::EnumString;

#[derive(Clone, EnumString, strum::Display, Eq, PartialEq)]
pub enum StorageImplementation {
    InMemory,
    Persistent,
}

#[derive(Clone, EnumString, strum::Display, Eq, PartialEq)]
pub enum ProcessingCacheImplementation {
    InMemory,
    Persistent,
}

#[derive(Clone, EnumString, strum::Display, Eq, PartialEq)]
pub enum ImageOptionsOverflowPolicy {
    Restrict,
    Rewrite,
}

pub struct Size {
    width: u32,
    height: u32,
}

impl Size {
    pub fn is_allowed_size(&self, width: &Option<u32>, height: &Option<u32>) -> bool {
        if let Some(width) = width
            && *width > self.width
        {
            return false;
        }
        if let Some(height) = height
            && *height > self.height
        {
            return false;
        }
        true
    }
}

pub struct ParseSizeError {
    #[allow(dead_code)]
    msg: String,
}

impl FromStr for Size {
    type Err = ParseSizeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let elems: Vec<&str> = s.split(',').collect();
        if elems.len() != 2 {
            return Err(ParseSizeError {
                msg: format!("Expected size \"width, height\", got {}", s),
            });
        }
        let sizes: Vec<u32> = elems
            .iter()
            .map_while(|el| el.parse::<u32>().ok())
            .collect();

        if sizes.len() != 2 {
            return Err(ParseSizeError {
                msg: format!("Expected size \"width, height\", got {}", s),
            });
        }

        Ok(Size {
            width: sizes.get(0).unwrap().clone(),
            height: sizes.get(1).unwrap().clone(),
        })
    }
}

#[derive(Envconfig)]
struct EnvConfig {
    #[envconfig(from = "HOST", default = "0.0.0.0")]
    pub host: String,
    #[envconfig(from = "PORT", default = "3021")]
    pub port: u32,

    // ------------------
    // Fetching from base api and prefetching
    #[envconfig(from = "BASE_FILE_API_URL")]
    base_file_api_url: Option<String>,
    #[envconfig(from = "BASE_FILE_API_URL_TIMEOUT", default = "30")]
    base_file_api_timeout: u32,
    #[envconfig(from = "API_KEY", default = "")]
    pub api_key: String,

    // ------------------
    // Caching settings settings
    #[envconfig(from = "STORAGE_IMPLEMENTATION", default = "InMemory")]
    pub storage_implementation: StorageImplementation,
    #[envconfig(from = "PROCESSING_CACHE_IMPLEMENTATION", default = "InMemory")]
    pub processing_cache_implementation: ProcessingCacheImplementation,
    /// Count of original images cached in memory
    #[envconfig(from = "STORAGE_CACHE_SIZE", default = "256")]
    pub storage_cache_size: NonZeroUsize,
    /// Count of processed images (after resize, crop and etc) stored in memory
    #[envconfig(from = "PROCESSING_CACHE_SIZE", default = "1024")]
    pub processing_cache_size: NonZeroUsize,
    /// Persistent db location (directory) for both processing and storage cache
    #[envconfig(from = "PERSISTENT_STORAGE_DIR", default = ".imgr-serve")]
    pub persistent_storage_dir: String,

    // ------------------
    // Processing settings
    /// Client cache (in browser) duration (in seconds) for served images
    #[envconfig(from = "CLIENT_CACHE_TTL", default = "31536000")]
    pub client_cache_ttl: usize,
    /// Max image resulting size after resize (width,height)
    #[envconfig(from = "MAX_IMAGE_RESIZE", default = "1920,1080")]
    pub max_image_resize: Size,

    /// Default resulting extension
    #[envconfig(from = "DEFAULT_EXTENSION", default = "Webp")]
    pub default_extension: Extensions,
    /// Allow custom extensions (if false, only DEFAULT_EXTENSION will be returned)
    #[envconfig(from = "ALLOW_CUSTOM_EXTENSION", default = "true")]
    pub allow_custom_extension: bool,

    /// Restrict max options (size, extensions and etc) per image
    /// This option prevents poisoning processing cache with insufficient options
    #[envconfig(from = "MAX_OPTIONS_PER_IMAGE", default = "32")]
    pub max_options_per_image: NonZeroUsize,
    /// Behaviour on exceeding limit of MAX_OPTIONS_PER_IMAGE.
    /// by default it will be rewrite it as LRU cache
    #[envconfig(from = "MAX_OPTIONS_PER_IMAGE_OVERFLOW_POLICY", default = "Rewrite")]
    pub max_options_per_image_overflow_policy: ImageOptionsOverflowPolicy,

    /// Enable OpenAPI and Swagger docs routes
    #[envconfig(from = "ENABLE_DOCS", default = "true")]
    pub enable_docs: bool,
}

pub struct Config {
    pub host: String,
    pub port: u32,
    pub api_key: String,
    pub processor: Processor,

    pub client_cache_ttl: usize,
    pub max_image_resize: Size,
    pub enable_docs: bool,
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

        let storage_size = env_conf.storage_cache_size;
        let cache_size = env_conf.processing_cache_size;
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
        let storage: Arc<tokio::sync::RwLock<dyn OriginalImageStorage + Send + Sync>> =
            match env_conf.storage_implementation {
                StorageImplementation::InMemory => Arc::new(tokio::sync::RwLock::with_max_readers(
                    CachingStorage::new(Some(storage_size)),
                    1024,
                )),
                StorageImplementation::Persistent => {
                    Arc::new(tokio::sync::RwLock::with_max_readers(
                        PersistentStorage::new(
                            persistent_store.clone().unwrap(),
                            Some(storage_size),
                        ),
                        1024,
                    ))
                }
            };

        info!(
            "Using {} processing cache",
            env_conf.processing_cache_implementation
        );
        let cache: Arc<tokio::sync::RwLock<dyn ProcessedImagesCache + Send + Sync>> =
            match env_conf.processing_cache_implementation {
                ProcessingCacheImplementation::InMemory => {
                    Arc::new(tokio::sync::RwLock::with_max_readers(
                        MemoryProcessedImageCache::new(
                            Some(storage_size),
                            env_conf.max_options_per_image.clone(),
                            env_conf.max_options_per_image_overflow_policy.clone(),
                        ),
                        1024,
                    ))
                }
                ProcessingCacheImplementation::Persistent => {
                    Arc::new(tokio::sync::RwLock::with_max_readers(
                        PersistentProcessedImageCache::new(
                            persistent_store.clone().unwrap(),
                            Some(storage_size),
                            env_conf.max_options_per_image.clone(),
                            env_conf.max_options_per_image_overflow_policy.clone(),
                        ),
                        1024,
                    ))
                }
            };

        let processor = Processor::new(
            storage,
            cache,
            base_file_api,
            persistent_store,
            env_conf.default_extension,
            env_conf.allow_custom_extension,
        );

        Config {
            host: env_conf.host,
            port: env_conf.port,
            api_key: env_conf.api_key,
            processor,
            client_cache_ttl: env_conf.client_cache_ttl,
            max_image_resize: env_conf.max_image_resize,
            enable_docs: env_conf.enable_docs,
        }
    }
}

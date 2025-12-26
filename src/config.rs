use crate::processed_image_cache::MemoryProcessedImageCache;
use crate::processing::Processor;
use crate::proxying_images::{FileApiBackend, SimpleFileApiBackend};
use crate::storage::CachingStorage;
use envconfig;
use envconfig::Envconfig;
use std::sync::{Arc};

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

        let storage = CachingStorage::new(None);
        let cache = MemoryProcessedImageCache::new(None);

        let processor = Processor::new(
            Arc::new(tokio::sync::Mutex::new(storage)),
            Arc::new(tokio::sync::Mutex::new(cache)),
            base_file_api
        );

        Config {
            host: env_conf.host,
            port: env_conf.port,
            api_key: env_conf.api_key,
            processor,
        }
    }
}

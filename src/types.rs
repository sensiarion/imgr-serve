use crate::image_types::Extensions;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tokio::task::JoinSet;

/// it may be uuid, or complex link with path, either will work as simple string
pub type ImageId = String;

#[derive(Clone)]
pub struct ImageContainer {
    pub data: Box<Vec<u8>>,
    pub filename: Option<String>,
    pub extension: Extensions,
}

impl ImageContainer {
    pub fn new(data: Box<Vec<u8>>, filename: Option<String>, extension: Extensions) -> Self {
        ImageContainer {
            data,
            filename,
            extension,
        }
    }
}

/// Trait defining scheduling and running of background tasks for storage/cache
#[async_trait]
pub trait BackgroundService {
    /// Defines period of running background task
    fn background_period(&self) -> Duration;

    /// Background task for storage
    async fn background(&mut self);

    fn cancel_token(&self) -> watch::Receiver<bool>;

    async fn stop(&mut self) {}
}

pub async fn serve_background(
    services: Vec<Arc<RwLock<dyn BackgroundService + Send + Sync>>>,
) -> JoinSet<()> {
    let mut futures = JoinSet::new();

    for s in services.iter() {
        let service = s.clone();
        futures.spawn(async move {
            let guard = service.read().await;
            let interval = guard.background_period();
            let mut rx = guard.cancel_token();

            drop(guard);
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) =>{
                        let mut guard = service.write().await;
                        guard.background().await;
                    }
                    _ = rx.changed() => {
                        if *rx.borrow(){
                            break;
                        }
                    }
                }
            }
        });
    }

    futures
}

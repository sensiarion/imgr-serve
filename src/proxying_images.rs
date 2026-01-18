/// Fetching images from original files API
use crate::utils::types::ImageId;
use async_trait::async_trait;
use log::debug;
use reqwest::{Client, StatusCode};
use serde::Serialize;
use std::time::Duration;

/// Error while fetching files from base api
#[derive(Debug, Serialize)]
pub struct FileApiError {
    pub reason: String,
    pub http_error_code: Option<u32>,
}

impl FileApiError {
    fn new(reason: String, http_error_code: Option<u32>) -> Self {
        FileApiError {
            reason,
            http_error_code,
        }
    }
}

#[async_trait]
pub trait FileApiBackend {
    /// Requesting file from original file api if it not found in cache
    async fn fetch_img_from_base_api(&self, image_id: &ImageId) -> Result<Vec<u8>, FileApiError>;
}

pub struct SimpleFileApiBackend {
    base_api_url: String,
    client: Client,
}

impl SimpleFileApiBackend {
    pub fn new(base_api_url: String, timeout: Option<u32>) -> Self {
        let timeout = Duration::from_secs(timeout.unwrap_or(30) as u64);
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout / 3)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to create base api url client");

        SimpleFileApiBackend {
            base_api_url: base_api_url.trim_end_matches("/").into(),
            client,
        }
    }
}

#[async_trait]
impl FileApiBackend for SimpleFileApiBackend {
    async fn fetch_img_from_base_api(&self, image_id: &ImageId) -> Result<Vec<u8>, FileApiError> {
        let resp = self
            .client
            .get(format!("{}/{}", self.base_api_url, image_id))
            .send()
            .await;
        if resp.is_err() {
            let err = resp.err().unwrap();
            debug!(
                "Got http error while trying to fetch image from file api: {}. Err: {}",
                image_id, err
            );
            return Err(FileApiError::new(
                "Failed to request image from base api".to_string(),
                None,
            ));
        }
        let resp = resp.unwrap();
        let status = resp.status();
        if status != StatusCode::OK {
            debug!(
                "Got http error from file api status={},resp={}",
                status,
                resp.text()
                    .await
                    .unwrap_or("unable to get response".into())
                    .chars()
                    .take(100)
                    .collect::<String>()
            );
            return Err(FileApiError::new(
                "Got error from file api".to_string(),
                Some(status.as_u16().into()),
            ));
        }

        Ok(resp.bytes().await.unwrap().to_vec())
    }
}

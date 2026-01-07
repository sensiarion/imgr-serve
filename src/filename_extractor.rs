use axum::http::HeaderMap;
use percent_encoding::percent_decode_str;
use sanitize_filename::sanitize;

pub struct FileNameExtractor;

impl FileNameExtractor {
    /// Extract and sanitize filename from headers
    pub fn extract(headers: &HeaderMap) -> Option<String> {
        // Extract raw filename
        let raw_filename = Self::_extract(headers);
        if raw_filename.is_none() {
            return None;
        }

        // Sanitize the filename
        let sanitized = sanitize(&raw_filename.unwrap());

        Some(sanitized)
    }

    /// Extract raw filename from headers
    fn _extract(headers: &HeaderMap) -> Option<String> {
        // Try Content-Disposition first
        if let Some(filename) = Self::from_content_disposition(headers) {
            return Some(filename);
        }

        None
    }

    // ... (keep the other extraction methods from previous example)
    fn from_content_disposition(headers: &HeaderMap) -> Option<String> {
        headers
            .get("Content-Disposition")
            .or_else(|| headers.get("content-disposition"))
            .and_then(|header| header.to_str().ok())
            .and_then(Self::parse_content_disposition)
    }

    fn parse_content_disposition(disposition: &str) -> Option<String> {
        let parts: Vec<&str> = disposition.split(';').map(str::trim).collect();

        for part in parts {
            if let Some(filename) = Self::extract_filename_from_part(part) {
                return Some(filename);
            }
        }

        None
    }

    fn extract_filename_from_part(part: &str) -> Option<String> {
        if part.starts_with("filename=") {
            return Self::extract_quoted_value(&part[9..]);
        } else if part.starts_with("filename*=") {
            return Self::extract_rfc5987_value(&part[10..]);
        }
        None
    }

    fn extract_quoted_value(value: &str) -> Option<String> {
        let value = value.trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() > 1 {
            Some(value[1..value.len() - 1].to_string())
        } else if value.starts_with('\'') && value.ends_with('\'') && value.len() > 1 {
            Some(value[1..value.len() - 1].to_string())
        } else {
            Some(value.to_string())
        }
    }

    fn extract_rfc5987_value(value: &str) -> Option<String> {
        let value = value.trim();

        if value.to_uppercase().starts_with("UTF-8''") {
            let encoded = &value[7..];
            match percent_decode_str(encoded).decode_utf8() {
                Ok(decoded) => Some(decoded.to_string()),
                Err(_) => Some(encoded.to_string()),
            }
        } else {
            Self::extract_quoted_value(value)
        }
    }
}

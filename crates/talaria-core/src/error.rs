use crate::models::ApiError;
use reqwest::StatusCode;
use std::fmt;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for the Hermes client.
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("missing API key for authenticated endpoint {endpoint}")]
    MissingApiKey { endpoint: String },
    #[error("supabase configuration missing: {0}")]
    MissingSupabaseConfig(String),
    #[error("supabase upload failed: {status} {message}")]
    SupabaseUpload { status: StatusCode, message: String },
    #[error("supabase db request failed: {status} {message}")]
    SupabaseDb { status: StatusCode, message: String },
    #[error("camera unavailable: {0}")]
    CameraUnavailable(String),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error {status}: {message}")]
    Api {
        status: StatusCode,
        message: String,
        api_error: Option<Box<ApiError>>,
        request_id: Option<String>,
    },
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Error {
    pub fn from_api(
        status: StatusCode,
        api_error: Option<ApiError>,
        body: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        let message = api_error
            .as_ref()
            .map(|e| e.error.clone())
            .or(body)
            .unwrap_or_else(|| "unknown API error".to_string());

        Error::Api {
            status,
            message,
            api_error: api_error.map(Box::new),
            request_id,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

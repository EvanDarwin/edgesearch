pub use crate::responses::*;

pub mod http;
pub mod query;
pub mod responses;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Reqwest error: {0}")]
    Reqwest(reqwest::Error),
    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("API error: {0}")]
    Api(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;

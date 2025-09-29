pub mod documents;
pub mod index;
pub mod indexes;
pub mod keywords;
pub mod search;

#[derive(serde::Serialize)]
pub struct StatusResponse {
    pub ready: bool,
}

#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl Into<String> for ErrorResponse {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap_or_else(|_| "{\"error\":\"internal error\"}".into())
    }
}

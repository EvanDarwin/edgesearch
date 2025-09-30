use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDocument {
    pub index: String,
    pub docs_count: u32,
    pub version: u8,
    pub created: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedResponse {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "id")]
    pub uuid: String,
    #[serde(skip)]
    pub index: String,
    #[serde(rename = "rev")]
    pub revision: u32,
    #[serde(rename = "lang")]
    pub lang: Option<String>,
    #[serde(rename = "body")]
    pub document_body: Option<String>,
    #[serde(rename = "keywords")]
    pub keywords: Option<Vec<(String, f64)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentResponse {
    pub updated: bool,
    pub scores: Vec<(String, f64)>,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub document_count: u32,
    pub matches: Vec<SearchResultRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultRow {
    pub doc_id: String,
    pub score: f64,
    pub keywords: Vec<(String, f64)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKeywordResponse {
    pub keyword: String,
    pub document_count: u32,
    pub scores: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteDocumentResponse {
    pub deleted: bool,
}

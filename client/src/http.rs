use crate::{
    query::{QueryBuilder, QueryExpr},
    ClientError, DeleteDocumentResponse, DeletedResponse, Document, ErrorResponse,
    GetKeywordResponse, IndexDocument, Result, SearchResponse, StatusResponse,
    UpdateDocumentResponse,
};
use std::{collections::HashMap, str::FromStr};

use futures::future::Future;
use reqwest::header::{HeaderName, HeaderValue};
use serde::Deserialize;

pub struct Client {
    base_url: String,
    api_key: Option<String>,
}

pub trait HttpClient: Send + Sync {
    fn request(
        &self,
        request: HttpRequest,
    ) -> Box<dyn Future<Output = Result<HttpResponse>> + Send + '_>;
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
}

static HEADER_API_KEY: &'static str = "X-API-Key";

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    // Status endpoint
    pub fn status(&self) -> Result<StatusResponse> {
        self.request::<StatusResponse>(HttpMethod::GET, "/", None, None)
    }

    // Index management endpoints
    pub fn list_indexes(&self) -> Result<Vec<String>> {
        self.request::<Vec<String>>(HttpMethod::GET, "/indexes", None, None)
    }

    pub fn get_index(&self, index: &str) -> Result<IndexDocument> {
        let url = format!("/{}", index);
        self.request::<IndexDocument>(HttpMethod::GET, &url, None, None)
    }

    pub fn create_index(&self, index: &str) -> Result<IndexDocument> {
        let url = format!("/{}", index);
        self.request::<IndexDocument>(HttpMethod::PUT, &url, None, None)
    }

    pub fn delete_index(&self, index: &str) -> Result<DeletedResponse> {
        let url = format!("/{}", index);
        self.request::<DeletedResponse>(HttpMethod::DELETE, &url, None, None)
    }

    // Document endpoints
    pub fn get_document(&self, index: &str, doc_id: &str) -> Result<Document> {
        let url = format!("/{}/doc/{}", index, doc_id);
        self.request::<Document>(HttpMethod::GET, &url, None, None)
    }

    pub fn add_document(&self, index: &str, body: String, lang: Option<&str>) -> Result<Document> {
        let mut url = format!("/{}/doc", index);
        if let Some(lang) = lang {
            url.push_str(&format!("?lang={}", urlencoding::encode(lang)));
        }
        self.request::<Document>(HttpMethod::POST, &url, Some(body), None)
    }

    pub fn update_document(
        &self,
        index: &str,
        doc_id: &str,
        body: String,
    ) -> Result<UpdateDocumentResponse> {
        let url = format!("/{}/doc/{}", index, doc_id);
        self.request::<UpdateDocumentResponse>(HttpMethod::PATCH, &url, Some(body), None)
    }

    pub fn delete_document(&self, index: &str, doc_id: &str) -> Result<DeleteDocumentResponse> {
        let url = format!("/{}/doc/{}", index, doc_id);
        self.request::<DeleteDocumentResponse>(HttpMethod::DELETE, &url, None, None)
    }

    // Search endpoint
    pub fn search(&self, index: &str, query: &str, full: Option<bool>) -> Result<SearchResponse> {
        let mut url = format!("/{}/search?query={}", index, urlencoding::encode(query));
        if let Some(full) = full {
            url.push_str(&format!("&full={}", full));
        }
        self.request::<SearchResponse>(HttpMethod::POST, &url, None, None)
    }

    /// Search using a QueryExpr
    pub fn search_expr(
        &self,
        index: &str,
        expr: &QueryExpr,
        full: Option<bool>,
    ) -> Result<SearchResponse> {
        self.search(index, &expr.to_query_string(), full)
    }

    /// Search using a QueryBuilder
    pub fn search_builder(
        &self,
        index: &str,
        builder: QueryBuilder,
        full: Option<bool>,
    ) -> Result<SearchResponse> {
        match builder.to_query_string() {
            Some(query) => self.search(index, &query, full),
            None => Err(ClientError::Api("Empty query builder".to_string())),
        }
    }

    // Keyword endpoint
    pub fn get_keyword(&self, index: &str, keyword: &str) -> Result<GetKeywordResponse> {
        let url = format!("/{}/keyword/{}", index, urlencoding::encode(keyword));
        self.request::<GetKeywordResponse>(HttpMethod::GET, url.as_str(), None, None)
    }

    fn request<T>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<String>,
        extra_headers: Option<HashMap<HeaderName, HeaderValue>>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.base_url, path);
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(api_key) = &self.api_key {
            headers.insert(HEADER_API_KEY, HeaderValue::from_str(api_key).unwrap());
        }

        if let Some(extra) = extra_headers {
            headers.extend(extra);
        }
        let response;
        let default_body = "".to_string();
        match method {
            HttpMethod::GET => {
                response = client
                    .get(&url)
                    .headers(headers)
                    .send()
                    .map_err(ClientError::Reqwest)?;
            }
            HttpMethod::POST => {
                response = client
                    .post(&url)
                    .headers(headers)
                    .body(body.unwrap_or(default_body))
                    .send()
                    .map_err(ClientError::Reqwest)?;
            }
            HttpMethod::PUT => {
                response = client
                    .put(&url)
                    .headers(headers)
                    .body(body.unwrap_or(default_body))
                    .send()
                    .map_err(ClientError::Reqwest)?;
            }
            HttpMethod::PATCH => {
                response = client
                    .patch(&url)
                    .headers(headers)
                    .body(body.unwrap_or(default_body))
                    .send()
                    .map_err(ClientError::Reqwest)?;
            }
            HttpMethod::DELETE => {
                response = client
                    .delete(url)
                    .headers(headers)
                    .send()
                    .map_err(ClientError::Reqwest)?;
            }
        }

        self.handle_response::<T>(response)
    }

    fn handle_response<T>(
        &self,
        response: reqwest::blocking::Response,
    ) -> std::result::Result<T, ClientError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status_code = response.status().as_u16();
        if status_code >= 200 && status_code < 300 {
            response
                .json::<T>()
                .map_err(|err| ClientError::Reqwest(err))
        } else {
            // Try to parse as error response first
            let raw_body = response.text().unwrap_or_default();
            let parsed_err = serde_json::from_str::<ErrorResponse>(&raw_body);
            if let Ok(error_response) = parsed_err {
                return Err(ClientError::Api(error_response.error));
            } else {
                Err(ClientError::Http(format!(
                    "HTTP {}: {}",
                    status_code, raw_body
                )))
            }
        }
    }
}

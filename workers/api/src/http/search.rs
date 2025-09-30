use worker::{Request, Response, Result, RouteContext};

use crate::{
    data::{bulk::BulkReader, keyword_shard::get_n_shards, PREFIX_DOCUMENT},
    durable::reader::get_durable_reader_namespace,
    lexer::lexer::QueryLexer,
    util::kv::get_kv_data_store,
};

pub async fn handle_search(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct SearchQuery {
        pub query: String,
        pub full: Option<bool>,
    }
    if let Some(index) = ctx.param("index") {
        if let Ok(query) = req.query::<SearchQuery>() {
            let store = get_kv_data_store(&ctx);
            let lexer = QueryLexer::from_str(query.query.as_str(), &store, &ctx.env);
            if !lexer.is_ok() {
                return Response::error(
                    crate::http::ErrorResponse {
                        error: "Failed to parse query".into(),
                    },
                    400,
                );
            }

            // Execute the search query
            let mut documents = lexer.unwrap().query(index).await;

            // If full document bodies are requested, fetch them
            if query.full.unwrap_or(false) {
                let durable_reader_ns = get_durable_reader_namespace(&ctx.env).unwrap();
                let durable_obj = durable_reader_ns.unique_id()?;
                let bulk_reader = BulkReader::new(get_n_shards(&ctx.env), &store, durable_obj);

                let doc_kv_keys: Vec<String> = documents
                    .iter()
                    .map(|key| format!("{}:{}{}", &index, PREFIX_DOCUMENT, &key.doc_id))
                    .collect();

                let full_doc_bodies = bulk_reader
                    .get_documents_kv_keys(doc_kv_keys.iter().map(|s| s.as_str()).collect())
                    .await;
                for i in 0..documents.len() {
                    let body = full_doc_bodies[i].document_body.clone();
                    documents[i].body = body;
                }
                // Iterate over all the found doc_ids and merge document data in
                return Response::from_json(&SearchResponse {
                    document_count: documents.len() as u32,
                    matches: documents,
                });
            }

            return Response::from_json(&SearchResponse {
                document_count: documents.len() as u32,
                matches: documents,
            });
        } else {
            return Response::error(
                crate::http::ErrorResponse {
                    error: "Missing query".into(),
                },
                400,
            );
        }
    } else {
        return Response::error(
            crate::http::ErrorResponse {
                error: "Missing index name".into(),
            },
            400,
        );
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct SearchResponse {
    document_count: u32,
    matches: Vec<SearchResultRow>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SearchResultRow {
    pub doc_id: String,
    pub score: f64,
    pub keywords: Vec<(String, f64)>,
    pub body: Option<String>,
}

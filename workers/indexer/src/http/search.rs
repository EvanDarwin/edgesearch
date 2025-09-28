use worker::{Request, Response, Result, RouteContext};

use crate::{
    data::document::{document_kv_key, Document},
    lexer::QueryLexer,
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
            let mut lexer = QueryLexer::new(query.query.as_str(), &store);
            let documents = lexer.query(index).await;

            if query.full.unwrap_or(false) {
                let document_bodies = Document::many_from_remote(
                    documents
                        .iter()
                        .map(|key| document_kv_key(index, &key.doc_id))
                        .collect(),
                    store.clone(),
                )
                .await
                .unwrap();

                let documents: Vec<SearchResultRow> = documents
                    .into_iter()
                    .map(move |doc| SearchResultRow {
                        body: document_bodies
                            .iter()
                            .find(|d| d.get_uuid() == doc.doc_id)
                            .and_then(|d| d.document_body.clone()),
                        ..doc
                    })
                    .collect();

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

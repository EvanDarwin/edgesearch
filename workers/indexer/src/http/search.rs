use std::collections::HashMap;

use worker::{Request, Response, Result, RouteContext};

use crate::{lexer::QueryLexer, util::kv::get_kv_data_store};

pub async fn handle_search(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    #[derive(serde::Deserialize)]
    struct SearchQuery {
        pub query: String,
    }
    if let Some(index) = ctx.param("index") {
        if let Ok(query) = req.query::<SearchQuery>() {
            let store = get_kv_data_store(&ctx);
            let mut lexer = QueryLexer::new(query.query.as_str(), store);
            let documents = lexer.query(index).await;

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
    matches: HashMap<String, Vec<(String, f64)>>,
}

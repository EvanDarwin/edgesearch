use std::collections::HashMap;

use worker::{Request, Response};

use crate::{data::keyword::KeywordManager, util::kv::get_kv_data_store};

#[derive(serde::Serialize)]
struct GetKeywordResponse {
    keyword: String,
    document_count: u32,
    scores: HashMap<String, f64>,
}
pub async fn handle_get_keyword(
    _req: Request,
    ctx: worker::RouteContext<()>,
) -> worker::Result<Response> {
    if let Some(index) = ctx.param("index") {
        if let Some(keyword) = ctx.param("keyword") {
            let state = get_kv_data_store(&ctx);
            let manager = KeywordManager::new(index.into(), &ctx.env, &state);
            let merged = manager.merge_keyword_shards(keyword.into()).await.unwrap();

            let document_count = merged.len() as u32;
            let scores: HashMap<String, f64> = merged.into_iter().collect();

            return Response::from_json(&GetKeywordResponse {
                keyword: keyword.into(),
                document_count,
                scores,
            });
        } else {
            return Response::error(
                crate::http::ErrorResponse {
                    error: "Missing keyword".into(),
                },
                400,
            );
        }
    }
    return Response::error(
        crate::http::ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}

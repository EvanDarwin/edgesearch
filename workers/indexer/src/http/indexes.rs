use worker::{Request, Response, Result, RouteContext};

use crate::{
    data::{index::IndexDocument, index_manager::IndexManager},
    http::ErrorResponse,
    util::kv::get_kv_data_store,
};

#[derive(serde::Serialize)]
struct DeletedResponse {
    deleted: bool,
}

pub async fn handle_list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let indexer = IndexManager::new(get_kv_data_store(&ctx));
    let known_indexes = indexer.list_indexes().await.unwrap();
    return Response::from_json(&known_indexes);
}

pub async fn handle_view(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache = get_kv_data_store(&ctx);
    if let Some(index) = ctx.param("index") {
        let indexer = IndexManager::new(cache);
        if let Ok(index_data) = indexer.read_index(index).await {
            return Response::from_json(&index_data);
        } else {
            return Response::error(
                ErrorResponse {
                    error: "Index not found".into(),
                },
                404,
            );
        }
    }
    return Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}

pub async fn handle_create(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache = get_kv_data_store(&ctx);
    if let Some(index) = ctx.param("index") {
        let indexer = IndexManager::new(cache);
        if IndexDocument::is_reserved_index(index) {
            return Response::error(
                ErrorResponse {
                    error: "Index name is reserved".into(),
                },
                400,
            );
        }

        let index_data = indexer.create_index(index).await.unwrap();
        return Response::from_json(&index_data);
    }
    Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    )
}

pub async fn handle_delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cache = get_kv_data_store(&ctx);
    if let Some(index) = ctx.param("index") {
        let indexer = IndexManager::new(cache);
        indexer.delete_index(index).await.unwrap();
        return Response::from_json(&DeletedResponse { deleted: true });
    }
    return Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}

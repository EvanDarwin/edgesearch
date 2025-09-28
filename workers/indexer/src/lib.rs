// Load before other modules which depend on macros in here.
#[macro_use]
mod util;
mod data;
mod durable;
mod http;
pub mod lexer;

use worker::{event, Context, Env, Request, Response, Result, RouteContext, Router};

use crate::data::DataStoreError;

fn check_auth(req: &Request, ctx: &RouteContext<()>) -> bool {
    // Check if API_KEY env var is set, if not ignore
    ctx.env
        .var("API_KEY")
        .map_err(DataStoreError::Worker)
        .map(|v| {
            let api_key = req.headers().get("X-API-Key").unwrap_or(None);
            api_key.as_ref() == Some(&v.to_string())
        })
        .unwrap_or_else(|_| false)
}

macro_rules! with_auth {
    ($handler:expr) => {
        |req: Request, ctx: RouteContext<()>| async move {
            if crate::check_auth(&req, &ctx) {
                $handler(req, ctx).await
            } else {
                worker::Response::error("Unauthorized", 401)
            }
        }
    };
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    return Router::new()
        .get_async("/", http::index::handle_index)
        // Search endpoints
        .post_async("/:index/search", with_auth!(http::search::handle_search))
        // Keyword endpoints
        .get_async(
            "/:index/keyword/:keyword",
            with_auth!(http::keywords::handle_get_keyword),
        )
        // Document endpoints
        .get_async(
            "/:index/document/:id",
            with_auth!(http::documents::handle_get_document),
        )
        .post_async(
            "/:index/document",
            with_auth!(http::documents::handle_add_document),
        )
        .patch_async(
            "/:index/document/:id",
            with_auth!(http::documents::handle_update_document),
        )
        .delete_async(
            "/:index/document/:id",
            with_auth!(http::documents::handle_delete_document),
        )
        // Index endpoints (protected)
        .get_async("/indexes", with_auth!(http::indexes::handle_list))
        .get_async("/:index", with_auth!(http::indexes::handle_view))
        .put_async("/:index", with_auth!(http::indexes::handle_create))
        .delete_async("/:index", with_auth!(http::indexes::handle_delete))
        // Run router
        .run(req, env)
        .await;
}

#[macro_export]
macro_rules! edge_log {
    ($module:expr, $index:expr, $msg:expr $(, $args:tt)* ) => {
        #[cfg(not(target_arch = "wasm32"))]
        worker::console_log!(
            "[{}][{}] {}",
            $module,
            $index,
            format!($msg $(,$args)*)
        );
    }
}

#[macro_export]
macro_rules! edge_warn {
    ($module:expr, $index:expr, $msg:expr $(, $args:tt)* ) => {
        worker::console_warn!(
            "[{}][{}] {}",
            $module,
            $index,
            format!($msg $(,$args)*)
        );
    }
}

#[macro_export]
macro_rules! edge_error {
    ($module:expr, $index:expr, $msg:expr $(, $args:tt)* ) => {
        worker::console_error!(
            "[{}][{}] {}",
            $module,
            $index,
            format!($msg $(,$args)*)
        );
    }
}

#[macro_export]
macro_rules! edge_debug {
    ($module:expr, $index:expr, $msg:expr $(, $args:tt)* ) => {
        #[cfg(not(target_arch = "wasm32"))]
        worker::console_debug!(
            "[{}][{}] {}",
            $module,
            $index,
            format!($msg $(,$args)*)
        );
    }
}

// Load before other modules which depend on macros in here.
#[macro_use]
mod util;
mod data;
mod durable;
mod http;

use worker::{console_log, event, Context, Env, Request, Response, Result, Router};

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_log!("{}, {}", req.method().to_string(), req.path().as_str());
    return Router::new()
        .get_async("/", http::index::handle_index)
        // Index endpoints
        .get_async("/index", http::indexes::handle_list)
        .put_async("/index/:index", http::indexes::handle_create)
        .delete_async("/index/:index", http::indexes::handle_delete)
        // Search endpoints
        .post_async("/search/:index", http::search::handle_search)
        // Keyword endpoints
        .post_async(
            "/keywords/:index/:keyword",
            http::keywords::handle_get_keyword,
        )
        // Document endpoints
        .get_async(
            "/documents/:index/:id",
            http::documents::handle_get_document,
        )
        .put_async(
            "/documents/:index/:id",
            http::documents::handle_add_document,
        )
        .delete_async(
            "/documents/:index/:id",
            http::documents::handle_delete_document,
        )
        // Run router
        .run(req, env)
        .await;
}

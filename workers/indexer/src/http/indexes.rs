use worker::{Request, Response, Result, RouteContext};

use crate::http::RESPONSE_JSON_STATUS_READY;

pub async fn handle_list(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes(RESPONSE_JSON_STATUS_READY.into());
}

pub async fn handle_create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes(RESPONSE_JSON_STATUS_READY.into());
}

pub async fn handle_delete(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes(RESPONSE_JSON_STATUS_READY.into());
}

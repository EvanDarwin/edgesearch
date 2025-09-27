use worker::{Request, Response, Result, RouteContext};

pub async fn handle_get_document(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes("Not implemented".into());
}

pub async fn handle_add_document(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes("Not implemented".into());
}

pub async fn handle_delete_document(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    return Response::from_bytes("Not implemented".into());
}

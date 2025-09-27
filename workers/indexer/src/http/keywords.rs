use worker::{Request, Response, Result, RouteContext};

pub async fn handle_get_keyword(
    req: Request,
    ctx: worker::RouteContext<()>,
) -> worker::Result<Response> {
    return Response::from_bytes("Not implemented".into());
}

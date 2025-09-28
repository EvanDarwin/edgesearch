use worker::{Request, Response, Result, RouteContext};

use crate::http::StatusResponse;

pub async fn handle_index(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    if req.headers().get("Accept").map_or(false, |accept| {
        accept.expect("unreadable").contains("text/html")
    }) {
        return Response::from_html(include_str!("../../index.html"));
    } else {
        return Response::from_json(&StatusResponse { ready: true });
    }
}

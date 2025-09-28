use std::sync::Arc;

use worker::{kv::KvStore, RouteContext};

const KV_BINDING_NAME: &'static str = "INDEX";

pub fn get_kv_data_store(ctx: &RouteContext<()>) -> Arc<KvStore> {
    let kv = ctx.kv(KV_BINDING_NAME);
    if kv.is_err() {
        panic!("Failed to get KV store binding");
    }
    return Arc::new(kv.unwrap());
}

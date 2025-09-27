use worker::{kv::KvStore, RouteContext};

const KV_BINDING_NAME: &'static str = "INDEX";

pub fn get_kv_data_store(ctx: RouteContext<()>) -> KvStore {
    let kv = ctx.kv(KV_BINDING_NAME);
    if kv.is_err() {
        panic!("Failed to get KV store binding");
    }
    return kv.unwrap();
}

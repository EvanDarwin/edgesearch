use std::sync::Arc;

use worker::{kv::KvStore, RouteContext};

const KV_BINDING_NAME: &'static str = "INDEX";

pub fn get_kv_data_store(ctx: &RouteContext<()>) -> Arc<KvStore> {
    Arc::new(ctx.kv(KV_BINDING_NAME).unwrap())
}

pub fn get_kv_data_store_from_env(env: &worker::Env) -> Arc<KvStore> {
    Arc::new(env.kv(KV_BINDING_NAME).unwrap())
}

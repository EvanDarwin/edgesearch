use serde::{Deserialize, Serialize};

use crate::{
    data::{DataStoreError, KvEntry, KvPersistent},
    util::kv::get_kv_data_store,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct IndexDocument {
    index: IndexName,
    count: u32,
    created: u64,
}

impl KvEntry for IndexDocument {
    type Key = IndexName;

    fn get_kv_key(&self) -> Self::Key {
        return format!("index:{}", self.index) as IndexName;
    }
}

impl KvPersistent for IndexDocument {
    async fn write(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError> {
        let store = get_kv_data_store(ctx);
        let data = serde_json::to_string(self).map_err(DataStoreError::Serialization)?;
        store
            .put(self.get_kv_key().as_str(), data.as_str())
            .map_err(DataStoreError::Kv)?
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;
        Ok(())
    }

    async fn read(key: &str, ctx: worker::RouteContext<()>) -> Result<Self, DataStoreError> {
        let store = get_kv_data_store(ctx);
        let result = store
            .get(key)
            .json::<IndexDocument>()
            .await
            .map_err(DataStoreError::Kv)?;
        result.ok_or_else(|| DataStoreError::NotFound(key.to_string()))
    }

    async fn save(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError> {
        self.write(ctx).await
    }
}

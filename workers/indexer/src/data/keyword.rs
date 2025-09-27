use serde::{Deserialize, Serialize};

use crate::data::default_loaded;
use crate::data::document::shard_from_document_id;
use crate::data::index::IndexName;
use crate::data::DataStoreError;
use crate::data::DocumentRef;
use crate::data::KeywordRef;
use crate::data::KvEntry;
use crate::data::KvPersistent;
use crate::util::kv::get_kv_data_store;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct KeywordShardData {
    // The name of the index the keyword belongs to
    index: IndexName,

    // The keyword indexed
    keyword: String,

    // The shard number for this keyword
    shard: u32,

    // Last modified timestamp (versioning)
    ts: u64,

    // Whether the document list has been loaded from KV
    #[serde(skip_serializing, default = "default_loaded")]
    loaded: bool,

    // List of document references containing this keyword (sets loaded)
    docs: Option<Vec<DocumentRef>>,
}

fn keyword_shard_kv_key(index: &str, keyword: &str, shard: u32, ts: u64) -> KeywordRef {
    return format!("{}:kw:{}:{}:{}", index, keyword, shard, ts) as KeywordRef;
}

impl KvEntry for KeywordShardData {
    type Key = KeywordRef;

    fn get_kv_key(&self) -> Self::Key {
        return keyword_shard_kv_key(
            &self.index.as_str(),
            &self.keyword.as_str(),
            self.shard,
            self.ts,
        );
    }
}

impl KvPersistent for KeywordShardData {
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
            .json::<KeywordShardData>()
            .await
            .map_err(DataStoreError::Kv)?;

        result.ok_or_else(|| DataStoreError::NotFound(key.to_string()))
    }

    async fn save(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError> {
        self.write(ctx).await
    }
}

impl KeywordShardData {
    pub fn new(
        index: IndexName,
        keyword: String,
        shard: u32,
        ts: u64,
        docs: Option<Vec<DocumentRef>>,
    ) -> KeywordShardData {
        return KeywordShardData {
            index,
            keyword,
            shard,
            ts,
            loaded: docs.is_some(),
            docs,
        };
    }

    pub async fn from_keyword(
        ctx: worker::RouteContext<()>,
        index: IndexName,
        keyword: String,
        num_shards: u32,
    ) -> Result<KeywordShardData, DataStoreError> {
        let shard = shard_from_document_id(keyword.clone(), num_shards);
        Self::read(
            &keyword_shard_kv_key(&index.as_str(), &keyword, shard, 0),
            ctx,
        )
        .await
    }
}

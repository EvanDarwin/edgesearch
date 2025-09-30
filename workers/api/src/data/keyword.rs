use std::sync::Arc;

use worker::{kv::KvStore, Env};

use crate::{
    data::{
        bulk::BulkReader, keyword_shard::get_n_shards, DataStoreError, IndexName, PREFIX_KEYWORD,
    },
    durable::reader::get_durable_reader_namespace,
    edge_log,
    util::http::url_decode,
};

pub struct KeywordManager<'a> {
    index: IndexName,
    env: &'a Env,
    state: &'a Arc<KvStore>,
}

type MergedKeywordData = Vec<(String, f64)>;
impl<'a> KeywordManager<'a> {
    pub fn new(index: IndexName, env: &'a Env, state: &'a Arc<KvStore>) -> KeywordManager<'a> {
        return KeywordManager { index, env, state };
    }

    pub async fn merge_keyword_shards(
        &self,
        keyword_raw: String,
    ) -> Result<MergedKeywordData, DataStoreError> {
        let keyword: String = url_decode(keyword_raw.as_str());
        let keyword_shards = self
            .state
            .list()
            .prefix(format!("{}:{}{}:", self.index, PREFIX_KEYWORD, keyword))
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;

        let shard_count = keyword_shards.keys.len();
        edge_log!(
            console_debug,
            "KeywordManager",
            &self.index,
            "keyword shard merge initiated  keyword={}, shard_count={}",
            keyword,
            shard_count
        );

        let kv_keys: Vec<&str> = keyword_shards
            .keys
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();

        // Use our new Durable Object reader to fetch the keyword shards in bulk async
        let durable_reader_ns = get_durable_reader_namespace(self.env)?;
        let durable_reader = durable_reader_ns.unique_id()?;
        let bulk = BulkReader::new(get_n_shards(self.env), &self.state, durable_reader);
        let kv_keys_len = kv_keys.len();
        let kv_data = bulk.get_keyword_kv_keys(kv_keys).await;
        assert!(kv_data.len() == kv_keys_len);

        // Flatten and sort documents by score
        let mut merged_keywords: Vec<(String, f64)> =
            kv_data.iter().flat_map(|data| data.docs.clone()).collect();
        merged_keywords.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total_doc_count = merged_keywords.len();
        edge_log!(
            console_log,
            "KeywordManager",
            &self.index,
            "keyword shard merge completed keyword={}, merged_docs={}",
            keyword,
            total_doc_count
        );

        Ok(merged_keywords)
    }
}

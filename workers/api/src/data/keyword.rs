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
        let durable_obj_ns = get_durable_reader_namespace(self.env)?;
        let durable_obj = durable_obj_ns.unique_id()?;
        let bulk_reader = BulkReader::new(get_n_shards(self.env), &self.state, durable_obj);

        let keyword: String = url_decode(keyword_raw.as_str());
        let keyword_shards = bulk_reader
            .list(format!("{}:{}{}:", self.index, PREFIX_KEYWORD, keyword).as_str())
            .await?;

        let shard_count = keyword_shards.len();
        edge_log!(
            console_debug,
            "KeywordManager",
            &self.index,
            "keyword shard merge initiated  keyword={}, shard_count={}",
            keyword,
            shard_count
        );

        let keyword_shards_str: Vec<&str> =
            keyword_shards.iter().map(|entry| entry.as_str()).collect();

        // Use our new Durable Object reader to fetch the keyword shards in bulk async
        let kv_keys_len = keyword_shards.len();
        let kv_data = bulk_reader.get_keyword_kv_keys(keyword_shards_str).await;
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

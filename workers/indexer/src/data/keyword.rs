use std::{collections::HashMap, sync::Arc};

use futures::future::join_all;
use worker::kv::KvStore;

use crate::{
    data::{
        keyword_shard::KeywordShardData, DataStoreError, IndexName, KvPersistent, PREFIX_KEYWORD,
    },
    edge_debug, edge_log, edge_warn,
    util::http::url_decode,
};

pub struct KeywordManager<'a> {
    index: IndexName,
    state: &'a Arc<KvStore>,
}

type MergedKeywordData = Vec<(String, f64)>;
impl<'a> KeywordManager<'a> {
    pub fn new(index: IndexName, state: &'a Arc<KvStore>) -> KeywordManager<'a> {
        return KeywordManager { index, state };
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
        edge_debug!(
            "KeywordManager",
            &self.index,
            "keyword shard merge initiated  keyword={}, shard_count={}",
            keyword,
            shard_count
        );

        let mut keyword_merge_map: HashMap<String, f64> = HashMap::new();
        let futures: Vec<_> = keyword_shards
            .keys
            .iter()
            .map(|entry| {
                let kv_key = &entry.name;
                KeywordShardData::read(kv_key, &self.state)
            })
            .collect();

        let shard_results = join_all(futures).await;
        for (i, shard_result) in shard_results.into_iter().enumerate() {
            let kv_key = &keyword_shards.keys[i].name;
            match shard_result {
                Ok(shard) => {
                    shard.docs.iter().for_each(|(doc, score)| {
                        keyword_merge_map.insert(doc.clone(), *score);
                    });
                }
                Err(err) => {
                    edge_warn!(
                        "KeywordManager",
                        &self.index,
                        "Failed to read keyword shard data for key {}: {:?}",
                        kv_key,
                        err
                    );
                }
            }
        }

        // Convert the HashMap into a Vec for sorting
        let mut sorted: Vec<(String, f64)> = keyword_merge_map.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total_doc_count = sorted.len();
        edge_log!(
            "KeywordManager",
            &self.index,
            "keyword shard merge completed keyword={}, merged_docs={}",
            keyword,
            total_doc_count
        );

        Ok(sorted)
    }
}

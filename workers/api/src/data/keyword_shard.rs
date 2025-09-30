use serde::{Deserialize, Serialize};
use worker::{kv::KvStore, Env};

use crate::{
    data::{
        document::shard_from_document_id, DataStoreError, DocumentRef, IndexName, KeywordRef,
        KvEntry, KvPersistent, DEFAULT_N_SHARDS, ENV_VAR_N_SHARDS, PREFIX_KEYWORD,
    },
    edge_log,
};

pub fn get_n_shards(env: &worker::Env) -> u32 {
    env.var(ENV_VAR_N_SHARDS)
        .map_err(DataStoreError::Worker)
        .map(|v| v.to_string().parse::<u32>())
        .unwrap_or(Ok(DEFAULT_N_SHARDS))
        .unwrap()
}

pub fn keyword_shard_kv_key(index: &str, keyword: &str, shard: u32) -> KeywordRef {
    return format!("{}:{}{}:{}", index, PREFIX_KEYWORD, keyword, shard) as KeywordRef;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeywordShardData {
    // The name of the index the keyword belongs to
    pub index: IndexName,

    // The keyword indexed
    pub keyword: String,

    // The shard number for this keyword
    pub shard: u32,

    // Last modified timestamp (versioning)
    pub ts: u64,

    // List of document references containing this keyword (sets loaded)
    pub docs: Vec<(DocumentRef, f64)>,
}

impl KvEntry for KeywordShardData {
    type Key = KeywordRef;

    fn get_kv_key(&self) -> Self::Key {
        return keyword_shard_kv_key(&self.index.as_str(), &self.keyword.as_str(), self.shard);
    }
}

impl KvPersistent for KeywordShardData {
    async fn read(key: &str, store: &KvStore) -> Result<Self, DataStoreError> {
        let result = store
            .get(key)
            .json::<KeywordShardData>()
            .await
            .map_err(DataStoreError::Kv)?;

        result.ok_or_else(|| DataStoreError::NotFound(key.to_string()))
    }
}

impl KeywordShardData {
    pub fn new(
        index: IndexName,
        keyword: String,
        shard: u32,
        ts: u64,
        docs: Vec<(DocumentRef, f64)>,
    ) -> KeywordShardData {
        return KeywordShardData {
            index,
            keyword,
            shard,
            ts,
            docs,
        };
    }

    pub async fn from_keyword(
        store: &KvStore,
        env: &Env,
        index: &str,
        doc_id: &str,
        keyword: &str,
    ) -> Result<KeywordShardData, DataStoreError> {
        let shard = shard_from_document_id(doc_id.to_string(), get_n_shards(env));
        let shard_key = keyword_shard_kv_key(index, keyword, shard);
        edge_log!(
            console_debug,
            "KeywordShardData",
            index,
            "KeywordShardData::from_keyword({}, {}) kv={}",
            doc_id,
            keyword,
            shard_key
        );

        let found_shard = Self::read(&keyword_shard_kv_key(&index, &keyword, shard), &store).await;
        if let Ok(shard_data) = found_shard {
            edge_log!(
                console_debug,
                "KeywordShardData",
                index,
                "loaded existing shard data for keyword '{}' shard {}",
                keyword,
                shard
            );
            Ok(shard_data)
        } else {
            edge_log!(
                console_debug,
                "KeywordShardData",
                index,
                "creating new shard data for keyword '{}' shard {}",
                keyword,
                shard
            );
            let mut shard = KeywordShardData::new(
                index.to_string(),
                keyword.to_string(),
                shard,
                worker::Date::now().as_millis().into(),
                vec![],
            );
            shard.write(&store).await?;
            Ok(shard)
        }
    }

    pub async fn add_document(
        &mut self,
        store: &KvStore,
        doc_id: &str,
        score: f64,
    ) -> Result<(), DataStoreError> {
        // Check if the document already exists in the list
        if !self.docs.iter().any(|(d, _)| d == doc_id) {
            self.docs.push((doc_id.to_string(), score));
            self.ts = worker::Date::now().as_millis().into();
            self.write(store).await?;
        }
        Ok(())
    }

    pub async fn remove_document(
        &mut self,
        store: &KvStore,
        doc_id: &str,
    ) -> Result<(), DataStoreError> {
        let original_len = self.docs.len();
        self.docs.retain(|(d, _)| d != doc_id);
        if self.docs.len() != original_len {
            self.ts = worker::Date::now().as_millis().into();
            self.write(store).await?;
        }
        Ok(())
    }
}

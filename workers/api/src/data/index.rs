use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use worker::kv::KvStore;

use crate::data::{DataStoreError, IndexName, KvEntry, KvPersistent, PREFIX_INDEX};

static RESERVED_INDEXES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("indexes", "Reserved for EdgeSearch system use");
    m.insert("_internal", "Internal service index");
    m
});

#[derive(Serialize, Deserialize, Clone)]
pub struct IndexDocument {
    pub index: IndexName,
    pub docs_count: u32,
    pub version: u8,
    pub created: u64,
}

impl IndexDocument {
    pub fn is_reserved_index(index: &str) -> bool {
        return RESERVED_INDEXES.contains_key(index);
    }
}

pub fn get_index_key(index: &str) -> IndexName {
    return format!("{}{}", PREFIX_INDEX, index) as IndexName;
}

impl KvEntry for IndexDocument {
    type Key = IndexName;

    fn get_kv_key(&self) -> Self::Key {
        get_index_key(&self.index)
    }
}

impl KvPersistent for IndexDocument {
    async fn read(key: &str, store: &KvStore) -> Result<Self, DataStoreError> {
        let result = store
            .get(key)
            .json::<IndexDocument>()
            .await
            .map_err(DataStoreError::Kv)?
            .unwrap();
        Ok(result)
    }

    async fn write(&mut self, store: &KvStore) -> Result<(), DataStoreError> {
        store
            .put(
                self.get_kv_key().as_str(),
                serde_json::to_string(self).unwrap().as_str(),
            )
            .map_err(DataStoreError::Kv)?
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;
        Ok(())
    }
}

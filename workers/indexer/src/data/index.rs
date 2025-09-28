use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use worker::kv::KvStore;

use crate::data::{
    DataStoreError, IndexName, KvEntry, KvPersistent, INDEX_VERSION_V1, PREFIX_INDEX,
};

static RESERVED_INDEXES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("indexes", "Reserved for EdgeSearch system use");
    m.insert("_internal", "Internal service index");
    m
});

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexDocument {
    pub index: IndexName,
    pub version: u8,
    pub created: u64,
}

impl IndexDocument {
    pub fn new(index: &str) -> IndexDocument {
        return IndexDocument {
            index: index.to_string(),
            version: INDEX_VERSION_V1,
            created: worker::Date::now().as_millis().into(),
        };
    }

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
            .map_err(DataStoreError::Kv)?;
        result.ok_or_else(|| DataStoreError::NotFound(key.to_string()))
    }
}

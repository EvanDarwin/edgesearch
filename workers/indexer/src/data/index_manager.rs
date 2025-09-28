use std::sync::Arc;

use worker::kv::KvStore;

use crate::{
    data::{
        index::{get_index_key, IndexDocument},
        DataStoreError, PREFIX_INDEX,
    },
    edge_debug, edge_log, edge_warn,
};

pub struct IndexManager {
    store: Arc<KvStore>,
}

impl IndexManager {
    pub fn new(store: Arc<KvStore>) -> IndexManager {
        return IndexManager { store };
    }

    pub async fn list_indexes(&self) -> Result<Vec<String>, DataStoreError> {
        let found_indexes = self
            .store
            .list()
            .prefix(PREFIX_INDEX.into())
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;

        let indexes: Vec<String> = found_indexes
            .keys
            .iter()
            .map(|key| -> String { key.name.strip_prefix(PREFIX_INDEX).unwrap().to_string() })
            .collect();

        let index_count = indexes.len();
        edge_debug!("IndexManager", "", "found {} indexes", index_count);
        Ok(indexes)
    }

    pub async fn read_index(&self, index: &str) -> Result<IndexDocument, DataStoreError> {
        let key = get_index_key(index);
        let document = self
            .store
            .get(&key)
            .json::<IndexDocument>()
            .await
            .map_err(DataStoreError::Kv)?;

        if document.is_none() {
            edge_debug!("IndexManager", index, "index not found in KV");
            return Err(DataStoreError::NotFound(index.to_string()));
        }

        edge_debug!("IndexManager", index, "load from KV");
        Ok(document.unwrap())
    }

    pub async fn create_index(&self, index_name: &str) -> Result<IndexDocument, DataStoreError> {
        // First, read to see if it already exists.
        let existing_version = self.read_index(index_name).await;
        // Return the existing version if it exists NOT AN ERROR
        if existing_version.is_ok() {
            edge_debug!(
                "IndexManager",
                index_name,
                "index already exists, skipping creation"
            );
            return Ok(existing_version.unwrap());
        } else {
            let err = existing_version.err().unwrap();
            edge_warn!("IndexManager", index_name, "index does not exist: {}", err);
        }

        let index_doc = &IndexDocument::new(index_name);
        let index_json = serde_json::to_string(index_doc).map_err(DataStoreError::Serialization)?;

        self.store
            .put(get_index_key(index_name).as_str(), &index_json)
            .map_err(DataStoreError::Kv)
            .unwrap()
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;

        edge_log!("IndexManager", index_name, "created index");
        Ok(index_doc.to_owned())
    }

    pub async fn delete_index(&self, index_name: &str) -> Result<(), DataStoreError> {
        let key = get_index_key(index_name);
        self.store.delete(&key).await.map_err(DataStoreError::Kv)?;
        edge_log!("IndexManager", index_name, "deleted index");
        Ok(())
    }
}

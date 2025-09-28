use serde::{Deserialize, Serialize};
use thiserror::Error;
use worker::kv::KvStore;

pub type KeywordRef = String;
pub type DocumentRef = String;
pub type IndexName = String;

pub type DocumentScore<'a> = (String, f64);

pub static PREFIX_INDEX: &str = "index:";
pub static PREFIX_DOCUMENT: &str = "document:";
pub static PREFIX_KEYWORD: &str = "kw:";

pub const INDEX_VERSION_V1: u8 = 1u8;

pub trait KvEntry: Sized + Serialize + Deserialize<'static> {
    type Key: Into<String>;
    fn get_kv_key(&self) -> Self::Key;
}

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error("No KV key named '{0}' was found")]
    NotFound(String),
    #[error("Serialization/Deserialization error: {0}")]
    Serialization(serde_json::Error),
    #[error("KV store error: {0:?}")]
    Kv(worker::kv::KvError),
    #[error("Worker error: {0}")]
    Worker(#[from] worker::Error),
}

trait KvPersistent: KvEntry + Deserialize<'static> + Serialize {
    async fn write(&mut self, store: &KvStore) -> Result<(), DataStoreError> {
        let kv_key = self.get_kv_key().into();
        let serialized = serde_json::to_string(self).unwrap();
        store
            .put(&kv_key, serialized)
            .map_err(DataStoreError::Kv)?
            .execute()
            .await
            .map_err(DataStoreError::Kv)
    }
    async fn read(key: &str, store: &KvStore) -> Result<Self, DataStoreError>;
}

#[macro_use]
pub mod document;
pub mod index;
pub mod index_manager;
pub mod keyword_shard;
#[macro_use]
pub mod keyword;

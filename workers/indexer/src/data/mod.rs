use thiserror::Error;

pub type KeywordRef = String;
pub type DocumentId = String;
pub type DocumentRef = String;
pub type IndexName = String;

// Specifies a version for the data structure stored in the KV.
type IndexHash = String;

const DATA_VERSION_V1: u8 = 1u8;

trait KvEntry: Sized {
    type Key;
    fn get_kv_key(&self) -> Self::Key;
}

trait KvVersionedEntry: KvEntry {
    fn get_data_version(&self) -> &u32;
}

#[derive(Error, Debug)]
pub enum DataStoreError {
    #[error("Key/value pair not found for key: {0}")]
    NotFound(String),
    #[error("Serialization/Deserialization error: {0}")]
    Serialization(serde_json::Error),
    #[error("KV store error: {0:?}")]
    Kv(worker::kv::KvError),
}

trait KvPersistent: KvEntry {
    async fn write(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError>;
    async fn read(key: &str, ctx: worker::RouteContext<()>) -> Result<Self, DataStoreError>;
    async fn save(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError>;
}

pub fn default_loaded() -> bool {
    true
}

#[macro_use]
mod document;
mod index;
#[macro_use]
mod keyword;

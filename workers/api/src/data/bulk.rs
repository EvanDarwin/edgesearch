use futures::future::join_all;
use serde::Deserialize;
use worker::{kv::KvStore, Method, ObjectId, RequestInit};

use crate::{
    data::{
        document::Document, encoding::read_length_prefixed, keyword_shard::KeywordShardData,
        DataStoreError, KvPersistent,
    },
    durable::reader::{get_document_limit, get_keyword_limit},
};

pub struct BulkReader<'a> {
    n_shards: u32,
    store: &'a KvStore,
    durable_obj: ObjectId<'a>,
}

static BULK_READER_DATA_KEYWORDS: &str = "/keywords";
static BULK_READER_DATA_DOCUMENTS: &str = "/documents";

impl<'a> BulkReader<'a> {
    pub fn new(n_shards: u32, store: &'a KvStore, durable_obj: ObjectId<'a>) -> BulkReader<'a> {
        BulkReader {
            n_shards,
            store,
            durable_obj,
        }
    }

    async fn chunked_request<S: for<'de> Deserialize<'de> + Clone>(
        &self,
        read_type: &str,
        kv_keys: Vec<&str>,
    ) -> Vec<S> {
        let max_per_chunk: u32;
        let path: &str;
        if read_type == BULK_READER_DATA_KEYWORDS {
            path = read_type;
            max_per_chunk = get_keyword_limit(self.n_shards);
        } else if read_type == BULK_READER_DATA_DOCUMENTS {
            path = read_type;
            max_per_chunk = 1000u32;
        } else {
            panic!("Unknown read_type provided: {}", read_type);
        }

        // Chunk into max_per_chunk sized pieces
        let chunk_futures: Vec<_> = kv_keys
            .chunks(max_per_chunk as usize)
            .map(async |chunk| {
                let req = worker::Request::new_with_init(
                    format!("https://do{}", path).as_str(),
                    &RequestInit {
                        method: Method::Post,
                        body: Some(chunk.join(",").as_str().into()),
                        ..Default::default()
                    },
                )
                .unwrap();

                self.durable_obj
                    .get_stub()
                    .unwrap()
                    .fetch_with_request(req)
                    .await
                    .unwrap()
                    .bytes()
                    .await
                    .unwrap()
            })
            .collect();

        let data_chunks: Vec<Vec<u8>> = join_all(chunk_futures).await;
        data_chunks
            .into_iter()
            .map(move |bytes| read_length_prefixed::<S>(&bytes))
            .flatten()
            .collect()
    }

    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, DataStoreError> {
        let mut response = self
            .store
            .list()
            // .prefix(format!("{}:{}{}:", self.index, PREFIX_KEYWORD, keyword))
            .prefix(prefix.into())
            .execute()
            .await
            .map_err(DataStoreError::Kv)?;
        let mut keys: Vec<String> = response.keys.iter().map(|k| k.name.to_string()).collect();

        while !response.list_complete {
            if let Some(cursor) = response.cursor {
                response = self
                    .store
                    .list()
                    .prefix(prefix.into())
                    .cursor(cursor)
                    .execute()
                    .await
                    .map_err(DataStoreError::Kv)?;

                keys.extend(
                    response
                        .keys
                        .iter()
                        .map(|k| k.name.to_string())
                        .collect::<Vec<String>>(),
                );
            } else {
                break;
            }
        }

        Ok(keys)
    }

    /// Directly query a list of keyword shard KV keys from the durable object,
    /// bypassing the 1,000 op limit through invoking extra requests to a durable object.
    pub async fn get_keyword_kv_keys(&self, kv_keys: Vec<&str>) -> Vec<KeywordShardData> {
        let keyword_chunk_limit = get_keyword_limit(self.n_shards);
        if kv_keys.len() < keyword_chunk_limit as usize {
            let futures: Vec<_> = kv_keys
                .iter()
                .map(async |kv_key| KeywordShardData::read(kv_key, self.store).await.unwrap())
                .collect();

            join_all(futures).await
        } else {
            self.chunked_request::<KeywordShardData>("/keywords", kv_keys)
                .await
        }
    }

    pub async fn get_documents_kv_keys(&self, kv_keys: Vec<&str>) -> Vec<Document> {
        let doc_chunk_limit = get_document_limit();
        if kv_keys.len() < doc_chunk_limit as usize {
            let futures: Vec<_> = kv_keys
                .iter()
                .map(async |kv_key| Document::read(kv_key, self.store).await.unwrap())
                .collect();

            join_all(futures).await
        } else {
            self.chunked_request::<Document>("/documents", kv_keys)
                .await
        }
    }
}

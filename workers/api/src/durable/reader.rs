use std::sync::Arc;

use futures::future::join_all;
use worker::{kv::KvStore, *};

use crate::{
    data::{encoding::LengthPrefixed, keyword_shard::get_n_shards},
    util::kv::get_kv_data_store_from_env,
};

trait DurableReaderInterface {
    async fn get_documents(store: &KvStore, doc_ids: Vec<&str>) -> Vec<Vec<u8>>;
    async fn get_keywords(store: &KvStore, keywords: Vec<&str>) -> Vec<Vec<u8>>;
}

fn length_prefix_data(data: &[u8], output: &mut Vec<u8>) -> LengthPrefixed {
    let size = data.len() as u32;
    output.extend_from_slice(&size.to_le_bytes());
    output.extend_from_slice(&data);
    LengthPrefixed {
        bytes: output.clone(),
    }
}
fn parse_body<'a>(body: &'a str) -> Vec<&'a str> {
    body.split(',').filter(|s| !s.trim().is_empty()).collect()
}
/// A hard limit for the maximum number of keywords that can be requested
/// within a single durable reader request. This is to prevent panics
/// if the KV limit is hit.
pub fn get_keyword_limit(n_shards: u32) -> u32 {
    1_000u32 / n_shards
}

pub fn get_document_limit() -> u32 {
    990u32
}

pub fn get_durable_reader_namespace(
    env: &worker::Env,
) -> std::result::Result<worker::ObjectNamespace, worker::Error> {
    env.durable_object(DurableReader::BINDING_ID)
}

#[durable_object]
pub struct DurableReader {
    store: Arc<worker::kv::KvStore>,
    n_shards: u32,
}

impl DurableReader {
    pub const BINDING_ID: &'static str = "READER";
}

impl DurableReaderInterface for DurableReader {
    async fn get_documents(store: &KvStore, doc_ids: Vec<&str>) -> Vec<Vec<u8>> {
        let futures: Vec<_> = doc_ids
            .iter()
            .map(async |doc_kw| {
                store
                    .get(doc_kw)
                    .bytes()
                    .await
                    .unwrap_or(Some(vec![]))
                    .unwrap()
            })
            .collect();

        join_all(futures).await
    }

    async fn get_keywords(store: &KvStore, keywords: Vec<&str>) -> Vec<Vec<u8>> {
        let keyword_data_futures: Vec<_> = keywords
            .iter()
            .map(async |kv_id| store.get(kv_id).bytes().await)
            .collect();

        join_all(keyword_data_futures)
            .await
            .into_iter()
            .filter_map(|res| res.ok().flatten())
            .collect::<Vec<Vec<u8>>>()
    }
}

impl DurableObject for DurableReader {
    fn new(_state: State, env: Env) -> Self {
        let n_shards = get_n_shards(&env);
        let store = get_kv_data_store_from_env(&env);
        DurableReader {
            store: store,
            n_shards,
        }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        match req.method() {
            Method::Post => match req.path().as_str() {
                "/keywords" => {
                    let text = req.text().await.unwrap();
                    let entries = parse_body(text.as_str());
                    if entries.len() as u32 > get_keyword_limit(self.n_shards) {
                        return Response::error(
                            &format!(
                                "Too many keywords requested. Current limit: {}",
                                get_keyword_limit(self.n_shards)
                            ),
                            400,
                        );
                    } else if entries.is_empty() {
                        return Response::error("No keywords provided", 400);
                    }

                    let keyword_docs = Self::get_keywords(&self.store, entries).await;
                    let body_sizes = keyword_docs.iter().map(|b| b.len() as u32).sum::<u32>();
                    let mut output: Vec<u8> =
                        Vec::with_capacity((4 * keyword_docs.len()) + body_sizes as usize);
                    for doc in keyword_docs.iter() {
                        length_prefix_data(doc.as_slice(), &mut output);
                    }
                    return Response::from_bytes(output);
                }
                "/documents" => {
                    let mut req = req;
                    let text = req.text().await.unwrap();
                    let entries = parse_body(text.as_str());
                    if entries.len() as u32 > get_keyword_limit(self.n_shards) {
                        return Response::error(
                            &format!(
                                "Too many document IDs requested. Current limit: {}",
                                get_keyword_limit(self.n_shards)
                            ),
                            400,
                        );
                    } else if entries.is_empty() {
                        return Response::error("No document IDs provided", 400);
                    }

                    let doc_bodies = Self::get_documents(&self.store, entries).await;
                    let body_sizes = doc_bodies.iter().map(|b| b.len() as u32).sum::<u32>();
                    let mut output: Vec<u8> =
                        Vec::with_capacity((doc_bodies.len() * 4) + body_sizes as usize);
                    for lp in doc_bodies.iter() {
                        length_prefix_data(lp, &mut output);
                    }
                    return Response::from_bytes(output);
                }
                _ => {
                    return Response::error("Method Not Allowed", 405);
                }
            },
            _ => {
                return Response::error("Method Not Allowed", 405);
            }
        }
    }
}

use crate::data::keyword_shard::KeywordShardData;
use crate::data::DocumentRef;
use crate::data::IndexName;
use crate::data::DEFAULT_YAKE_MIN_CHARS;
use crate::data::DEFAULT_YAKE_NGRAMS;
use crate::data::PREFIX_DOCUMENT;
use crate::edge_log;
use futures::future::join_all;
use lingua::IsoCode639_1;
use nanoid::nanoid;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use worker::kv::KvStore;
use worker::Env;
use yake_rust::{Config, StopWords};

use crate::data::{DataStoreError, KvEntry, KvPersistent};

pub fn document_kv_key(index: &str, uuid: &DocumentRef) -> DocumentRef {
    format!("{}:{}{}", &index, PREFIX_DOCUMENT, &uuid)
}

/// Determine the shard for the document ID that the keyword data is stored in
pub fn shard_from_document_id(doc_id: String, num_shards: u32) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(doc_id.as_bytes());
    let hash = hasher.finalize();
    let int_hash = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
    int_hash % num_shards
}

impl KvEntry for Document {
    type Key = DocumentRef;
    fn get_kv_key(&self) -> DocumentRef {
        return document_kv_key(&self.index, &self.uuid);
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Document {
    #[serde(rename = "id")]
    uuid: DocumentRef,
    #[serde(skip)]
    pub index: IndexName,
    #[serde(rename = "rev", alias = "version")]
    pub revision: u32,
    #[serde(rename = "lang", alias = "lang")]
    pub lang: Option<IsoCode639_1>,
    #[serde(rename = "body", alias = "document_body")]
    pub document_body: Option<String>,
    #[serde(rename = "keywords", alias = "keywords")]
    pub keywords: Option<Vec<(String, f64)>>,
}

impl KvPersistent for Document {
    async fn read(key: &str, store: &KvStore) -> Result<Document, DataStoreError> {
        let result = store
            .get(&key)
            .json::<Document>()
            .await
            .map_err(DataStoreError::Kv)?
            .ok_or_else(|| DataStoreError::NotFound(key.to_string()));
        result
    }
}

static KEYWORD_DETECTOR: Lazy<lingua::LanguageDetector> =
    Lazy::new(|| lingua::LanguageDetectorBuilder::from_all_languages().build());

static STOPWORDS_CACHE: Lazy<std::collections::HashMap<String, StopWords>> = Lazy::new(|| {
    let mut map = std::collections::HashMap::new();
    // Iterate over certain IsoCode639_1 variants and pre-load their stopwords
    let iso_codes = vec![IsoCode639_1::EN];
    for code in iso_codes {
        let lang_str = code.to_string();
        map.insert(
            lang_str.clone(),
            StopWords::predefined(&lang_str.as_str()).unwrap(),
        );
    }
    map
});

fn get_yake_config_from_env(env: &Env) -> Config {
    let ngrams = env
        .var("YAKE_NGRAMS")
        .ok()
        .map(|v| v.to_string().parse::<u8>().unwrap_or(3))
        .unwrap_or(DEFAULT_YAKE_NGRAMS);
    let min_chars = env
        .var("YAKE_MINIMUM_CHARS")
        .ok()
        .map(|v| v.to_string().parse::<u8>().unwrap_or(2))
        .unwrap_or(DEFAULT_YAKE_MIN_CHARS);

    Config {
        ngrams: ngrams as usize,
        minimum_chars: min_chars as usize,
        remove_duplicates: true,
        ..Config::default()
    }
}

impl Document {
    const MAX_CUSTOM_ID_LENGTH: usize = 64;
    const MIN_CUSTOM_ID_LENGTH: usize = 1;

    pub fn get_uuid(&self) -> String {
        return self.uuid.clone();
    }

    /// Determine if the provided ID is a valid (custom)
    /// document identifier
    pub fn is_valid_id(id: &str) -> bool {
        id.len() <= Self::MAX_CUSTOM_ID_LENGTH
            && id.len() >= Self::MIN_CUSTOM_ID_LENGTH
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    pub fn new(index: &str) -> Document {
        let uuid: DocumentRef = nanoid!(16);
        return Document {
            uuid: uuid,
            index: index.to_string(),
            revision: 0u32,
            lang: None,
            keywords: None,
            document_body: None,
        };
    }

    pub fn new_with_id(index: &str, id: &str) -> Document {
        return Document {
            uuid: id.to_string(),
            index: index.to_string(),
            revision: 0u32,
            lang: None,
            keywords: None,
            document_body: None,
        };
    }

    pub async fn from_remote(
        store: &KvStore,
        index: &str,
        uuid: DocumentRef,
    ) -> Result<Document, DataStoreError> {
        let mut document = Document::read(&document_kv_key(&index, &uuid), &store).await?;
        document.index = index.to_string();
        Ok(document)
    }

    pub fn set_language(&mut self, lang: IsoCode639_1) {
        self.lang = Some(lang);
    }

    fn detect_language(content: &String) -> Option<IsoCode639_1> {
        let lang = KEYWORD_DETECTOR.detect_language_of(content)?;
        Some(lang.iso_code_639_1())
    }

    pub async fn update(
        &mut self,
        store: &KvStore,
        env: &Env,
        document_body: String,
        recalculate_lang: bool,
    ) -> Result<u32, DataStoreError> {
        // If there is no language set, try to detect it based on our new content
        if self.lang.is_none() || recalculate_lang {
            let detected_lang = Document::detect_language(&document_body);
            if detected_lang.is_some() {
                self.lang = detected_lang;
            }
        }

        let lang_str = format!("{}", &self.lang.unwrap());
        // Check if we have cached stopwords for this language
        let stopwords = if let Some(cached) = STOPWORDS_CACHE.get(&lang_str) {
            cached.clone()
        } else {
            edge_log!(
                console_warn,
                "Document",
                &self.index,
                "No cached stopwords for language {}",
                lang_str
            );
            let sw = StopWords::predefined(&lang_str.as_str()).unwrap();
            sw
        };
        let yake_config = get_yake_config_from_env(env);
        let _keywords: Vec<(String, f64)> =
            yake_rust::get_n_best(50, &document_body, &stopwords, &yake_config)
                .iter()
                .map(|item| (item.keyword.clone(), 1.0f64 - item.score))
                .collect();

        // Calculate which keywords were added/removed
        let mut kw_removed: Vec<&str> = vec![];
        let old_keywords = self.keywords.clone().unwrap_or_else(|| vec![]);
        let new_keywords = _keywords.clone();
        self.keywords = Some(_keywords);

        let new_kw_set = HashSet::from_iter(new_keywords.iter().map(|(kw, _)| kw.as_str()));
        let existing_kw_set: HashSet<&str> =
            old_keywords.iter().map(|(kw, _)| kw.as_str()).collect();

        for kw in existing_kw_set.difference(&new_kw_set) {
            kw_removed.push(kw);
        }
        self.document_body = Some(document_body);
        self.revision += 1;
        self.write(&store).await?;

        // Actually update all of the keyword shards
        let doc_id = self.uuid.clone();
        let current_keywords = self.keywords.as_ref().unwrap();

        // Collect all removal futures
        let removal_futures: Vec<_> = kw_removed
            .iter()
            .map(|removed_kw| {
                let store = &store;
                let index = &self.index;
                let doc_id = &doc_id;
                let removed_kw = removed_kw.as_ref();
                async move {
                    let mut shard =
                        KeywordShardData::from_keyword(store, env, index, doc_id, &removed_kw)
                            .await
                            .ok()
                            .unwrap();

                    edge_log!(
                        console_debug,
                        "Documents",
                        index,
                        "Removing document {} from keyword shard for keyword '{}'",
                        doc_id,
                        removed_kw
                    );
                    shard
                        .remove_document(store, doc_id)
                        .await
                        .unwrap_or_else(|_| {
                            edge_log!(
                                console_warn,
                                "Documents",
                                index,
                                "Failed to remove document {} from keyword shard for keyword '{}'",
                                doc_id,
                                removed_kw
                            );
                        });
                }
            })
            .collect();

        // Collect all addition futures
        let addition_futures: Vec<_> = current_keywords
            .iter()
            .map(|(added_kw, score)| {
                let store = &store;
                let index = &self.index;
                let doc_id = &doc_id;
                let added_kw = added_kw.clone();
                let score = *score;
                async move {
                    let mut shard =
                        KeywordShardData::from_keyword(store, env, index, doc_id, &added_kw)
                            .await
                            .ok()
                            .unwrap();

                    edge_log!(
                        console_debug,
                        "Documents",
                        index,
                        "Adding document {} to keyword shard for keyword '{}'",
                        doc_id,
                        added_kw
                    );
                    shard
                        .add_document(store, doc_id, score)
                        .await
                        .unwrap_or_else(|_| {
                            edge_log!(
                                console_warn,
                                "Documents",
                                index,
                                "Failed to add document {} to keyword shard for keyword '{}'",
                                doc_id,
                                added_kw
                            );
                        });
                }
            })
            .collect();

        join_all(removal_futures).await;
        join_all(addition_futures).await;
        Ok(self.revision)
    }
}

use crate::data::default_loaded;
use crate::data::DocumentRef;
use lingua::IsoCode639_1;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sha2::Sha256;
use worker::RouteContext;
use yake_rust::{Config, StopWords};

use crate::{
    data::{DataStoreError, KvEntry, KvPersistent, KvVersionedEntry},
    util::kv::get_kv_data_store,
};

const DOCUMENT_VERSION_V1: u8 = 1u8;

fn document_kv_key(index: String, uuid: &DocumentRef) -> DocumentRef {
    let key: String = format!("{}:doc:{}", &index, &uuid);
    key.to_owned().as_str()
}

// Determine the shard for the document ID that the keyword data is stored in
pub fn shard_from_document_id(keyword: String, num_shards: u32) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(keyword.as_bytes());
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

impl KvVersionedEntry for Document
where
    Document: KvEntry,
{
    fn get_data_version(&self) -> &u32 {
        return &self.revision;
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Document {
    uuid: DocumentRef,
    index: IndexName,
    revision: u32,
    lang: Option<IsoCode639_1>,
    document_body: Option<String>,
    keywords: Option<Vec<(String, f64)>>,
    metadata: Option<DocumentMetadata>,
    #[serde(skip_serializing, default = "default_loaded")]
    loaded: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct DocumentMetadata {
    version: u8,
    revision: u32,
    created: u64,
    updated: u64,
}

impl KvPersistent for Document {
    async fn write(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError> {
        let kv = get_kv_data_store(ctx);
        let serialized = serde_json::to_string(self).unwrap();
        kv.put(&self.get_kv_key(), serialized)
            .map_err(DataStoreError::Kv)?
            .execute()
            .await
            .map_err(DataStoreError::Kv)
    }

    async fn read(key: &str, ctx: worker::RouteContext<()>) -> Result<Document, DataStoreError> {
        let kv = get_kv_data_store(ctx);
        let result = kv
            .get(&key)
            .json::<Document>()
            .await
            .map_err(DataStoreError::Kv)?;

        result.ok_or_else(|| DataStoreError::NotFound(key.to_string()))
    }

    async fn save(&self, ctx: worker::RouteContext<()>) -> Result<(), DataStoreError> {
        self.write(ctx).await
    }
}

impl Document {
    pub fn new(index: &str) -> Document {
        let uuid: DocumentRef = nanoid!(16);
        return Document {
            uuid: uuid,
            index: index.to_string(),
            loaded: false,
            revision: 0u32,
            lang: None,
            keywords: None,
            document_body: None,
            metadata: None,
        };
    }

    pub async fn from_remote(
        ctx: worker::RouteContext<()>,
        index: &str,
        uuid: DocumentRef,
    ) -> Result<Document, DataStoreError> {
        Document::read(&document_kv_key(&index, &uuid), ctx).await
    }

    pub fn set_language(&mut self, lang: IsoCode639_1) {
        self.lang = Some(lang);
    }

    fn detect_language(content: &String) -> Option<IsoCode639_1> {
        let detector = lingua::LanguageDetectorBuilder::from_all_languages().build();
        let lang = detector.detect_language_of(content)?;
        Some(lang.iso_code_639_1())
    }

    pub async fn update(
        &mut self,
        ctx: RouteContext<()>,
        document_body: String,
        recalculate_lang: bool,
    ) -> Result<(), DataStoreError> {
        // If there is no language set, try to detect it based on our new content
        if self.lang.is_none() || recalculate_lang {
            let detected_lang = Document::detect_language(&document_body);
            if detected_lang.is_some() {
                self.lang = detected_lang;
            }
        }

        let lang_str = format!("{}", &self.lang.unwrap());
        let stopwords = StopWords::predefined(&lang_str.as_str()).unwrap();
        let yake_config = Config {
            ngrams: 3,
            minimum_chars: 2,
            remove_duplicates: true,
            ..Config::default()
        };

        let keywords = yake_rust::get_n_best(
            50,
            &self.document_body.as_ref().unwrap(),
            &stopwords,
            &yake_config,
        )
        .iter()
        .map(|item| (item.keyword.clone(), item.score))
        .collect();

        // Update data and increment revision
        self.keywords = Some(keywords);
        self.document_body = Some(document_body);
        self.revision += 1;

        self.write(ctx).await
    }
}

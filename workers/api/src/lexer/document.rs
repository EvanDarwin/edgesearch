use lingua::IsoCode639_1;
use once_cell::sync::Lazy;
use worker::Env;
use yake_rust::{Config, StopWords};

use crate::{
    data::{DocumentScore, DEFAULT_YAKE_MIN_CHARS, DEFAULT_YAKE_NGRAMS},
    edge_log,
};

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

pub struct DocumentLexer<'a> {
    env: &'a Env,
    body: &'a str,
}

impl<'a> DocumentLexer<'a> {
    pub fn new(env: &'a Env, body: &'a str) -> Self {
        DocumentLexer { env, body: body }
    }

    pub fn try_string(&self, lang: &str) -> Option<Vec<DocumentScore>> {
        let stopwords = if let Some(cached) = STOPWORDS_CACHE.get(lang) {
            cached.clone()
        } else {
            edge_log!(
                console_warn,
                "Document",
                "",
                "No cached stopwords for language {}",
                lang
            );
            let sw = StopWords::predefined(&lang);
            sw.unwrap()
        };
        let yake_config = get_yake_config_from_env(self.env);
        let _keywords: Vec<(String, f64)> =
            yake_rust::get_n_best(50, &self.body, &stopwords, &yake_config)
                .iter()
                .map(|item| (item.keyword.clone(), 1.0f64 - item.score))
                .collect();

        Some(_keywords)
    }

    pub fn try_json<'j>(&self, lang: &str) -> Option<Vec<DocumentScore<'j>>> {
        let parsed_json: serde_json::Value = serde_json::from_str(self.body).ok()?;

        let mut cleaned_str = String::new();
        self.extract_text_json(&parsed_json, &mut cleaned_str);

        // Create a temporary DocumentLexer with the cleaned string
        let temp_lexer = DocumentLexer {
            env: self.env,
            body: &cleaned_str,
        };
        temp_lexer.try_string(lang)
    }

    // Deeply iterate through each JSON Value and extract text nodes
    fn extract_text_json(&self, value: &serde_json::Value, acc: &mut String) {
        match value {
            serde_json::Value::String(s) => {
                acc.push_str(s);
                acc.push('\n');
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    self.extract_text_json(item, acc);
                }
            }
            serde_json::Value::Object(map) => {
                for (_key, val) in map {
                    self.extract_text_json(val, acc);
                }
            }
            _ => {}
        }
    }
}

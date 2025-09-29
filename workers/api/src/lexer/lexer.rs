use std::{collections::HashMap, sync::Arc};

use futures::future::join_all;
use worker::kv::KvStore;

use crate::{
    data::keyword::KeywordManager,
    edge_log,
    http::search::SearchResultRow,
    lexer::{
        scoring::score_collective_keywords,
        tokenizer::{StringTokenizer, Tokenable},
        DocumentMatches, Expr, KeywordCache, QueryError, Token,
    },
};

///
/// QueryLexer is responsible for parsing search queries, but also executes search
/// queries for documents matching the keywords in the query.
///
/// This struct handles the complete pipeline from raw inputs into search results by:
/// 1. Parses the input query string into AST
/// 2. Iterate through the AST and collect matching documents for each keyword
/// 3. `AND` / `OR` / `NOT` merges the document sets recursively
/// 4. Returns the final set of matching documents with individual keyword scores
///
pub struct QueryLexer<'a> {
    /// The parsed Abstract Syntax Tree representation of the query
    ast: Expr,
    /// Reference to the KV store for retrieving keyword data
    store: &'a Arc<KvStore>,
    /// Current query execution results
    result: DocumentMatches,
    /// Cache of keyword data to avoid repeated KV store lookups
    kw_cache: KeywordCache,
}

impl<'a> QueryLexer<'a> {
    /// Create a new QueryLexer a precompiled query
    pub fn new(ast: Expr, store: &'a Arc<KvStore>) -> Result<QueryLexer<'a>, QueryError> {
        Ok(QueryLexer {
            ast,
            store,
            result: HashMap::new(),
            kw_cache: HashMap::new(),
        })
    }

    /// Create a new [`QueryLexer`] through tokenization of a raw query string
    pub fn from_str(query: &str, store: &'a Arc<KvStore>) -> Result<QueryLexer<'a>, QueryError> {
        let tokens = StringTokenizer::tokenize(query)?;
        let ast = StringTokenizer::parse(tokens).unwrap();
        Self::new(ast, store)
    }

    /// Collect all [`Expr::Word`] keywords from the AST and turn them into a list of keyword strings
    pub fn collect_keywords(expr: &Expr) -> Vec<&str> {
        match expr {
            Expr::Word(word) => vec![word.as_str()],
            Expr::Not(inner) => Self::collect_keywords(inner),
            Expr::And(left, right) | Expr::Or(left, right) => {
                let mut keywords = Self::collect_keywords(left);
                keywords.extend(Self::collect_keywords(right));
                keywords
            }
        }
    }

    /// Using the query AST provided during construction, execute the query recursively
    /// against the provided index and keyword shards in the KV store.
    pub async fn query(&mut self, index: &str) -> Vec<SearchResultRow> {
        // Cleanup and preload keyword data
        self.kw_cache.clear();
        self.result.clear();
        self.preload_keyword_data(index).await;

        let ast_str = format!("{}", &self.ast);
        edge_log!(console_debug, "QueryLexer", index, "AST={}", ast_str);

        self.filter_documents_on_query(index, self.ast.clone())
            .iter()
            .map(move |(doc_id, kw_matches)| SearchResultRow {
                doc_id: doc_id.to_string(),
                score: score_collective_keywords(kw_matches),
                keywords: kw_matches
                    .iter()
                    .map(|(kw, score)| (kw.clone(), *score))
                    .collect(),
                body: None, // document body is not fetched in the QueryLexer
            })
            .collect::<Vec<SearchResultRow>>()
    }

    /// Retrieves the keywords for all possible keywords in the query, generating a cache
    /// and invoking a maximum of (N * N_SHARDS) KV reads, with a single LIST request.
    async fn preload_keyword_data(&mut self, index: &str) -> () {
        let manager = KeywordManager::new(index.to_string(), &self.store);

        // preload all keyword data in the cache
        let all_keywords = Self::collect_keywords(&self.ast);
        let keyword_futures: Vec<_> = all_keywords
            .iter()
            .filter(|kw| !self.kw_cache.contains_key(**kw))
            .map(async |kw| {
                (
                    *kw,
                    manager.merge_keyword_shards(kw.to_string()).await.unwrap(),
                )
            })
            .collect();

        let keyword_shard_data = join_all(keyword_futures).await;
        for (keyword, doc_matches) in keyword_shard_data.into_iter() {
            self.kw_cache.insert(keyword.to_string(), doc_matches);
        }
    }

    /// Steps through the AST tree and recursively merges keyword score sets into document IDs.
    fn filter_documents_on_query(
        &mut self,
        index: &str,
        expr: Expr,
    ) -> HashMap<String, Vec<(String, f64)>> {
        match expr {
            Expr::Not(inner) => {
                let inner_matches = self.filter_documents_on_query(index, *inner);
                let mut negated_result = HashMap::new();
                for (doc_id, kws) in self.result.iter() {
                    if !inner_matches.contains_key(doc_id) {
                        negated_result.insert(doc_id.clone(), kws.clone());
                    }
                }
                self.result = negated_result;
                self.result.clone()
            }
            Expr::And(left, right) => {
                let left_result = self.filter_documents_on_query(index, *left);
                let right_result = self.filter_documents_on_query(index, *right);
                let and_result = left_result
                    .iter()
                    .filter(|(doc_id, _)| right_result.contains_key(*doc_id))
                    .map(|(doc_id, kws)| {
                        let merged_kws = {
                            let mut kws = kws.clone();
                            let right_kws = right_result.get(doc_id).unwrap();
                            for (kw, score) in right_kws {
                                if !kws.iter().any(|(k, _)| k == kw) {
                                    kws.push((kw.clone(), *score));
                                }
                            }
                            kws
                        };
                        (doc_id.to_string(), merged_kws)
                    })
                    .collect::<HashMap<String, Vec<(String, f64)>>>();
                self.result = and_result;
                self.result.clone()
            }
            Expr::Or(left, right) => {
                let mut left_branch = self.filter_documents_on_query(index, *left);
                let right_branch = self.filter_documents_on_query(index, *right);
                Self::set_merge(&mut left_branch, right_branch);
                self.result = left_branch;
                self.result.clone()
            }
            Expr::Word(word) => {
                // Vec<(doc_id, score)>
                let kw_data = self.kw_cache.get(&word).unwrap();
                let to_add: HashMap<String, Vec<(String, f64)>> = kw_data
                    .iter()
                    .map(|(doc_id, score)| (doc_id.clone(), vec![(word.clone(), *score)]))
                    .collect();
                self.result = to_add;
                self.result.clone()
            }
        }
    }

    /// Merge two keyword result sets together, avoiding duplicates
    fn set_merge<T>(
        into: &mut HashMap<String, Vec<(String, T)>>,
        from: HashMap<String, Vec<(String, T)>>,
    ) where
        T: PartialEq + Copy,
    {
        // Merge items that are in `from` into `into`
        for (doc_key, item) in from {
            match into.get_mut(&doc_key) {
                Some(existing) => {
                    // Collect keywords to add to avoid borrowing conflicts
                    let mut to_add = Vec::new();
                    for (keyword, score) in item {
                        if !existing.iter().any(|(k, _)| k == &keyword) {
                            to_add.push((keyword, score));
                        }
                    }
                    existing.extend(to_add);
                }
                None => {
                    into.insert(doc_key, item);
                }
            }
        }
    }
}

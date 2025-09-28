use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    sync::Arc,
};

use futures::future::join_all;
use worker::{console_warn, kv::KvStore};

use crate::{
    data::{
        document::Document, keyword::KeywordManager, keyword_shard::KeywordShardData,
        DocumentScore, IndexName, KeywordScore, PREFIX_DOCUMENT,
    },
    edge_debug, edge_warn,
};

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Invalid token in query: {0}")]
    InvalidToken(char),
}

enum Token {
    Word(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Word(String),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Word(word) => write!(f, "{}", word),
            Expr::Not(inner) => write!(f, "~({})", inner),
            Expr::And(left, right) => write!(f, "({} && {})", left, right),
            Expr::Or(left, right) => write!(f, "({} || {})", left, right),
        }
    }
}

pub struct QueryLexer<'a> {
    ast: Option<Expr>,
    store: Arc<KvStore>,

    result: &'a mut HashMap<String, Vec<(String, f64)>>,

    // <keyword, Vec<(doc_id, score)>>
    kw_cache: HashMap<String, Vec<DocumentScore<'static>>>,
}

impl<'a> QueryLexer<'a> {
    pub fn new(query: &str, store: Arc<KvStore>) -> QueryLexer {
        let result = Box::leak(Box::new(HashMap::new()));
        let tokens = Self::tokenize(&query).unwrap();
        QueryLexer {
            ast: Self::parse(&tokens),
            store,
            result,
            kw_cache: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.kw_cache.clear();
        self.result.clear();
    }

    pub fn ast_to_str(&self) -> String {
        format!("{:?}", self.ast)
    }

    // Given a certain query express, search all keyword data, narrowing down document_ids
    pub async fn query(&mut self, index: &str) -> HashMap<String, Vec<(String, f64)>> {
        self.clear();

        // Preload keyword data
        self.preload_keyword_data(index).await;

        let ast_str = self.ast_to_str();
        edge_warn!("QueryLexer", index, "AST={}", ast_str);

        // Make sure to actually pass the reference here
        let ast = self.ast.clone().unwrap();
        self.filter_documents_on_query(index, ast)
    }

    async fn preload_keyword_data(&mut self, index: &str) -> () {
        let manager = KeywordManager::new(index.to_string(), &self.store);

        // preload all keyword data in the cache
        let all_keywords = Self::collect_keywords(self.ast.as_ref().unwrap());
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
    // New method to get all document IDs from the KV store
    fn get_all_doc_ids(&self) -> HashSet<String> {
        // Iterate over self.kv_cache and collect all unique document IDs
        let mut all_doc_ids = HashSet::new();
        for (_, scores) in self.kw_cache.iter() {
            all_doc_ids.extend(scores.iter().map(|(doc_id, _)| doc_id.clone()));
        }
        all_doc_ids
    }

    // HashMap<doc_id, Vec<(keyword, score)>>
    fn filter_documents_on_query(
        &mut self,
        index: &str,
        expr: Expr,
    ) -> HashMap<String, Vec<(String, f64)>> {
        let mut current_result = self.result.clone();

        match expr {
            Expr::Not(inner) => {
                self.result.clear();
                let inner_matches = self.filter_documents_on_query(index, *inner);
                let negated_result = current_result
                    .iter()
                    .filter(|(doc_id, kws)| !inner_matches.contains_key(*doc_id))
                    .map(|(doc_id, kws)| (doc_id.clone(), kws.clone()))
                    .collect();
                *self.result = negated_result;
                self.result.clone()
            }

            Expr::And(left, right) => {
                let left_result = self.filter_documents_on_query(index, *left);
                let right_result = self.filter_documents_on_query(index, *right);

                left_result
                    .into_iter()
                    .filter(|(doc_id, _)| right_result.contains_key(doc_id))
                    .map(|(doc_id, kws)| (doc_id, kws.clone()))
                    .collect()
            }
            Expr::Or(left, right) => {
                let mut or_branch = self.filter_documents_on_query(index, *left);
                let right_branch = self.filter_documents_on_query(index, *right);
                Self::diff_sets(&mut or_branch, right_branch, true);
                or_branch
            }
            Expr::Word(word) => {
                // Vec<(doc_id, score)>
                let kw_data = self.kw_cache.get(&word).unwrap();

                let to_add: HashMap<String, Vec<(String, f64)>> = kw_data
                    .iter()
                    .map(|(doc_id, score)| (doc_id.clone(), vec![(word.clone(), *score)]))
                    .collect();
                Self::diff_sets(&mut current_result, to_add, true);

                *self.result = current_result.clone();
                current_result
            }
        }
    }

    fn diff_sets<T>(
        into: &mut HashMap<String, Vec<(String, T)>>,
        from: HashMap<String, Vec<(String, T)>>,
        keep: bool,
    ) where
        T: PartialEq + Copy,
    {
        if !keep {
            // Remove items that are in `from` from `into`
            into.retain(|doc_id, _| !from.contains_key(doc_id));
        } else {
            // Merge items that are in `from` into `into`
            for item in from {
                if into.contains_key(&item.0) {
                    let existing = into.get_mut(&item.0).unwrap();
                    for kw in item.1 {
                        if !existing.iter().any(|(k, _)| k == &kw.0) {
                            existing.push(kw.clone());
                        }
                    }
                    continue;
                } else {
                    into.insert(item.0, item.1.clone());
                }
            }
        }
    }

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

    fn tokenize(input: &str) -> Result<Vec<Token>, QueryError> {
        let mut chars = input.chars().peekable();
        let mut tokens = Vec::new();
        while let Some(ch) = chars.next() {
            match ch {
                ' ' | '\t' | '\n' => continue,
                '(' => tokens.push(Token::LParen),
                ')' => tokens.push(Token::RParen),
                '&' => {
                    if chars.peek() != Some(&'&') {
                        Err(QueryError::InvalidToken(ch))?
                    }
                    chars.next();
                    tokens.push(Token::And);
                }
                '|' => {
                    if chars.peek() != Some(&'|') {
                        Err(QueryError::InvalidToken(ch))?
                    }
                    chars.next();
                    tokens.push(Token::Or);
                }
                '~' => tokens.push(Token::Not),
                '"' => {
                    let mut word = String::new();
                    while let Some(c) = chars.next() {
                        if c == '"' {
                            break;
                        }
                        word.push(c);
                    }
                    tokens.push(Token::Word(word));
                }
                _ => {
                    Err(QueryError::InvalidToken(ch))?
                    // Skip invalid characters or handle errors
                }
            }
        }
        Ok(tokens)
    }

    fn parse(tokens: &[Token]) -> Option<Expr> {
        let mut iter = tokens.iter().peekable();
        Self::parse_or(&mut iter)
    }

    fn parse_or(iter: &mut std::iter::Peekable<std::slice::Iter<Token>>) -> Option<Expr> {
        let mut left = Self::parse_and(iter)?;
        while let Some(Token::Or) = iter.peek() {
            iter.next();
            let right = Self::parse_and(iter)?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_and(iter: &mut std::iter::Peekable<std::slice::Iter<Token>>) -> Option<Expr> {
        let mut left = Self::parse_not(iter)?;
        while let Some(Token::And) = iter.peek() {
            iter.next();
            let right = Self::parse_not(iter)?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_not(iter: &mut std::iter::Peekable<std::slice::Iter<Token>>) -> Option<Expr> {
        if let Some(Token::Not) = iter.peek() {
            iter.next();
            let expr = Self::parse_primary(iter)?;
            return Some(Expr::Not(Box::new(expr)));
        }
        Self::parse_primary(iter)
    }

    fn parse_primary(iter: &mut std::iter::Peekable<std::slice::Iter<Token>>) -> Option<Expr> {
        match iter.next() {
            Some(Token::Word(word)) => Some(Expr::Word(word.clone())),
            Some(Token::LParen) => {
                let expr = Self::parse_or(iter)?;
                if let Some(Token::RParen) = iter.next() {
                    Some(expr)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

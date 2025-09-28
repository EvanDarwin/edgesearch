use std::{collections::HashMap, fmt::Display, sync::Arc};

use futures::future::join_all;
use worker::kv::KvStore;

use crate::{
    data::{keyword::KeywordManager, DocumentScore},
    edge_warn,
    http::search::SearchResultRow,
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
    store: &'a Arc<KvStore>,
    result: HashMap<String, Vec<(String, f64)>>,

    // <keyword, Vec<(doc_id, score)>>
    kw_cache: HashMap<String, Vec<DocumentScore<'static>>>,
}

impl<'a> QueryLexer<'a> {
    pub fn new(query: &str, store: &'a Arc<KvStore>) -> QueryLexer<'a> {
        let tokens = Self::tokenize(&query).unwrap();
        QueryLexer {
            ast: Self::parse(&tokens),
            store,
            result: HashMap::new(),
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
    pub async fn query(&mut self, index: &str) -> Vec<SearchResultRow> {
        self.clear();

        // Preload keyword data
        self.preload_keyword_data(index).await;

        let ast_str = self.ast_to_str();
        edge_warn!("QueryLexer", index, "AST={}", ast_str);

        let ast = self.ast.clone().unwrap();
        let matches = self.filter_documents_on_query(index, ast);

        // Consolidate Vec<(keyword, score)> into a single score (f64), 1.0 being best rank
        matches
            .iter()
            .map(move |(doc_id, kw_matches)| {
                let total_matches = kw_matches.len() as u32;
                let score: f64;
                if total_matches == 1u32 {
                    score = kw_matches[0].1;
                } else {
                    score = kw_matches.iter().map(|(_, score)| *score).sum::<f64>()
                        / (total_matches as f64);
                }
                let keywords = kw_matches
                    .iter()
                    .map(|(kw, score)| (kw.clone(), *score))
                    .collect();

                SearchResultRow {
                    doc_id: doc_id.to_string(),
                    score,
                    keywords,
                    body: None,
                }
            })
            .collect::<Vec<SearchResultRow>>()
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

    // HashMap<doc_id, Vec<(keyword, score)>>
    fn filter_documents_on_query(
        &mut self,
        index: &str,
        expr: Expr,
    ) -> HashMap<String, Vec<(String, f64)>> {
        match expr {
            Expr::Not(inner) => {
                let inner_matches = self.filter_documents_on_query(index, *inner);
                let negated_result: HashMap<String, Vec<(String, f64)>> = self
                    .result
                    .iter()
                    .filter(move |(doc_id, _)| !inner_matches.contains_key(doc_id.as_str()))
                    .map(|(doc_id, kws)| (doc_id.to_string(), kws.clone()))
                    .collect();
                let match_count = negated_result.len();

                edge_warn!(
                    "QueryLexer",
                    index,
                    "NOT matches={}, data={:?}",
                    match_count,
                    negated_result
                );
                self.result = negated_result;
                self.result.clone()
            }
            Expr::And(left, right) => {
                let left_result = self.filter_documents_on_query(index, *left);
                let left_count = left_result.len();

                let right_result = self.filter_documents_on_query(index, *right);
                let right_count = right_result.len();

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

                edge_warn!(
                    "QueryLexer",
                    index,
                    "AND left={}, right={}, data={:?}",
                    left_count,
                    right_count,
                    and_result
                );
                self.result = and_result;
                self.result.clone()
            }
            Expr::Or(left, right) => {
                let mut left_branch = self.filter_documents_on_query(index, *left);
                let right_branch = self.filter_documents_on_query(index, *right);
                let match_count = left_branch.len() + right_branch.len();
                Self::set_merge(&mut left_branch, right_branch);
                edge_warn!(
                    "QueryLexer",
                    index,
                    "OR matches={}, data={:?}",
                    match_count,
                    left_branch
                );
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

    fn set_merge<T>(
        into: &mut HashMap<String, Vec<(String, T)>>,
        from: HashMap<String, Vec<(String, T)>>,
    ) where
        T: PartialEq + Copy,
    {
        // Merge items that are in `from` into `into`
        for (doc_key, item) in from {
            if into.contains_key(&doc_key) {
                let existing = into.get_mut(&doc_key).unwrap();
                for (keyword, score) in item {
                    if !existing.iter().any(|(k, _)| k == &keyword) {
                        existing.push((keyword.clone(), score));
                    }
                }
                continue;
            } else {
                into.insert(doc_key, item.clone());
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

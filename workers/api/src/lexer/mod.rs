//! A query lexer that tokenizes, parses, and executes search queries based on strings.
//!
//! This module provides the core functionality for processing search queries,
//!   as well as the implementation for recursively merging and filtering document
//!   IDs with matching keywords related to the search.

use std::{collections::HashMap, fmt::Display};

/// Type alias for document matches: [`HashMap<doc_id, Vec<(keyword, score)>>`]
type DocumentMatches = HashMap<String, Vec<(String, f64)>>;

/// Type alias for keyword cache: HashMap<keyword, Vec<(doc_id, score)>>
type KeywordCache = HashMap<String, Vec<(String, f64)>>;

/// Describes an error that occurred during query parsing or execution
#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Invalid token in query: {0}")]
    InvalidToken(char),
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Unclosed quoted string")]
    UnclosedQuote,
    #[error("Empty query")]
    EmptyQuery,
    #[error("Missing closing parenthesis")]
    MissingClosingParen,
}

/// Describes an AST token in the search language
#[derive(Clone)]
pub enum Token {
    Word(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

/// Describes an expression node in the query AST
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

pub mod lexer;
pub mod scoring;
pub mod tokenizer;

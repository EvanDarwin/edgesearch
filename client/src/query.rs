use std::fmt::Display;

/// Query builder for constructing search expressions programmatically.
/// This is the inverse of the AST used in the search lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryExpr {
    /// A simple word or phrase
    Word(String),
    /// Logical NOT operation
    Not(Box<QueryExpr>),
    /// Logical AND operation  
    And(Box<QueryExpr>, Box<QueryExpr>),
    /// Logical OR operation
    Or(Box<QueryExpr>, Box<QueryExpr>),
}

impl QueryExpr {
    /// Create a word/phrase expression
    pub fn word<S: Into<String>>(word: S) -> Self {
        QueryExpr::Word(format!("{}", word.into()))
    }

    /// Create a NOT expression
    pub fn not(self) -> Self {
        QueryExpr::Not(Box::new(self))
    }

    /// Create an AND expression with another expression
    pub fn and(self, other: QueryExpr) -> Self {
        QueryExpr::And(Box::new(self), Box::new(other))
    }

    /// Create an OR expression with another expression  
    pub fn or(self, other: QueryExpr) -> Self {
        QueryExpr::Or(Box::new(self), Box::new(other))
    }

    /// Convert the expression to a query string that can be parsed by the lexer
    pub fn to_query_string(&self) -> String {
        match self {
            QueryExpr::Word(word) => {
                format!("\"{}\"", word)
            }
            QueryExpr::Not(inner) => format!("~({})", inner.to_query_string()),
            QueryExpr::And(left, right) => format!(
                "({} && {})",
                left.to_query_string(),
                right.to_query_string()
            ),
            QueryExpr::Or(left, right) => format!(
                "({} || {})",
                left.to_query_string(),
                right.to_query_string()
            ),
        }
    }
}

impl Display for QueryExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_query_string())
    }
}

/// Builder for constructing complex search queries fluently
pub struct QueryBuilder {
    expr: Option<QueryExpr>,
}

impl QueryBuilder {
    /// Create a new empty query builder
    pub fn new() -> Self {
        Self { expr: None }
    }

    /// Start with a word/phrase
    pub fn word<S: Into<String>>(word: S) -> Self {
        Self {
            expr: Some(QueryExpr::word(word)),
        }
    }

    /// Add an AND condition
    pub fn and<S: Into<String>>(mut self, word: S) -> Self {
        let new_expr = QueryExpr::word(word);
        self.expr = Some(match self.expr {
            Some(existing) => existing.and(new_expr),
            None => new_expr,
        });
        self
    }

    /// Add an AND condition with a complex expression
    pub fn and_expr(mut self, expr: QueryExpr) -> Self {
        self.expr = Some(match self.expr {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    /// Add an OR condition
    pub fn or<S: Into<String>>(mut self, word: S) -> Self {
        let new_expr = QueryExpr::word(word);
        self.expr = Some(match self.expr {
            Some(existing) => existing.or(new_expr),
            None => new_expr,
        });
        self
    }

    /// Add an OR condition with a complex expression
    pub fn or_expr(mut self, expr: QueryExpr) -> Self {
        self.expr = Some(match self.expr {
            Some(existing) => existing.or(expr),
            None => expr,
        });
        self
    }

    /// Negate the entire current expression
    pub fn not(mut self) -> Self {
        if let Some(expr) = self.expr {
            self.expr = Some(expr.not());
        }
        self
    }

    /// Build the final query expression
    pub fn build(self) -> Option<QueryExpr> {
        self.expr
    }

    /// Build and convert to query string
    pub fn to_query_string(self) -> Option<String> {
        self.expr.map(|expr| expr.to_query_string())
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_expr_word() {
        let expr = QueryExpr::word("hello");
        assert_eq!(expr.to_query_string(), "hello");
    }

    #[test]
    fn test_query_expr_quoted_word() {
        let expr = QueryExpr::word("hello world");
        assert_eq!(expr.to_query_string(), "\"hello world\"");
    }

    #[test]
    fn test_query_expr_not() {
        let expr = QueryExpr::word("hello").not();
        assert_eq!(expr.to_query_string(), "~(hello)");
    }

    #[test]
    fn test_query_expr_and() {
        let expr = QueryExpr::word("hello").and(QueryExpr::word("world"));
        assert_eq!(expr.to_query_string(), "(hello && world)");
    }

    #[test]
    fn test_query_expr_or() {
        let expr = QueryExpr::word("hello").or(QueryExpr::word("world"));
        assert_eq!(expr.to_query_string(), "(hello || world)");
    }

    #[test]
    fn test_query_expr_complex() {
        let expr = QueryExpr::word("programming")
            .and(QueryExpr::word("rust"))
            .or(QueryExpr::word("hello world").not());
        assert_eq!(
            expr.to_query_string(),
            "((programming && rust) || ~(\"hello world\"))"
        );
    }

    #[test]
    fn test_query_builder_basic() {
        let builder = QueryBuilder::word("hello");
        assert_eq!(builder.to_query_string(), Some("hello".to_string()));
    }

    #[test]
    fn test_query_builder_and() {
        let builder = QueryBuilder::word("hello").and("world");
        assert_eq!(
            builder.to_query_string(),
            Some("(hello && world)".to_string())
        );
    }

    #[test]
    fn test_query_builder_or() {
        let builder = QueryBuilder::word("hello").or("world");
        assert_eq!(
            builder.to_query_string(),
            Some("(hello || world)".to_string())
        );
    }

    #[test]
    fn test_query_builder_complex() {
        let builder = QueryBuilder::word("programming")
            .and("tutorials")
            .or_expr(QueryExpr::word("world").and(QueryExpr::word("peace")).not());

        let expected = "((programming && tutorials) || ~((world && peace)))";
        assert_eq!(builder.to_query_string(), Some(expected.to_string()));
    }

    #[test]
    fn test_query_builder_not() {
        let builder = QueryBuilder::word("hello").not();
        assert_eq!(builder.to_query_string(), Some("~(hello)".to_string()));
    }

    #[test]
    fn test_query_builder_empty() {
        let builder = QueryBuilder::new();
        assert_eq!(builder.to_query_string(), None);
    }

    #[test]
    fn test_display_trait() {
        let expr = QueryExpr::word("hello").and(QueryExpr::word("world"));
        assert_eq!(format!("{}", expr), "(hello && world)");
    }

    #[test]
    fn test_special_characters_quoting() {
        let expr = QueryExpr::word("hello && world");
        assert_eq!(expr.to_query_string(), "\"hello && world\"");

        let expr2 = QueryExpr::word("hello || world");
        assert_eq!(expr2.to_query_string(), "\"hello || world\"");

        let expr3 = QueryExpr::word("hello ~ world");
        assert_eq!(expr3.to_query_string(), "\"hello ~ world\"");

        let expr4 = QueryExpr::word("hello (world)");
        assert_eq!(expr4.to_query_string(), "\"hello (world)\"");
    }
}

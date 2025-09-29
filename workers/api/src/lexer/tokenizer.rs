use crate::lexer::{Expr, QueryError, Token};

/// Describes the input medium tokenizer
pub trait Tokenable<'a> {
    type Type;
    fn tokenize(input: Self::Type) -> Result<Vec<Token>, QueryError>;
    fn parse(tokens: Vec<Token>) -> Option<Expr>;
}

/// Processes simple strings into our search AS
///
/// Some examples of valid inputs:
///  - `"apple"`
///  - `"apple" && "banana"`
///  - `("apple" || "banana") && ~"grape"`
pub struct StringTokenizer {}
impl StringTokenizer {
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

impl<'a> Tokenable<'a> for StringTokenizer {
    type Type = &'a str;

    fn parse(tokens: Vec<Token>) -> Option<Expr> {
        let mut iter = tokens.iter().peekable();
        Self::parse_or(&mut iter)
    }

    fn tokenize(input: Self::Type) -> Result<Vec<Token>, QueryError> {
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
                    let mut found_closing_quote = false;
                    while let Some(c) = chars.next() {
                        if c == '"' {
                            found_closing_quote = true;
                            break;
                        }
                        word.push(c);
                    }
                    if !found_closing_quote {
                        return Err(QueryError::UnclosedQuote);
                    }
                    tokens.push(Token::Word(word));
                }
                _ => {
                    return Err(QueryError::InvalidToken(ch));
                }
            }
        }
        Ok(tokens)
    }
}

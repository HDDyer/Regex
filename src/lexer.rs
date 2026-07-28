use crate::error::LexError;
use logos::Logos;

/// Tokens
/// - `|`            alternation
/// - `*` `+` `?`    quantifiers (star, plus, optional)
/// - `.`             any-char wildcard
/// - `(` `)`         grouping
/// - `[` `]`         character class delimiters
/// - `^` `$`         anchors (also negation inside `[...]`)
/// - `{` `}` `,`     bounded repetition, e.g. `{2,5}`
/// - `-`              range separator inside `[...]`, e.g. `[a-z]`
/// - `\x`             escaped literal / class shorthand, e.g. `\d`, `\.`, `\n`
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(error = LexError)]
#[logos(skip r" +")]
pub enum Token {
    #[token("|")]
    Pipe,

    #[token("*")]
    Star,

    #[token("+")]
    Plus,

    #[token("?")]
    Question,

    #[token(".")]
    Dot,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token("^")]
    Caret,

    #[token("$")]
    Dollar,

    #[token("-")]
    Dash,

    #[token(",")]
    Comma,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    /// `\` followed by exactly one character, e.g. `\d`, `\.`, `\\`, `\n`.
    #[regex(r"\\.", |lex| lex.slice().chars().nth(1).unwrap())]
    Escaped(char),

    /// A literal character including numbers/letters/other
    #[regex(
        r#"[^|*+?.()\[\]^$\-,{}\\ \t\r\n]"#,
        |lex| lex.slice().chars().next().unwrap()
    )]
    Literal(char),
}

impl Token {
    pub fn is_operator(&self) -> bool {
        !matches!(self, Token::Literal(_) | Token::Escaped(_))
    }
}

impl<'a> Lexer<'a> {
    pub fn span(&self) -> std::ops::Range<usize> {
        self.inner.span()
    }
}

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>,
    source_len: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: Token::lexer(source),
            source_len: source.len(),
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.inner.next()?;
        let span = self.inner.span();

        match result {
            Ok(token) => Some(Ok(token)),
            Err(err) => {
                let slice = self.inner.slice();
                let refined: LexError = if slice == "\\" && span.end == self.source_len {
                    LexError::DanglingEscape
                } else {
                    err
                };
                Some(Err(refined))
            }
        }
    }
}

/// Lexes a string into a vector of tokens, returns an error for any unrecognized tokens.
pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).collect()
}

#[test]
fn lexes_all_operators() {
    let tokens = lex(r"|*+?.()[]^$-,{}").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Pipe,
            Token::Star,
            Token::Plus,
            Token::Question,
            Token::Dot,
            Token::LParen,
            Token::RParen,
            Token::LBracket,
            Token::RBracket,
            Token::Caret,
            Token::Dollar,
            Token::Dash,
            Token::Comma,
            Token::LBrace,
            Token::RBrace,
        ]
    );
}

#[test]
fn lexes_literals() {
    let tokens = lex(r"abc123").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Literal('a'),
            Token::Literal('b'),
            Token::Literal('c'),
            Token::Literal('1'),
            Token::Literal('2'),
            Token::Literal('3')
        ]
    );
}

#[test]
fn lexes_escaped() {
    let tokens = lex(r"\d\w\s\.\\").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Escaped('d'),
            Token::Escaped('w'),
            Token::Escaped('s'),
            Token::Escaped('.'),
            Token::Escaped('\\'),
        ]
    );
}

#[test]
fn lexes_dangling_escape() {
    let result = lex(r"abc\");
    assert!(result.is_err());
}
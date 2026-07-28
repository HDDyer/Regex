use std::{fmt, str::FromStr};

use crate::{
    derivative,
    error::ParseError,
    parser::{Node, Parser},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Regex {
    pattern: String,
    root: Node,
}

impl Regex {
    /// Lexes and parses `pattern`, producing a `Regex` ready to match
    /// against input strings.
    ///
    /// Returns a [`ParseError`] if `pattern` isn't a valid regex (unclosed
    /// groups/classes, invalid escapes, malformed `{m,n}` ranges, etc).
    pub fn new(pattern: &str) -> Result<Self, ParseError> {
        let mut tokens = Vec::new();
        let mut lexer = crate::lexer::Lexer::new(pattern);

        while let Some(result) = lexer.next() {
            match result {
                Ok(token) => tokens.push(token),
                Err(_err) => {
                    let pos = lexer.span().start;
                    let slice = pattern[pos..].chars().take(10).collect::<String>();
                    return Err(ParseError::LexError { pos, slice });
                }
            }
        }

        let root = Parser::new(tokens).parse()?;
        Ok(Self {
            pattern: pattern.to_string(),
            root,
        })
    }

    /// Returns `true` if `input` matches this regex in its entirety.
    pub fn is_match(&self, input: &str) -> bool {
        derivative::is_match(&self.root, input)
    }

    /// The original pattern string this regex was compiled from.
    pub fn as_str(&self) -> &str {
        &self.pattern
    }
}

/// `"a*b".parse::<Regex>()` as an alternative to `Regex::new`.
impl FromStr for Regex {
    type Err = ParseError;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        Regex::new(pattern)
    }
}

impl fmt::Display for Regex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

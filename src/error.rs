use crate::lexer::Token;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Default)]
pub enum LexError {
    #[default]
    #[error("unrecognized token")]
    UnrecognizedToken,

    #[error("dangling escape character `\\` at end of input")]
    DanglingEscape,

    #[error("invalid character encoding in input")]
    InvalidChar,
}

#[derive(Error, Debug, Clone, PartialEq, Default)]
pub enum ParseError {
    #[error("unrecognized token at position {pos}: {slice:?}")]
    LexError { pos: usize, slice: String },

    #[error("unexpected token: {0:?}")]
    UnexpectedToken(Token),

    #[error("unexpected end of input")]
    UnexpectedEndOfInput,

    #[error("unclosed parenthesis at position {open_pos}")]
    UnclosedParen { open_pos: usize },

    #[error("empty group at position {pos}")]
    EmptyGroup { pos: usize },

    #[error("unclosed character class at position {open_pos}")]
    UnclosedClass { open_pos: usize },

    #[error("empty character class at position {pos}")]
    EmptyClass { pos: usize },

    #[error("invalid range in character class: {start}..{end}")]
    InvalidRange { start: char, end: char },

    #[error("invalid range in repeat: {min},{max}")]
    InvalidRepeatRange { min: u32, max: u32 },

    #[error("unclosed brace at position {open_pos}")]
    UnclosedBrace { open_pos: usize },

    #[error("unknown escape sequence: \\{0}")]
    UnknownEscape(char),

    #[error("Unexpected Number Overflow")]
    NumberOverflow,

    #[error("Unreachable")]
    Unreachable,

    #[default]
    #[error("generic parse error")]
    Generic,
}

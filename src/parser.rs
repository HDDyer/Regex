use crate::error::ParseError;
use crate::lexer::Token;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Empty,   // ∅
    Epsilon, // ε

    ///  character literal
    Literal(char),
    /// `.` any character wildcard
    AnyChar,
    /// concat of nodes
    Concat(Vec<Node>),
    /// `a|b`
    Alt(Rc<Node>, Rc<Node>),
    /// `a*`
    Star(Rc<Node>),
    /// `a+`
    Plus(Rc<Node>),
    /// `a?`
    Optional(Rc<Node>),
    /// `a{m,n}` None means unbounded (`{m,}`)
    Repeat {
        node: Rc<Node>,
        min: u32,
        max: Option<u32>,
    },
    /// `(...)` capturing group
    Group(Rc<Node>),
    /// `[abc]`, `[^abc]`, `[a-z]`
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    /// `^`
    StartAnchor,
    /// `$`
    EndAnchor,
    /// escaped shorthand `\d`, `\w`, `\s`
    ClassShorthand(char),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassItem {
    Char(char),
    Range(char, char),
    Shorthand(char), // \d, \w, \s in a class
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn escape_to_node(c: char) -> Result<Node, ParseError> {
        match c {
            'd' | 'w' | 's' => Ok(Node::ClassShorthand(c)),
            'n' => Ok(Node::Literal('\n')),
            't' => Ok(Node::Literal('\t')),
            'r' => Ok(Node::Literal('\r')),
            // any punctuation/operator char escaped -> literal
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '^' | '$' | '-' | ',' | '{' | '}'
            | '\\' | '|' => Ok(Node::Literal(c)),
            other => Err(ParseError::UnknownEscape(other)),
        }
    }

    fn operator_token_to_char(tok: &Token) -> Option<char> {
        match tok {
            Token::Pipe => Some('|'),
            Token::Star => Some('*'),
            Token::Plus => Some('+'),
            Token::Question => Some('?'),
            Token::Dot => Some('.'),
            Token::LParen => Some('('),
            Token::RParen => Some(')'),
            Token::Caret => Some('^'),
            Token::Dollar => Some('$'),
            Token::Comma => Some(','),
            Token::LBrace => Some('{'),
            Token::RBrace => Some('}'),
            Token::LBracket => Some('['),
            _ => None,
        }
    }
}

impl Parser {
    pub fn parse(&mut self) -> Result<Node, ParseError> {
        let node = self.alternation()?;
        if let Some(tok) = self.peek() {
            return Err(ParseError::UnexpectedToken(tok.clone()));
        }
        Ok(node)
    }

    // alternation = concat ('|' concat)
    fn alternation(&mut self) -> Result<Node, ParseError> {
        let mut left = self.concatenation()?;

        while let Some(Token::Pipe) = self.peek() {
            self.advance(); // consume |
            let right = self.concatenation()?;
            left = Node::Alt(Rc::new(left), Rc::new(right));
        }

        Ok(left)
    }

    // Concatenation = Repetition*
    fn concatenation(&mut self) -> Result<Node, ParseError> {
        let mut nodes = Vec::new();

        while let Some(tok) = self.peek() {
            match tok {
                Token::Pipe | Token::RParen => break, // ends a concat
                _ => nodes.push(self.repetition()?),
            }
        }

        match nodes.len() {
            0 => Err(ParseError::UnexpectedEndOfInput),
            1 => Ok(nodes.pop().unwrap()),
            _ => Ok(Node::Concat(nodes)),
        }
    }

    // Repetition = Atom ('*')?
    fn repetition(&mut self) -> Result<Node, ParseError> {
        let node = self.atom()?;

        if matches!(node, Node::StartAnchor | Node::EndAnchor) 
            && let Some(Token::Star | Token::Plus | Token::Question) = self.peek() {
            return Err(ParseError::UnexpectedToken(self.advance().unwrap()));
        } 
        

        match self.peek() {
            Some(Token::Star) => {
                self.advance();
                Ok(Node::Star(Rc::new(node)))
            }
            Some(Token::Plus) => {
                self.advance();
                Ok(Node::Plus(Rc::new(node)))
            }
            Some(Token::Question) => {
                self.advance();
                Ok(Node::Optional(Rc::new(node)))
            }
            Some(Token::LBrace) => {
                self.advance(); // consume {
                let open_pos = self.pos - 1;
                let (min, max) = self.parse_range()?;

                match self.advance() {
                    Some(Token::RBrace) => {
                        if let Some(max) = max
                            && (max < min)
                        {
                            return Err(ParseError::InvalidRepeatRange { min, max });
                        }

                        Ok(Node::Repeat {
                            node: Rc::new(node),
                            min,
                            max,
                        })
                    }
                    _ => Err(ParseError::UnclosedBrace { open_pos }),
                }
            }
            _ => Ok(node),
        }
    }

    // Atom = Literal | '(' Alternation ')'
    fn atom(&mut self) -> Result<Node, ParseError> {
        match self.advance() {
            Some(Token::Literal(c)) => Ok(Node::Literal(c)),
            Some(Token::Dot) => Ok(Node::AnyChar),
            Some(Token::Caret) => Ok(Node::StartAnchor),
            Some(Token::Dollar) => Ok(Node::EndAnchor),
            Some(Token::LParen) => {
                let open_pos = self.pos - 1;

                if let Some(Token::RParen) = self.peek() {
                    return Err(ParseError::EmptyGroup { pos: open_pos });
                }

                let inner = self.alternation()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(Node::Group(Rc::new(inner))),
                    _ => Err(ParseError::UnclosedParen { open_pos }),
                }
            }
            Some(Token::LBracket) => self.class(),
            Some(Token::Escaped(c)) => Self::escape_to_node(c),
            Some(other) => Err(ParseError::UnexpectedToken(other)),
            None => Err(ParseError::UnexpectedEndOfInput),
        }
    }

    // parse a class when the parser reaches a LBracket [
    // [a-z]
    fn class(&mut self) -> Result<Node, ParseError> {
        let open_pos = self.pos - 1;
        let mut negated = false;

        // classes starting with a Caret are negated
        if let Some(Token::Caret) = self.peek() { 
            negated = true;
            self.advance();
        }

        let mut items = Vec::new();

        loop {
            match self.peek() {
                None => return Err(ParseError::UnclosedClass { open_pos }),
                Some(Token::RBracket) => break,
                Some(Token::Escaped(c)) => {
                    let c = *c;
                    self.advance();
                    match Self::escape_to_node(c)? {
                        Node::ClassShorthand(c) => items.push(ClassItem::Shorthand(c)),
                        Node::Literal(c) => items.push(ClassItem::Char(c)),
                        _ => return Err(ParseError::Unreachable),
                    }
                }
                Some(Token::Literal(c)) => {
                    // get the start character 
                    // ex: [a-z] -> a
                    let start = *c;
                    self.advance();

                    // checks if there is a dash and a next character to form the end of the class-range. 
                    // Otherwise just push a single character
                    if let Some(Token::Dash) = self.peek() 
                        && let Some(Token::Literal(end)) = self.tokens.get(self.pos + 1) { 
                        let end = *end;
                        if end < start {
                            return Err(ParseError::InvalidRange { start, end });
                        }
                        self.advance(); // consume -
                        self.advance(); // consume end char
                        items.push(ClassItem::Range(start, end));
                        continue;
                    }

                    items.push(ClassItem::Char(start));
                }
                Some(Token::Dash) => {
                    self.advance();
                    items.push(ClassItem::Char('-'));
                }
                Some(tok) => {
                    if let Some(c) = Self::operator_token_to_char(tok) {
                        self.advance();
                        items.push(ClassItem::Char(c));
                    } else {
                        return Err(ParseError::UnexpectedToken(self.advance().unwrap()));
                    }
                }
            }
        }

        self.advance();

        if items.is_empty() {
            return Err(ParseError::EmptyClass { pos: open_pos });
        }

        Ok(Node::Class { negated, items })
    }
    
    // parses a repeat range
    fn parse_range(&mut self) -> Result<(u32, Option<u32>), ParseError> {
        let min = self.parse_number()?;
        let mut max = Some(min);

        // if the next character is a comma, check if a max character exists
        if let Some(Token::Comma) = self.peek() {
            self.advance();
            max = None;
            if matches!(self.peek(), Some(Token::Literal(c)) if c.is_ascii_digit()) {
                max = Some(self.parse_number()?);
            }
        }
        Ok((min, max))
    }

    // convert a character literal to a Result<u32, parseError>
    fn parse_number(&mut self) -> Result<u32, ParseError> {
        let mut digits = String::new();
        while let Some(Token::Literal(c)) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            digits.push(*c);
            self.advance();
        }
        if digits.is_empty() {
            return match self.advance() {
                Some(tok) => Err(ParseError::UnexpectedToken(tok)),
                None => Err(ParseError::UnexpectedEndOfInput),
            };
        }

        digits.parse().map_err(|_| ParseError::NumberOverflow)
    }
}

// TESTS

#[test]
fn parser_simple_regex() {
    let tokens = vec![
        Token::Literal('a'),
        Token::Star,
        Token::Literal('b'),
        Token::Pipe,
        Token::Literal('c'),
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Alt(
            Rc::new(Node::Concat(vec![
                Node::Star(Rc::new(Node::Literal('a'))),
                Node::Literal('b')
            ])),
            Rc::new(Node::Literal('c'))
        )
    );
}

#[test]
fn parser_grouped_regex() {
    let tokens = vec![
        Token::LParen,
        Token::Literal('a'),
        Token::Pipe,
        Token::Literal('b'),
        Token::RParen,
        Token::Star,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Star(Rc::new(Node::Group(Rc::new(Node::Alt(
            Rc::new(Node::Literal('a')),
            Rc::new(Node::Literal('b'))
        )))))
    );
}

#[test]
fn parser_plus_and_optional() {
    let tokens = vec![
        Token::Literal('a'),
        Token::Plus,
        Token::Literal('b'),
        Token::Question,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Concat(vec![
            Node::Plus(Rc::new(Node::Literal('a'))),
            Node::Optional(Rc::new(Node::Literal('b')))
        ])
    );
}

#[test]
fn parser_any_char() {
    let tokens = vec![
        Token::Literal('a'),
        Token::Dot,
        Token::Star,
        Token::Literal('b'),
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Concat(vec![
            Node::Literal('a'),
            Node::Star(Rc::new(Node::AnyChar)),
            Node::Literal('b'),
        ])
    );
}

#[test]
fn parser_escaped_sequences_without_consuming_next_token() {
    let tokens = vec![Token::Escaped('d'), Token::Literal('a')];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Concat(vec![Node::ClassShorthand('d'), Node::Literal('a'),])
    );
}

#[test]
fn parser_start_end_anchors() {
    let tokens = vec![Token::Caret, Token::Literal('a'), Token::Dollar];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Concat(vec![Node::StartAnchor, Node::Literal('a'), Node::EndAnchor,])
    );
}

#[test]
fn parser_nested_groups() {
    let tokens = vec![
        Token::LParen,
        Token::LParen,
        Token::Literal('a'),
        Token::Pipe,
        Token::Literal('b'),
        Token::RParen,
        Token::Literal('c'),
        Token::RParen,
        Token::Star,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Star(Rc::new(Node::Group(Rc::new(Node::Concat(vec![
            Node::Group(Rc::new(Node::Alt(
                Rc::new(Node::Literal('a')),
                Rc::new(Node::Literal('b'))
            ))),
            Node::Literal('c'),
        ])))))
    );
}

#[test]
fn parser_class_with_range() {
    let tokens = vec![
        Token::LBracket,
        Token::Literal('a'),
        Token::Dash,
        Token::Literal('z'),
        Token::RBracket,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Class {
            negated: false,
            items: vec![ClassItem::Range('a', 'z')],
        }
    );
}

#[test]
fn parser_class_with_negation_and_shorthand() {
    let tokens = vec![
        Token::LBracket,
        Token::Caret,
        Token::Escaped('d'),
        Token::RBracket,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Class {
            negated: true,
            items: vec![ClassItem::Shorthand('d')],
        }
    );
}

#[test]
fn parser_class_with_multiple_items() {
    let tokens = vec![
        Token::LBracket,
        Token::Literal('a'),
        Token::Dash,
        Token::Literal('z'),
        Token::Escaped('d'),
        Token::RBracket,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Class {
            negated: false,
            items: vec![ClassItem::Range('a', 'z'), ClassItem::Shorthand('d'),],
        }
    );
}

#[test]
fn parser_unescaped_bracket_in_class() {
    let tokens = vec![
        Token::LBracket,
        Token::Literal('a'),
        Token::LBracket,
        Token::Literal('b'),
        Token::RBracket,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Class {
            negated: false,
            items: vec![
                ClassItem::Char('a'),
                ClassItem::Char('['),
                ClassItem::Char('b')
            ],
        }
    );
}

#[test]
fn parser_repeat_with_min_max() {
    let tokens = vec![
        Token::Literal('a'),
        Token::LBrace,
        Token::Literal('2'),
        Token::Comma,
        Token::Literal('5'),
        Token::RBrace,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Repeat {
            node: Rc::new(Node::Literal('a')),
            min: 2,
            max: Some(5),
        }
    );
}

#[test]
fn parser_repeat_with_min_only() {
    let tokens = vec![
        Token::Literal('a'),
        Token::LBrace,
        Token::Literal('3'),
        Token::Comma,
        Token::RBrace,
    ];
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().unwrap();
    assert_eq!(
        ast,
        Node::Repeat {
            node: Rc::new(Node::Literal('a')),
            min: 3,
            max: None,
        }
    );
}

#[test]
fn parser_fails_on_repeated_start_anchor() {
    let tokens = vec![Token::Caret, Token::Star];
    let mut parser = Parser::new(tokens);
    let err = parser.parse().unwrap_err();
    assert_eq!(err, ParseError::UnexpectedToken(Token::Star));
}

#[test]
fn parser_fails_on_unclosed_parenthesis() {
    let tokens = vec![Token::LParen, Token::Literal('a')];
    let mut parser = Parser::new(tokens);
    let err = parser.parse().unwrap_err();
    assert_eq!(err, ParseError::UnclosedParen { open_pos: 0 });
}

#[test]
fn parser_fails_on_double_quantifier() {
    let tokens = vec![Token::Literal('a'), Token::Star, Token::Star];
    let mut parser = Parser::new(tokens);
    let err = parser.parse().unwrap_err();
    assert_eq!(err, ParseError::UnexpectedToken(Token::Star));
}

#[test]
fn parser_fails_on_empty_group() {
    let tokens = vec![Token::LParen, Token::RParen];
    let mut parser = Parser::new(tokens);
    let err = parser.parse().unwrap_err();
    assert_eq!(err, ParseError::EmptyGroup { pos: 0 });
}

#[test]
fn parser_fails_on_empty_class() {
    let tokens = vec![Token::LBracket, Token::RBracket];
    let mut parser = Parser::new(tokens);
    let err = parser.parse().unwrap_err();
    assert_eq!(err, ParseError::EmptyClass { pos: 0 });
}

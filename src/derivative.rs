use crate::parser::{ClassItem, Node};
use std::rc::Rc;

impl Node {
    pub fn is_nullable(&self) -> bool {
        match self {
            Node::Empty => false,
            Node::Epsilon => true,

            Node::Literal(_) => false,
            Node::AnyChar => false,
            Node::Class { .. } => false,
            Node::ClassShorthand(_) => false,

            Node::Concat(nodes) => nodes.iter().all(Node::is_nullable),
            Node::Alt(a, b) => a.is_nullable() || b.is_nullable(),

            Node::Star(_) => true,
            Node::Plus(a) => a.is_nullable(),
            Node::Optional(_) => true,

            Node::Repeat { min, .. } => *min == 0,

            Node::Group(inner) => inner.is_nullable(),

            Node::StartAnchor => true,
            Node::EndAnchor => true,
        }
    }

    /// Computes the Brzozowski derivative of this node with respect to a
    /// single input character `c`.
    pub fn derivative(&self, c: char) -> Node {
        match self {
            Node::Empty => Node::Empty,
            Node::Epsilon => Node::Empty,

            Node::Literal(lit) => {
                if *lit == c {
                    Node::Epsilon
                } else {
                    Node::Empty
                }
            }

            Node::AnyChar => Node::Epsilon,

            Node::Class { negated, items } => {
                let hit = items.iter().any(|item| class_item_matches(item, c));
                if hit != *negated {
                    Node::Epsilon
                } else {
                    Node::Empty
                }
            }

            Node::ClassShorthand(kind) => {
                if shorthand_matches(*kind, c) {
                    Node::Epsilon
                } else {
                    Node::Empty
                }
            }

            Node::Concat(nodes) => derive_concat(nodes, c),

            Node::Alt(a, b) => simplify(Node::Alt(
                Rc::new(a.derivative(c)),
                Rc::new(b.derivative(c)),
            )),

            // d/dc(a*) = d(a,c) · a*
            Node::Star(a) => simplify(Node::Concat(vec![a.derivative(c), Node::Star(a.clone())])),

            // a+ == a a*, derives the same way.
            Node::Plus(a) => simplify(Node::Concat(vec![a.derivative(c), Node::Star(a.clone())])),

            // a? == a|ε, and d(ε,c) = ∅, reduces to d(a,c).
            Node::Optional(a) => a.derivative(c),

            Node::Repeat { node, min, max } => derive_repeat(node, *min, *max, c),

            Node::Group(inner) => inner.derivative(c),

            Node::StartAnchor => Node::Empty,
            Node::EndAnchor => Node::Empty,
        }
    }
}

fn derive_concat(nodes: &[Node], c: char) -> Node {
    match nodes {
        [] => Node::Empty,
        [only] => only.derivative(c),
        [first, rest @ ..] => {
            let head_deriv = simplify(Node::Concat(vec![
                first.derivative(c),
                Node::Concat(rest.to_vec()),
            ]));
            if first.is_nullable() {
                simplify(Node::Alt(
                    Rc::new(head_deriv),
                    Rc::new(derive_concat(rest, c)),
                ))
            } else {
                head_deriv
            }
        }
    }
}

// Computes derivative of a pattern. Consumes one char, updates bounds, concats tail
fn derive_repeat(node: &Rc<Node>, min: u32, max: Option<u32>, c: char) -> Node {
    if min == 0 && max == Some(0) {
        return Node::Empty;
    }

    let next_min = min.saturating_sub(1);
    let next_max = max.map(|m| m.saturating_sub(1));
    let tail = Node::Repeat {
        node: node.clone(),
        min: next_min,
        max: next_max,
    };

    simplify(Node::Concat(vec![node.derivative(c), tail]))
}

/// Does a single character-class item match `ch`?
fn class_item_matches(item: &ClassItem, ch: char) -> bool {
    match item {
        ClassItem::Char(literal) => *literal == ch,
        ClassItem::Range(start, end) => *start <= ch && ch <= *end,
        ClassItem::Shorthand(kind) => shorthand_matches(*kind, ch),
    }
}

/// checks if char satisfies the \d, \w, or \s shorthand class kind
fn shorthand_matches(kind: char, ch: char) -> bool {
    match kind {
        'd' => ch.is_ascii_digit(),
        'w' => ch.is_alphanumeric() || ch == '_',
        's' => ch.is_whitespace(),
        _ => false,
    }
}

/// Recovers an owned `Node` from an `Rc<Node>`, to avoid a clone
fn unwrap_rc(rc: Rc<Node>) -> Node {
    Rc::try_unwrap(rc).unwrap_or_else(|rc| (*rc).clone())
}

/// Simplifies a Alt or Concat node to prevent unbounded growth over a long input.
/// If it is another node type, it returns it unchanged.
fn simplify(node: Node) -> Node {
    match node {
        Node::Alt(a, b) => {
            // ∅ | R and R | ∅ collapse to R.
            if matches!(*a, Node::Empty) {
                unwrap_rc(b)
            } else if matches!(*b, Node::Empty) {
                unwrap_rc(a)
            } else if a == b {
                unwrap_rc(a)
            } else {
                Node::Alt(a, b)
            }
        }

        Node::Concat(parts) => {
            if parts.iter().any(|p| matches!(p, Node::Empty)) {
                return Node::Empty;
            }

            let mut flat = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    Node::Epsilon => {}
                    Node::Concat(inner) => flat.extend(inner),
                    other => flat.push(other),
                }
            }

            match flat.len() {
                0 => Node::Epsilon,
                1 => flat.pop().unwrap(),
                _ => Node::Concat(flat),
            }
        }

        other => other,
    }
}

/// Matches `input` against a parsed regex `node` by folding a Brzozowski
/// derivative over every character, then checking nullability at the end.
pub fn is_match(node: &Node, input: &str) -> bool {
    let mut current = node.clone();

    for c in input.chars() {
        current = current.derivative(c);
        if current == Node::Empty {
            return false;
        }
    }

    current.is_nullable()
}

#[test]
fn matches_simple_literal_concat() {
    // abc
    let node = Node::Concat(vec![
        Node::Literal('a'),
        Node::Literal('b'),
        Node::Literal('c'),
    ]);
    assert_eq!(is_match(&node, "abc"), true);
    assert_eq!(is_match(&node, "abd"), false);
    assert_eq!(is_match(&node, "ab"), false);
    assert_eq!(is_match(&node, "abcd"), false);
}

#[test]
fn matches_alternation() {
    // a|b
    let node = Node::Alt(Rc::new(Node::Literal('a')), Rc::new(Node::Literal('b')));
    assert_eq!(is_match(&node, "a"), true);
    assert_eq!(is_match(&node, "b"), true);
    assert_eq!(is_match(&node, "c"), false);
    assert_eq!(is_match(&node, ""), false);
}

#[test]
fn matches_star() {
    // a*
    let node = Node::Star(Rc::new(Node::Literal('a')));
    assert_eq!(is_match(&node, ""), true);
    assert_eq!(is_match(&node, "a"), true);
    assert_eq!(is_match(&node, "aaaaa"), true);
    assert_eq!(is_match(&node, "aaab"), false);
}

#[test]
fn matches_plus() {
    // a+
    let node = Node::Plus(Rc::new(Node::Literal('a')));
    assert_eq!(is_match(&node, ""), false);
    assert_eq!(is_match(&node, "a"), true);
    assert_eq!(is_match(&node, "aaa"), true);
    assert_eq!(is_match(&node, "aab"), false);
}

#[test]
fn matches_optional() {
    // ab?c
    let node = Node::Concat(vec![
        Node::Literal('a'),
        Node::Optional(Rc::new(Node::Literal('b'))),
        Node::Literal('c'),
    ]);
    assert_eq!(is_match(&node, "ac"), true);
    assert_eq!(is_match(&node, "abc"), true);
    assert_eq!(is_match(&node, "abbc"), false);
}

#[test]
fn matches_any_char() {
    // a.c
    let node = Node::Concat(vec![Node::Literal('a'), Node::AnyChar, Node::Literal('c')]);
    assert_eq!(is_match(&node, "abc"), true);
    assert_eq!(is_match(&node, "azc"), true);
    assert_eq!(is_match(&node, "ac"), false);
}

#[test]
fn matches_class_range_and_negation() {
    // [a-z]
    let lower = Node::Class {
        negated: false,
        items: vec![ClassItem::Range('a', 'z')],
    };
    assert_eq!(is_match(&lower, "m"), true);
    assert_eq!(is_match(&lower, "M"), false);

    // [^a-z]
    let not_lower = Node::Class {
        negated: true,
        items: vec![ClassItem::Range('a', 'z')],
    };
    assert_eq!(is_match(&not_lower, "M"), true);
    assert_eq!(is_match(&not_lower, "m"), false);
}

#[test]
fn matches_shorthand_classes() {
    assert_eq!(is_match(&Node::ClassShorthand('d'), "5"), true);
    assert_eq!(is_match(&Node::ClassShorthand('d'), "x"), false);

    assert_eq!(is_match(&Node::ClassShorthand('w'), "_"), true);
    assert_eq!(is_match(&Node::ClassShorthand('s'), " "), true);
    assert_eq!(is_match(&Node::ClassShorthand('s'), "x"), false);
}

#[test]
fn matches_exact_and_bounded_repeat() {
    // a{2,3}
    let node = Node::Repeat {
        node: Rc::new(Node::Literal('a')),
        min: 2,
        max: Some(3),
    };
    assert_eq!(is_match(&node, "a"), false);
    assert_eq!(is_match(&node, "aa"), true);
    assert_eq!(is_match(&node, "aaa"), true);
    assert_eq!(is_match(&node, "aaaa"), false);
}

#[test]
fn matches_unbounded_min_repeat() {
    // a{2,}
    let node = Node::Repeat {
        node: Rc::new(Node::Literal('a')),
        min: 2,
        max: None,
    };
    assert_eq!(is_match(&node, "a"), false);
    assert_eq!(is_match(&node, "aa"), true);
    assert_eq!(is_match(&node, "aaaaaaaa"), true);
}

#[test]
fn matches_groups_transparently() {
    // (ab)+
    let node = Node::Plus(Rc::new(Node::Group(Rc::new(Node::Concat(vec![
        Node::Literal('a'),
        Node::Literal('b'),
    ])))));
    assert_eq!(is_match(&node, "ab"), true);
    assert_eq!(is_match(&node, "abab"), true);
    assert_eq!(is_match(&node, "aba"), false);
}

#[test]
fn start_anchor_only_matches_at_position_zero() {
    // ^ab
    let node = Node::Concat(vec![
        Node::StartAnchor,
        Node::Literal('a'),
        Node::Literal('b'),
    ]);
    assert_eq!(is_match(&node, "ab"), true);
}

#[test]
fn end_anchor_matches_at_true_end() {
    // ab$
    let node = Node::Concat(vec![
        Node::Literal('a'),
        Node::Literal('b'),
        Node::EndAnchor,
    ]);
    assert_eq!(is_match(&node, "ab"), true);
    assert_eq!(is_match(&node, "abc"), false);
}

#[test]
fn empty_and_epsilon_base_cases() {
    assert_eq!(is_match(&Node::Empty, ""), false);
    assert_eq!(is_match(&Node::Empty, "a"), false);
    assert_eq!(is_match(&Node::Epsilon, ""), true);
    assert_eq!(is_match(&Node::Epsilon, "a"), false);
}

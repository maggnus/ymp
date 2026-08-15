#![forbid(unsafe_code)]
// Each acceptance check that includes this module uses the part of it its own reading needs; the
// rest is still compiled, and a reading that is unused today is not a defect.
#![allow(dead_code)]

//! One reading of Rust source, shared by the acceptance checks that scan the shipped product.
//!
//! A check that scans source has two ways to be wrong about what ships. It can read text, and then
//! a name hides behind a comment or a string literal. Or it can cut the file at the first test
//! marker, and then every shipped line after that point goes unexamined. This module removes both:
//! the source is scanned into words and syntactic marks with comments and literals dropped, and an
//! item is removed only where its condition cannot hold in a build of the executable.

use std::collections::BTreeSet;

/// A word is an identifier or a keyword; a mark is any other character that carries syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Element {
    Word(String),
    Mark(char),
}

#[derive(Clone, Debug)]
pub struct Token {
    pub line: usize,
    element: Element,
}

impl Token {
    pub fn word(&self) -> Option<&str> {
        match &self.element {
            Element::Word(word) => Some(word.as_str()),
            Element::Mark(_) => None,
        }
    }

    pub fn is(&self, mark: char) -> bool {
        self.element == Element::Mark(mark)
    }
}

pub fn is_word(tokens: &[Token], index: usize, word: &str) -> bool {
    tokens.get(index).and_then(Token::word) == Some(word)
}

/// Every identifier of a Rust source, with the line it appears on.
pub fn identifiers(source: &str) -> Vec<(usize, String)> {
    elements(source)
        .into_iter()
        .filter_map(|token| match token.element {
            Element::Word(word) => Some((token.line, word)),
            Element::Mark(_) => None,
        })
        .collect()
}

/// Every word and syntactic mark of a Rust source, with the line it appears on.
///
/// Comments, string and raw-string literals and character literals carry no source elements and
/// are skipped; a lifetime is not a character literal, so `'a` yields the word `a` rather than
/// swallowing the source that follows it.
pub fn elements(source: &str) -> Vec<Token> {
    let characters: Vec<char> = source.chars().collect();
    let mut found = Vec::new();
    let mut line = 1usize;
    let mut index = 0usize;

    let word = |c: char| c.is_alphanumeric() || c == '_';

    while index < characters.len() {
        let current = characters[index];
        let next = characters.get(index + 1).copied();

        if current == '\n' {
            line += 1;
            index += 1;
            continue;
        }
        if current == '/' && next == Some('/') {
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1usize;
            index += 2;
            while index < characters.len() && depth > 0 {
                match (characters[index], characters.get(index + 1).copied()) {
                    ('\n', _) => line += 1,
                    ('/', Some('*')) => {
                        depth += 1;
                        index += 1;
                    }
                    ('*', Some('/')) => {
                        depth -= 1;
                        index += 1;
                    }
                    _ => {}
                }
                index += 1;
            }
            continue;
        }
        // A raw string, but only where `r` starts a token rather than ending an identifier.
        if current == 'r'
            && matches!(next, Some('"') | Some('#'))
            && index
                .checked_sub(1)
                .is_none_or(|previous| !word(characters[previous]))
        {
            let mut hashes = 0usize;
            let mut cursor = index + 1;
            while characters.get(cursor) == Some(&'#') {
                hashes += 1;
                cursor += 1;
            }
            if characters.get(cursor) == Some(&'"') {
                cursor += 1;
                let closing: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                while cursor < characters.len() {
                    if characters[cursor] == '\n' {
                        line += 1;
                    }
                    if characters[cursor] == '"'
                        && characters[cursor..]
                            .iter()
                            .take(closing.chars().count())
                            .copied()
                            .eq(closing.chars())
                    {
                        cursor += closing.chars().count();
                        break;
                    }
                    cursor += 1;
                }
                index = cursor;
                continue;
            }
        }
        if current == '"' {
            index += 1;
            while index < characters.len() {
                match characters[index] {
                    '\\' => index += 1,
                    '\n' => line += 1,
                    '"' => break,
                    _ => {}
                }
                index += 1;
            }
            index += 1;
            continue;
        }
        if current == '\'' {
            let lifetime = next.is_some_and(|c| c.is_alphabetic() || c == '_')
                && characters.get(index + 2) != Some(&'\'');
            index += 1;
            if lifetime {
                continue;
            }
            while index < characters.len() && characters[index] != '\'' {
                if characters[index] == '\\' {
                    index += 1;
                }
                index += 1;
            }
            index += 1;
            continue;
        }
        if current.is_alphabetic() || current == '_' {
            let start = index;
            while index < characters.len() && word(characters[index]) {
                index += 1;
            }
            found.push(Token {
                line,
                element: Element::Word(characters[start..index].iter().collect()),
            });
            continue;
        }
        if !current.is_whitespace() {
            found.push(Token {
                line,
                element: Element::Mark(current),
            });
        }
        index += 1;
    }

    found
}

/// The index just past the `}` that closes the block opening at `open`.
pub fn block_end(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    let mut cursor = open;
    while cursor < tokens.len() {
        if tokens[cursor].is('{') {
            depth += 1;
        } else if tokens[cursor].is('}') {
            depth -= 1;
            if depth == 0 {
                return cursor + 1;
            }
        }
        cursor += 1;
    }
    tokens.len()
}

/// The index just past the item beginning at `cursor`: past its block, or past the `;` or `,`
/// that ends it where it has none.
pub fn item_end(tokens: &[Token], cursor: usize) -> usize {
    let mut depth = 0usize;
    let mut position = cursor;
    while position < tokens.len() {
        let token = &tokens[position];
        if token.is('(') || token.is('[') {
            depth += 1;
        } else if token.is(')') || token.is(']') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && token.is('{') {
            return block_end(tokens, position);
        } else if depth == 0 && (token.is(';') || token.is(',')) {
            return position + 1;
        }
        position += 1;
    }
    tokens.len()
}

/// The body of the outer attribute beginning at `index`, and the index just past its `]`.
fn attribute(tokens: &[Token], index: usize) -> Option<(usize, &[Token])> {
    if !tokens.get(index).is_some_and(|token| token.is('#')) {
        return None;
    }
    if !tokens.get(index + 1).is_some_and(|token| token.is('[')) {
        return None;
    }
    let mut depth = 0usize;
    let mut cursor = index + 1;
    while cursor < tokens.len() {
        if tokens[cursor].is('[') {
            depth += 1;
        } else if tokens[cursor].is(']') {
            depth -= 1;
            if depth == 0 {
                return Some((cursor + 1, &tokens[index + 2..cursor]));
            }
        }
        cursor += 1;
    }
    None
}

/// Whether an attribute body keeps its item out of every build of the product.
///
/// Only a condition that cannot hold in a build of the executable does. `cfg(test)` is such a
/// condition, and so is `cfg(all(...))` in which one term is such a condition, because `all`
/// requires every term. Nothing else is: `cfg(any(unix, test))` holds on a unix host with no test
/// build in sight, so that item ships and stays in view, and `cfg(not(test))` and every
/// unrecognised shape stay in view for the same reason. The word `test` appearing somewhere in
/// the condition is not on its own a reason to stop reading the item.
fn gates_out_of_the_product(body: &[Token]) -> bool {
    cfg_condition(body).is_some_and(only_in_a_test_build)
}

/// The condition of a `cfg(...)` attribute body.
fn cfg_condition(body: &[Token]) -> Option<&[Token]> {
    if body.first().and_then(Token::word) != Some("cfg") {
        return None;
    }
    parenthesised(body, 1)
}

/// The tokens the `(` at `open` encloses, where its `)` ends the slice.
fn parenthesised(tokens: &[Token], open: usize) -> Option<&[Token]> {
    if !tokens.get(open).is_some_and(|token| token.is('(')) {
        return None;
    }
    let mut depth = 0usize;
    for position in open..tokens.len() {
        if tokens[position].is('(') {
            depth += 1;
        } else if tokens[position].is(')') {
            depth -= 1;
            if depth == 0 {
                return (position + 1 == tokens.len()).then_some(&tokens[open + 1..position]);
            }
        }
    }
    None
}

/// Whether a `cfg` condition holds in a test build alone.
fn only_in_a_test_build(condition: &[Token]) -> bool {
    if condition.len() == 1 && condition[0].word() == Some("test") {
        return true;
    }
    if condition.first().and_then(Token::word) != Some("all") {
        return false;
    }
    parenthesised(condition, 1)
        .is_some_and(|terms| listed_terms(terms).into_iter().any(only_in_a_test_build))
}

/// The comma-separated terms of a `cfg` list, split at the commas that are not nested.
fn listed_terms(tokens: &[Token]) -> Vec<&[Token]> {
    let mut terms = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for position in 0..tokens.len() {
        if tokens[position].is('(') {
            depth += 1;
        } else if tokens[position].is(')') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && tokens[position].is(',') {
            terms.push(&tokens[start..position]);
            start = position + 1;
        }
    }
    if start < tokens.len() {
        terms.push(&tokens[start..]);
    }
    terms
}

/// The elements a build of the product compiles.
///
/// An item gated on `cfg(test)` is removed together with its body, and the source that follows it
/// is still read. Cutting the file at the first marker instead would leave every shipped line
/// after that point unexamined.
pub fn shipped_elements(source: &str) -> Vec<Token> {
    let tokens = elements(source);
    let mut shipped = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        match test_gated_item(&tokens, index) {
            Some(end) => index = end,
            None => {
                shipped.push(tokens[index].clone());
                index += 1;
            }
        }
    }
    shipped
}

/// The index just past a `cfg(test)` item beginning at `index`, where one begins there.
fn test_gated_item(tokens: &[Token], index: usize) -> Option<usize> {
    let (mut cursor, body) = attribute(tokens, index)?;
    if !gates_out_of_the_product(body) {
        return None;
    }
    while let Some((next, _)) = attribute(tokens, cursor) {
        cursor = next;
    }
    Some(item_end(tokens, cursor))
}

/// The lines on which a build of the product compiles something.
///
/// This is the reading offered to a check that matches on the text of a line rather than on
/// elements: a line carries no shipped element when it holds only a comment, only a literal, or
/// only source that a `cfg(test)` item removed, and is therefore not a line of the product. The
/// answer follows the same item removal as [`shipped_elements`], so a shipped line standing after
/// a test module is present rather than cut away with it.
pub fn shipped_lines(source: &str) -> BTreeSet<usize> {
    shipped_elements(source)
        .into_iter()
        .map(|token| token.line)
        .collect()
}

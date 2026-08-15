//! Acceptance: a command reaches durable state only where the interface reaches it, and nothing
//! the shipped binary links can begin a run without an approved contract.
//!
//! Two halves.
//!
//! The first half reads the public command surface. That surface performs the interface's actions
//! through the interface's own session. It therefore has no business naming the kernel writer, the
//! journal, the object store or the filesystem, and this check rejects a module of that surface
//! which does. It rejects the interface's input channel for the same reason: a stated value that
//! reached the input row could open a surface the command never asked for and complete a decision
//! standing behind it. The check that must fail: give `src/surface.rs` a line such as
//! `Application::create_with_contract(&root, &c)?` — or import it under any alias, since the
//! import itself names the writer — or a line that turns a stated value into `KeyCode::Char`, and
//! `the_public_command_surface_never_names_the_writer` reports the file, the line and the name
//! with a non-zero exit.
//!
//! The second half reads every workspace crate the shipped binary links, and rejects a call to a
//! constructor that begins a run without an approved contract. The set of crates it reads is
//! computed from the manifests rather than listed here: a crate is read when the executable's own
//! dependency closure reaches it, and a crate that starts a run is excused only when that closure
//! demonstrably does not. Development dependencies build tests and never the executable, so they
//! are not followed. The check that must fail: put `Application::create(&root, "run", budget)`
//! into any linked crate's shipped code — before or after a test module, under its own name or
//! under an alias — and `no_crate_the_shipped_binary_reaches_starts_a_run_without_a_contract`
//! reports the file, the line and the call with a non-zero exit.
//!
//! Both halves read the source as elements, not as text: it is scanned into words and marks with
//! comments, string literals, raw strings, character literals and lifetimes removed. A call
//! therefore cannot hide behind a substring, behind an alias (`use ymp_application::Application
//! as Kernel` binds `Kernel`, and `Kernel::create` is the same start), behind `Self` inside an
//! `impl` block that names the type, behind a rename made in another file or another crate, or
//! behind a position after a test marker.
//!
//! Two readings carry that last part. The names a rename binds are collected from every file the
//! second half reads before any call is judged, so `pub use ... as Runner` in one crate and
//! `Runner::create` in another are the same start; the table is by name, so a name bound to the
//! type anywhere in the closure denotes it everywhere in the closure, which errs towards
//! reporting. And an item is removed from the reading only where its condition cannot hold in a
//! build of the product — `cfg(test)`, or `cfg(all(...))` with such a term. `cfg(any(unix,
//! test))` ships on a unix host, so it stays in view; the file is never cut at a marker.
//!
//! What it still lets through, stated rather than implied:
//!
//! * code that is generated rather than written — a macro expansion or an `include!` — because
//!   the check reads the source as it stands, and a name a macro binds is likewise unread;
//! * a rename that passes through a glob re-export (`pub use other::*`) without naming the type,
//!   since the table follows names and not module paths;
//! * a writer reached through a name that is neither a forbidden crate nor a forbidden type, for
//!   instance a future `ymp-tui` helper that writes; that is a change to the interface's own
//!   path, which is the path this surface is required to use, and the interface's tests own it;
//! * anything in the internal namespace, which the first half deliberately excludes; the second
//!   half reads it like every other shipped module.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The modules of the public command surface. Each one is scanned.
const PUBLIC_SURFACE: [&str; 3] = ["lib.rs", "main.rs", "surface.rs"];

/// The product's own machinery, deliberately outside the first half of this check.
const INTERNAL: [&str; 1] = ["internal.rs"];

/// Names that reach durable state without the interface's session.
const WRITER: [&str; 5] = [
    "Application",
    "Journal",
    "ObjectStore",
    "ArtifactStore",
    "DataRootLock",
];

/// Crates that own durable state, run agents or judge candidates. The command surface reaches
/// all of it through the interface's session or not at all.
const KERNEL_CRATES: [&str; 13] = [
    "ymp_storage",
    "ymp_artifacts",
    "ymp_kernel",
    "ymp_verifier",
    "ymp_agent_rpc",
    "ymp_agent_mcp",
    "ymp_runtime_api",
    "ymp_runtime_claude",
    "ymp_runtime_codex",
    "ymp_runtime_fake",
    "ymp_runtime_registry",
    "ymp_runtime_supervisor",
    "ymp_testkit",
];

/// Ways to write a file directly.
const FILESYSTEM: [&str; 3] = ["fs", "File", "OpenOptions"];

/// The interface's input channel.
///
/// An argument value is a value, never a key press. The input row reads a leading slash as its
/// command line and a control character as a key of its own, so a command that turned a stated
/// value into key presses could open a surface it never asked for and complete a decision
/// standing behind it. No module of this surface may name that channel.
const KEY_CHANNEL: [&str; 6] = [
    "crossterm",
    "KeyCode",
    "KeyEvent",
    "KeyModifiers",
    "handle_key",
    "handle_event",
];

/// The crate that produces the executable a user runs.
const SHIPPED_BINARY: &str = "ymp-cli";

/// The type whose constructors begin a run.
const RUN_STARTER: &str = "Application";

/// Its constructors that begin a run carrying no approved contract.
const UNBOUND_STARTS: [&str; 2] = ["create", "create_with_config"];

/// The one scenario, which W1-APP-02m owns: the contract-bound entry that stores the contract
/// before the run begins, and the constructor's own delegation beside it. These two files are the
/// subject of the check rather than an exception to it — the check exists to keep every other
/// start out.
const SCENARIO: [&str; 2] = [
    "ymp-application/src/contract.rs",
    "ymp-application/src/lib.rs",
];

/// Crates whose presence in the closure shows that the closure was read rather than lost. A fault
/// in the manifest reader that returned an empty or truncated set would otherwise let the second
/// half pass by reading nothing.
const LINKED_CORE: [&str; 7] = [
    "ymp-cli",
    "ymp-tui",
    "ymp-application",
    "ymp-kernel",
    "ymp-storage",
    "ymp-domain",
    "ymp-runtime-supervisor",
];

fn source_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn modules() -> BTreeSet<String> {
    fs::read_dir(source_directory())
        .expect("readable source directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn every_module_of_this_crate_is_either_scanned_or_named_as_internal() {
    let declared: BTreeSet<String> = PUBLIC_SURFACE
        .iter()
        .chain(INTERNAL.iter())
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        modules(),
        declared,
        "a module was added without deciding whether the writer check reads it"
    );
}

#[test]
fn the_public_command_surface_never_names_the_writer() {
    let forbidden: BTreeSet<&str> = WRITER
        .iter()
        .chain(KERNEL_CRATES.iter())
        .chain(FILESYSTEM.iter())
        .chain(KEY_CHANNEL.iter())
        .copied()
        .collect();

    let mut offenders = Vec::new();
    for module in PUBLIC_SURFACE {
        let path = source_directory().join(module);
        let source = fs::read_to_string(&path).expect("readable module");
        for (line, identifier) in identifiers(&source) {
            if forbidden.contains(identifier.as_str()) {
                offenders.push(format!("{module}:{line}: {identifier}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the public command surface reaches durable state outside the interface's session:\n{}",
        offenders.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// The source, read as elements
// ---------------------------------------------------------------------------------------------

/// A word is an identifier or a keyword; a mark is any other character that carries syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Element {
    Word(String),
    Mark(char),
}

#[derive(Clone, Debug)]
struct Token {
    line: usize,
    element: Element,
}

impl Token {
    fn word(&self) -> Option<&str> {
        match &self.element {
            Element::Word(word) => Some(word.as_str()),
            Element::Mark(_) => None,
        }
    }

    fn is(&self, mark: char) -> bool {
        self.element == Element::Mark(mark)
    }
}

fn is_word(tokens: &[Token], index: usize, word: &str) -> bool {
    tokens.get(index).and_then(Token::word) == Some(word)
}

/// Every identifier of a Rust source, with the line it appears on.
fn identifiers(source: &str) -> Vec<(usize, String)> {
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
fn elements(source: &str) -> Vec<Token> {
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
fn block_end(tokens: &[Token], open: usize) -> usize {
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
fn item_end(tokens: &[Token], cursor: usize) -> usize {
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
fn shipped_elements(source: &str) -> Vec<Token> {
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

/// The names under which the sources read together denote the type that starts runs.
///
/// Its own name always denotes it. A `use` with `as` and a `type` alias each bind another name.
/// A binding made in one file is honoured in all of them, because a crate that re-exports the
/// type under a new name (`pub use ymp_application::Application as Runner`) hands that name to
/// every file that imports it, and reading each file on its own would lose the call. The pass
/// therefore runs over all the sources and repeats until it binds nothing new.
fn run_starter_names(sources: &[Vec<Token>]) -> BTreeSet<String> {
    let mut names = BTreeSet::from([RUN_STARTER.to_owned()]);
    loop {
        let bound = names.len();
        for tokens in sources {
            bind_aliases(tokens, &mut names);
        }
        if names.len() == bound {
            return names;
        }
    }
}

/// Every name this source binds to a name that already denotes the type that starts runs.
fn bind_aliases(tokens: &[Token], names: &mut BTreeSet<String>) {
    let mut index = 0usize;
    while index < tokens.len() {
        if is_word(tokens, index, "use") {
            let end = item_end(tokens, index);
            for position in index..end {
                let denotes = tokens[position]
                    .word()
                    .is_some_and(|word| names.contains(word));
                if denotes
                    && is_word(tokens, position + 1, "as")
                    && let Some(alias) = tokens.get(position + 2).and_then(Token::word)
                {
                    names.insert(alias.to_owned());
                }
            }
            index = end;
            continue;
        }
        if is_word(tokens, index, "type") {
            let end = item_end(tokens, index);
            let alias = tokens.get(index + 1).and_then(Token::word);
            let assigned = tokens[index + 1..end]
                .iter()
                .skip(1)
                .any(|token| token.word().is_some_and(|word| names.contains(word)));
            if let Some(alias) = alias
                && assigned
            {
                names.insert(alias.to_owned());
            }
            index = end;
            continue;
        }
        index += 1;
    }
}

/// The block ranges in which `Self` denotes the type that starts runs.
fn self_denotes_the_starter(tokens: &[Token], names: &BTreeSet<String>) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for index in 0..tokens.len() {
        if !is_word(tokens, index, "impl") {
            continue;
        }
        let Some(open) = (index..tokens.len()).find(|position| tokens[*position].is('{')) else {
            continue;
        };
        let names_the_starter = tokens[index + 1..open]
            .iter()
            .any(|token| token.word().is_some_and(|word| names.contains(word)));
        if names_the_starter {
            ranges.push((open, block_end(tokens, open)));
        }
    }
    ranges
}

/// Every call in one source's shipped code to a constructor that begins a run without a contract,
/// with the line it appears on. `names` is the table the sources bound together.
fn contractless_starts(tokens: &[Token], names: &BTreeSet<String>) -> Vec<(usize, String)> {
    let ranges = self_denotes_the_starter(tokens, names);

    let mut found = Vec::new();
    for index in 0..tokens.len() {
        let Some(word) = tokens[index].word() else {
            continue;
        };
        let denotes_starter = names.contains(word)
            || (word == "Self"
                && ranges
                    .iter()
                    .any(|(open, close)| index > *open && index < *close));
        if !denotes_starter {
            continue;
        }
        let path = tokens.get(index + 1).is_some_and(|token| token.is(':'))
            && tokens.get(index + 2).is_some_and(|token| token.is(':'));
        if !path {
            continue;
        }
        let Some(member) = tokens.get(index + 3).and_then(Token::word) else {
            continue;
        };
        if UNBOUND_STARTS.contains(&member) {
            found.push((tokens[index].line, format!("{word}::{member}")));
        }
    }
    found
}

// ---------------------------------------------------------------------------------------------
// The dependency closure of the shipped binary
// ---------------------------------------------------------------------------------------------

/// The section a manifest header opens, with the brackets of an array-of-tables removed.
fn section_header(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim()
            .to_owned(),
    )
}

/// Whether a manifest section declares crates that are linked into the package itself.
///
/// A development dependency builds tests and examples and never the executable, so it opens no
/// path a user can run.
fn links_into_the_package(section: &str) -> bool {
    section.contains("dependencies") && !section.contains("dev-dependencies")
}

/// The key a manifest line assigns to, with `name.workspace = true` reduced to `name`.
fn assigned_key(line: &str) -> Option<String> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (left, _) = line.split_once('=')?;
    let key = left.trim().split('.').next()?.trim().trim_matches('"');
    (!key.is_empty()).then(|| key.to_owned())
}

/// The crate a `package = "name"` entry renames, wherever it appears on the line.
fn renamed_package(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("package")?;
    let rest = rest.trim_start().strip_prefix('=')?;
    let (_, quoted) = rest.split_once('"')?;
    let (name, _) = quoted.split_once('"')?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// The crates a manifest links into its own build.
fn linked_dependencies(manifest: &str) -> BTreeSet<String> {
    let mut linked = BTreeSet::new();
    let mut section = String::new();
    for line in manifest.lines() {
        let line = line.trim();
        if let Some(header) = section_header(line) {
            section = header;
            if links_into_the_package(&section) {
                // `[dependencies.some-crate]` names its dependency in the header itself.
                if let Some(last) = section.rsplit('.').next()
                    && last != "dependencies"
                    && last != "build-dependencies"
                {
                    linked.insert(last.trim_matches('"').to_owned());
                }
            }
            continue;
        }
        if !links_into_the_package(&section) {
            continue;
        }
        if let Some(renamed) = renamed_package(line) {
            linked.insert(renamed);
        }
        match assigned_key(line) {
            Some(key) if key != "package" => {
                linked.insert(key);
            }
            _ => {}
        }
    }
    linked
}

/// Every crate of this workspace, by package name, with the directory that holds it.
fn workspace_crates(root: &Path) -> BTreeMap<String, PathBuf> {
    let mut crates = BTreeMap::new();
    for group in ["crates", "tools"] {
        let Ok(entries) = fs::read_dir(root.join(group)) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(manifest) = fs::read_to_string(entry.path().join("Cargo.toml")) else {
                continue;
            };
            if let Some(name) = package_name(&manifest) {
                crates.insert(name, entry.path());
            }
        }
    }
    crates
}

fn package_name(manifest: &str) -> Option<String> {
    let mut section = String::new();
    for line in manifest.lines() {
        let line = line.trim();
        if let Some(header) = section_header(line) {
            section = header;
            continue;
        }
        if section != "package" {
            continue;
        }
        if assigned_key(line).as_deref() == Some("name") {
            let (_, value) = line.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// The workspace crates the shipped executable links, directly or through another crate.
fn linked_by_the_shipped_binary(root: &Path) -> BTreeSet<String> {
    let crates = workspace_crates(root);
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut frontier = vec![SHIPPED_BINARY.to_owned()];
    while let Some(name) = frontier.pop() {
        if !reached.insert(name.clone()) {
            continue;
        }
        let Some(directory) = crates.get(&name) else {
            continue;
        };
        let manifest =
            fs::read_to_string(directory.join("Cargo.toml")).expect("readable manifest of a crate");
        for dependency in linked_dependencies(&manifest) {
            if crates.contains_key(&dependency) {
                frontier.push(dependency);
            }
        }
    }
    reached
}

/// The shipped elements of the sources the check reads, each with the path it came from, and the
/// table of names those sources bind to the type that starts runs.
///
/// The table is built from the whole set before any call is judged, so a rename made in one file
/// is recognised in the file that calls through it.
fn read_together(
    sources: &[(String, String, String)],
) -> (Vec<String>, Vec<Vec<Token>>, BTreeSet<String>) {
    let paths = sources.iter().map(|(_, path, _)| path.clone()).collect();
    let parsed: Vec<Vec<Token>> = sources
        .iter()
        .map(|(_, _, text)| shipped_elements(text))
        .collect();
    let names = run_starter_names(&parsed);
    (paths, parsed, names)
}

/// Every Rust source of every workspace crate: the crate that holds it, its path from the
/// workspace root, and its text.
fn workspace_sources(root: &Path) -> Vec<(String, String, String)> {
    let mut sources = Vec::new();
    for (name, directory) in workspace_crates(root) {
        collect(&directory.join("src"), root, &name, &mut sources);
    }
    sources.sort();
    sources
}

fn collect(
    directory: &Path,
    root: &Path,
    owner: &str,
    sources: &mut Vec<(String, String, String)>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, owner, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path).expect("readable source");
            sources.push((owner.to_owned(), relative, text));
        }
    }
}

#[test]
fn no_crate_the_shipped_binary_reaches_starts_a_run_without_a_contract() {
    let root = workspace_root();
    let linked = linked_by_the_shipped_binary(&root);
    for required in LINKED_CORE {
        assert!(
            linked.contains(required),
            "the dependency closure of the shipped binary was read as {linked:?}, which does not \
             reach {required}; the check would then read nothing and pass on an empty set"
        );
    }

    // A crate outside the closure is not excluded by declaration: the closure above shows it is
    // absent from the executable, so a start it holds opens no path a user can run. It is left
    // out of the reading entirely, aliases included, because nothing it names reaches the binary.
    let read: Vec<(String, String, String)> = workspace_sources(&root)
        .into_iter()
        .filter(|(owner, _, _)| linked.contains(owner))
        .collect();
    let (paths, parsed, names) = read_together(&read);

    let mut offenders = Vec::new();
    for (path, tokens) in paths.iter().zip(&parsed) {
        if SCENARIO.iter().any(|scenario| path.ends_with(scenario)) {
            continue;
        }
        for (line, call) in contractless_starts(tokens, &names) {
            offenders.push(format!("{path}:{line}: {call}"));
        }
    }

    assert!(
        offenders.is_empty(),
        "the shipped binary links crates that begin a run outside the contract-bound scenario:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_fixture_crate_stays_outside_the_shipped_binary() {
    let root = workspace_root();
    let linked = linked_by_the_shipped_binary(&root);
    assert!(
        !linked.contains("ymp-testkit"),
        "the fixture crate starts runs without a contract and has entered the dependency closure \
         of the shipped binary; either it returns to a development dependency or its starts go \
         through the scenario"
    );

    let fixture: Vec<(String, String, String)> = workspace_sources(&root)
        .into_iter()
        .filter(|(owner, _, _)| owner == "ymp-testkit")
        .collect();
    let (_, parsed, names) = read_together(&fixture);
    let starts_a_run = parsed
        .iter()
        .any(|tokens| !contractless_starts(tokens, &names).is_empty());
    assert!(
        starts_a_run,
        "the fixture crate no longer starts a run without a contract, so the closure above is no \
         longer what keeps that start away from the executable"
    );
}

/// The fixture runtime is a test double. Nothing the shipped executable links may reach it: a
/// driver the workspace does not attest must not be buildable — and therefore not offerable — on
/// any path a user can run. The closure is read from the manifests, so the only way to satisfy
/// this is for every linked crate to declare the fixture as a development dependency or not at
/// all. The check that must fail: return `ymp-runtime-fake` to the `[dependencies]` of any crate
/// the binary links, and this reports the closure with a non-zero exit.
#[test]
fn the_fixture_runtime_stays_outside_the_shipped_binary() {
    let root = workspace_root();
    let linked = linked_by_the_shipped_binary(&root);
    for required in LINKED_CORE {
        assert!(
            linked.contains(required),
            "the dependency closure of the shipped binary was read as {linked:?}, which does not \
             reach {required}; the check would then pass on an empty set"
        );
    }
    assert!(
        !linked.contains("ymp-runtime-fake"),
        "the fixture runtime has entered the dependency closure of the shipped binary, so a test \
         double is buildable — and offerable — on a path a user can run; it belongs in the \
         development dependencies of the crates whose checks need it. The closure was read as \
         {linked:?}"
    );
}

#[test]
fn the_scanner_reads_elements_rather_than_text() {
    let source = r####"
// Application::create in a comment is not a call.
/* Application::create in a block comment is not a call either. */
fn f<'a>(value: &'a str) -> &'a str {
    let _ = "Application::create in a string";
    let _ = r#"Application::create in a raw string"#;
    let _ = '"';
    Kernel::create(value)
}
"####;
    let names: Vec<String> = identifiers(source)
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert!(
        !names.contains(&"Application".to_owned()),
        "a comment or a literal was read as code: {names:?}"
    );
    // A lifetime must not swallow the source after it: what follows is still scanned.
    assert!(names.contains(&"Kernel".to_owned()), "{names:?}");
    assert!(names.contains(&"create".to_owned()), "{names:?}");

    // An aliased import still names the writer, so the check sees it.
    let aliased = "use ymp_application::Application as Kernel;";
    assert!(
        identifiers(aliased)
            .into_iter()
            .any(|(_, name)| name == "Application"),
        "an aliased import hid the writer"
    );
}

/// The starts of each fixture source, read as one set the way the workspace sources are read.
fn starts_of(sources: &[&str]) -> Vec<Vec<(usize, String)>> {
    let parsed: Vec<Vec<Token>> = sources
        .iter()
        .map(|source| shipped_elements(source))
        .collect();
    let names = run_starter_names(&parsed);
    parsed
        .iter()
        .map(|tokens| contractless_starts(tokens, &names))
        .collect()
}

#[test]
fn a_start_is_seen_after_a_test_marker_and_through_an_alias() {
    let source = r####"
use ymp_application::{Application as Kernel, PreparedContract};

#[cfg(test)]
mod tests {
    use ymp_application::Application;

    #[test]
    fn fixture() {
        let _ = Application::create(&root, "run-1", budget);
    }
}

fn bound(root: &Path, prepared: &PreparedContract) {
    let _ = Application::create_with_contract(root, prepared);
}

fn aliased(root: &Path) {
    let _ = Kernel::create(root, "run-1", budget);
}

#[cfg(test)]
fn helper() {
    let _ = Kernel::create_with_config(root, "run-1", budget, config);
}

type Runner = Kernel;

impl Reported for Runner {
    fn report(root: &Path) {
        let _ = Self::create_with_config(root, "run-1", budget, config);
    }
}
"####;
    let found = starts_of(&[source])[0].clone();
    let calls: Vec<&str> = found.iter().map(|(_, call)| call.as_str()).collect();

    // The call inside the test module and the call on a `cfg(test)` item are not shipped.
    assert_eq!(
        calls,
        vec!["Kernel::create", "Self::create_with_config"],
        "the shipped starts of the fixture were read as {found:?}"
    );
    // The alias sits after the first test marker; cutting the file there would hide it.
    assert!(
        found.iter().any(|(line, _)| *line > 4),
        "a start after the first test marker went unread: {found:?}"
    );
    // The contract-bound entry is the approved path and is not reported.
    assert!(
        !calls.iter().any(|call| call.contains("with_contract")),
        "the contract-bound entry was reported as a contractless start: {found:?}"
    );
}

#[test]
fn a_condition_that_still_ships_keeps_its_item_in_view() {
    // `any(unix, test)` holds on a unix host with no test build in sight, so the item ships and
    // its start must be read. Only a condition that cannot hold in a build of the product removes
    // an item from the reading.
    let ships = r####"
#[cfg(any(unix, test))]
fn on_unix(root: &Path) {
    let _ = Application::create(root, "run-1", budget);
}

#[cfg(any(test, debug_assertions))]
fn when_checked(root: &Path) {
    let _ = Application::create_with_config(root, "run-1", budget, config);
}

#[cfg(not(test))]
fn outside_a_test(root: &Path) {
    let _ = Application::create(root, "run-1", budget);
}
"####;
    let found = starts_of(&[ships])[0].clone();
    assert_eq!(
        found.len(),
        3,
        "a shipped item was removed from the reading by a condition it still satisfies: {found:?}"
    );

    // A condition that cannot hold in a build of the product does remove its item.
    let never_ships = r####"
#[cfg(test)]
fn plain(root: &Path) {
    let _ = Application::create(root, "run-1", budget);
}

#[cfg(all(test, unix))]
fn on_unix(root: &Path) {
    let _ = Application::create(root, "run-1", budget);
}

#[cfg(all(any(unix, windows), test))]
fn on_either(root: &Path) {
    let _ = Application::create(root, "run-1", budget);
}
"####;
    assert!(
        starts_of(&[never_ships])[0].is_empty(),
        "a test-only item was read as shipped: {:?}",
        starts_of(&[never_ships])[0]
    );
}

#[test]
fn an_alias_bound_in_another_file_is_carried_to_the_call() {
    let binding = "pub use ymp_application::Application as Runner;";
    let caller = r####"
use crate::exports::Runner;

fn start(root: &Path) {
    let _ = Runner::create(root, "run-1", budget);
}
"####;
    let read = starts_of(&[binding, caller]);
    assert_eq!(
        read[1]
            .iter()
            .map(|(_, call)| call.as_str())
            .collect::<Vec<&str>>(),
        vec!["Runner::create"],
        "a rename made in another file hid the call: {:?}",
        read[1]
    );
    // Read on its own, the calling file gives the name no meaning, which is the reading this
    // check no longer performs.
    assert!(
        starts_of(&[caller])[0].is_empty(),
        "the fixture no longer distinguishes the two readings"
    );
}

#[test]
fn the_manifest_reader_follows_what_is_linked_and_not_what_is_only_tested() {
    let manifest = r####"
[package]
name = "ymp-example"

[dependencies]
ymp-domain.workspace = true
renamed = { package = "ymp-kernel", path = "../ymp-kernel" }

[build-dependencies]
ymp-storage.workspace = true

[dev-dependencies]
ymp-testkit.workspace = true

[target.'cfg(unix)'.dev-dependencies]
ymp-runtime-fake.workspace = true
"####;
    assert_eq!(package_name(manifest).as_deref(), Some("ymp-example"));
    let linked = linked_dependencies(manifest);
    assert!(linked.contains("ymp-domain"), "{linked:?}");
    assert!(linked.contains("ymp-kernel"), "{linked:?}");
    assert!(linked.contains("ymp-storage"), "{linked:?}");
    assert!(!linked.contains("ymp-testkit"), "{linked:?}");
    assert!(!linked.contains("ymp-runtime-fake"), "{linked:?}");
}

//! Acceptance: a command reaches durable state only where the interface reaches it.
//!
//! The command surface performs the interface's actions through the interface's own session. It
//! therefore has no business naming the kernel writer, the journal, the object store or the
//! filesystem, and this check rejects a module of that surface which does. The check that must
//! fail: give `src/surface.rs` a line such as `Application::create_with_contract(&root, &c)?` —
//! or import it under any alias, since the import itself names the writer — and
//! `the_public_command_surface_never_names_the_writer` reports the file, the line and the name
//! with a non-zero exit.
//!
//! The source is read as elements, not as text: it is scanned into identifiers with comments,
//! string literals, raw strings, character literals and lifetimes removed, and a forbidden name
//! is an identifier equal to a forbidden name. A call cannot hide behind a substring, an alias
//! (`use ymp_application::Application as K` still contains the identifier `Application`), a glob
//! import (the call site still contains it) or a position after a test marker (the whole file is
//! scanned, including its test module).
//!
//! What it still lets through, stated rather than implied:
//!
//! * code that is generated rather than written — a macro expansion or an `include!` — because
//!   the check reads the source as it stands;
//! * a writer reached through a name that is neither a forbidden crate nor a forbidden type, for
//!   instance a future `ymp-tui` helper that writes; that is a change to the interface's own
//!   path, which is the path this surface is required to use, and the interface's tests own it;
//! * anything in the internal namespace, which this check deliberately excludes and which
//!   `ymp-application/tests/one_start_path.rs` covers for run starts across the whole workspace.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The modules of the public command surface. Each one is scanned.
const PUBLIC_SURFACE: [&str; 3] = ["lib.rs", "main.rs", "surface.rs"];

/// The product's own machinery, deliberately outside this check.
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
const KERNEL_CRATES: [&str; 12] = [
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
    "ymp_runtime_supervisor",
    "ymp_testkit",
];

/// Ways to write a file directly.
const FILESYSTEM: [&str; 3] = ["fs", "File", "OpenOptions"];

fn source_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
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

/// Every identifier of a Rust source, with the line it appears on.
///
/// Comments, string and raw-string literals and character literals carry no identifiers and are
/// skipped; a lifetime is not a character literal, so `'a` yields the identifier `a` rather than
/// swallowing the source that follows it.
fn identifiers(source: &str) -> Vec<(usize, String)> {
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
            found.push((line, characters[start..index].iter().collect()));
            continue;
        }
        index += 1;
    }

    found
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

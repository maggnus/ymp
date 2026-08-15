//! The plane boundary, checked rather than described.
//!
//! Two of the things a payload must not be able to cause — a fetch and a tool invocation — cannot
//! be established by putting an adversarial payload through the board, because a board that never
//! did either would pass such a test whether or not it could. What establishes them is that the
//! library has no way to perform one: it declares no dependency on a package that holds control or
//! verification state, and its own sources name no file, no address and no child process.
//!
//! Both checks are pure functions of text, so each is run twice: once over this crate's real
//! manifest and sources, and once over a fixture that carries exactly what the check is looking
//! for. Without the second half a passing check would only mean that nothing matched.

/// Everything this crate is allowed to depend on: serialization, hashing and error types. No ymp
/// package appears here, which is what makes a control writer, a verifier and a store unreachable
/// from this plane rather than merely unused by it.
const ALLOWED_DEPENDENCIES: [&str; 5] = ["hex", "serde", "serde_json", "sha2", "thiserror"];

/// What no source of this library may name. Each one is a way out of the process, or a way to
/// smuggle content in at compile time.
const FORBIDDEN_MARKERS: [&str; 8] = [
    "std::fs",
    "std::io",
    "std::net",
    "std::os",
    "std::process",
    "include_bytes!",
    "include_str!",
    "extern \"C\"",
];

/// The sources the library is built from. The suite's own files are deliberately not among them:
/// this file reads the manifest and those sources, which is exactly what a library file may not do.
const LIBRARY_SOURCES: [&str; 6] = [
    "lib.rs",
    "budget.rs",
    "records.rs",
    "protocol.rs",
    "ledger.rs",
    "observatory.rs",
];

/// The dependency names a manifest declares.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut declared = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[dependencies]";
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line
            .split(['=', '.', ' '])
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if !name.is_empty() {
            declared.push(name);
        }
    }
    declared
}

/// The dependencies a manifest declares that the plane boundary does not admit.
fn unadmitted_dependencies(manifest: &str) -> Vec<String> {
    declared_dependencies(manifest)
        .into_iter()
        .filter(|name| !ALLOWED_DEPENDENCIES.contains(&name.as_str()))
        .collect()
}

/// Every place a source names something the library may not reach, with the line it is on.
fn reachable_effects(name: &str, source: &str) -> Vec<String> {
    let mut findings = Vec::new();
    for (index, line) in source.lines().enumerate() {
        for marker in FORBIDDEN_MARKERS {
            if line.contains(marker) {
                findings.push(format!("{name}:{}: {marker}", index + 1));
            }
        }
    }
    findings
}

fn read(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the audited file {} could not be read: {error}",
            path.display()
        )
    })
}

/// The collaboration plane depends on no other ymp package.
///
/// The negative half puts the same check to a manifest that declares one. Without it the check
/// would pass on a crate that had just acquired a control writer.
#[test]
fn the_collaboration_plane_depends_on_no_other_ymp_package() {
    let manifest = read("Cargo.toml");
    assert_eq!(
        unadmitted_dependencies(&manifest),
        Vec::<String>::new(),
        "the collaboration plane declares a dependency the boundary does not admit; a package \
         holding control or verification state would be reachable from a board record"
    );

    let borrowed_authority = "[package]\nname = \"ymp-board\"\n\n[dependencies]\n\
                              serde.workspace = true\nymp-domain.workspace = true\n";
    assert_eq!(
        unadmitted_dependencies(borrowed_authority),
        vec!["ymp-domain".to_owned()],
        "the check does not notice a manifest that pulls the control plane into this one, so it \
         is not what keeps the two apart"
    );
}

/// No source of the library names a file, an address or a child process.
///
/// The negative half puts the same check to a source that names all three. A payload that asks for
/// a fetch or a tool call therefore reaches a library with nothing to ask, which is a stronger
/// statement than observing that one particular payload caused neither.
#[test]
fn the_collaboration_plane_reaches_no_file_no_address_and_no_process() {
    let mut findings = Vec::new();
    for source in LIBRARY_SOURCES {
        findings.extend(reachable_effects(source, &read(&format!("src/{source}"))));
    }
    assert_eq!(
        findings,
        Vec::<String>::new(),
        "a source of the collaboration plane names a way out of the process, so the plane could \
         act on what a payload asks for"
    );

    let obedient = "fn act(payload: &str) {\n    \
                    let _ = std::fs::read(payload);\n    \
                    let _ = std::net::TcpStream::connect(payload);\n    \
                    let _ = std::process::Command::new(payload);\n}\n";
    assert_eq!(
        reachable_effects("fixture.rs", obedient),
        vec![
            "fixture.rs:2: std::fs".to_owned(),
            "fixture.rs:3: std::net".to_owned(),
            "fixture.rs:4: std::process".to_owned(),
        ],
        "the check does not notice a source that opens a file, reaches an address and starts a \
         process, so it is not what establishes that this plane cannot"
    );
}

/// The suite's own files are audited as sources of the library only if they are ever compiled into
/// one, which they are not: this file names the audited sources explicitly, and every one of them
/// is a module the library declares.
#[test]
fn the_audit_covers_every_source_the_library_is_built_from() {
    let declared: Vec<String> = read("src/lib.rs")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("pub mod ")
                .and_then(|rest| rest.strip_suffix(';'))
                .map(|name| format!("{name}.rs"))
        })
        .collect();
    for module in &declared {
        assert!(
            LIBRARY_SOURCES.contains(&module.as_str()),
            "the library declares the module {module}, and the audit does not cover it"
        );
    }
    assert_eq!(
        declared.len() + 1,
        LIBRARY_SOURCES.len(),
        "the audited sources and the modules the library declares have drifted apart"
    );
}

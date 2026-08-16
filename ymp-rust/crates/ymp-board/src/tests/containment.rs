//! The plane boundary, checked rather than described.
//!
//! Two of the things a payload must not be able to cause — a fetch and a tool invocation — cannot
//! be established by putting an adversarial payload through the board, because a board that never
//! did either would pass such a test whether or not it could. What establishes them is that the
//! library has no way to perform one: it declares no dependency on a package that holds control or
//! verification state, and its own sources name no file, no address and no child process.
//!
//! One source is an exception, and it is bounded by three checks rather than by trust. The board
//! keeps durable records, so `store.rs` opens files; it is held to naming no address and no child
//! process like every other source, and additionally to building no path out of anything but the
//! three file names it declares. An identifier a participant chose therefore never becomes a path
//! component, and the widest thing a payload could ask of this crate remains nothing at all.
//!
//! Every check is a pure function of text, so each is run twice: once over this crate's real
//! manifest and sources, and once over a fixture that carries exactly what the check is looking
//! for. Without the second half a passing check would only mean that nothing matched.

/// Everything this crate is allowed to depend on: serialization, hashing, error types, and the
/// exclusive filesystem lock one writer of a board section holds. No ymp package appears here,
/// which is what makes a control writer, a verifier and a store unreachable from this plane rather
/// than merely unused by it.
const ALLOWED_DEPENDENCIES: [&str; 6] = ["fs2", "hex", "serde", "serde_json", "sha2", "thiserror"];

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

/// What the durable-records source may not name. It is the same list without the two markers a
/// file is written through: reaching the filesystem is what this one source is for, and reaching
/// an address, a child process or a compile-time inclusion is not.
const FORBIDDEN_IN_PERSISTENCE: [&str; 6] = [
    "std::net",
    "std::os",
    "std::process",
    "include_bytes!",
    "include_str!",
    "extern \"C\"",
];

/// The only path components the durable-records source may build a path from. Each is a constant
/// of this crate, so nothing an author, a scope or a payload named decides what is written where.
const ALLOWED_PATH_COMPONENTS: [&str; 3] = ["OPENING_RECORD", "FACT_RECORD", "SECTION_LOCK"];

/// The sources the kernel is built from, which reach nothing outside the process at all. The
/// suite's own files are deliberately not among them: this file reads the manifest and those
/// sources, which is exactly what a library file may not do.
const KERNEL_SOURCES: [&str; 6] = [
    "lib.rs",
    "budget.rs",
    "records.rs",
    "protocol.rs",
    "ledger.rs",
    "observatory.rs",
];

/// The sources that keep the board's durable records.
const PERSISTENCE_SOURCES: [&str; 1] = ["store.rs"];

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

/// Every place a source names something it may not reach, with the line it is on.
fn reachable_effects(name: &str, source: &str, forbidden: &[&str]) -> Vec<String> {
    let mut findings = Vec::new();
    for (index, line) in source.lines().enumerate() {
        for marker in forbidden {
            if line.contains(marker) {
                findings.push(format!("{name}:{}: {marker}", index + 1));
            }
        }
    }
    findings
}

/// Every path this source extends with something that is not one of its own constants.
///
/// The argument is read as it is written. A call that computes its component, reads it out of a
/// record or takes it from a caller is not one of the admitted constants and is reported, which is
/// the point: what may be reported is the whole question of whether a board record can name a file.
fn unconstrained_path_components(name: &str, source: &str) -> Vec<String> {
    let mut findings = Vec::new();
    for (index, line) in source.lines().enumerate() {
        for (offset, _) in line.match_indices(".join(") {
            let argument: String = line[offset + ".join(".len()..]
                .chars()
                .take_while(|character| *character != ')')
                .collect();
            let argument = argument.trim().to_owned();
            if !ALLOWED_PATH_COMPONENTS.contains(&argument.as_str()) {
                findings.push(format!("{name}:{}: {argument}", index + 1));
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

/// No source of the kernel names a file, an address or a child process.
///
/// The negative half puts the same check to a source that names all three. A payload that asks for
/// a fetch or a tool call therefore reaches a kernel with nothing to ask, which is a stronger
/// statement than observing that one particular payload caused neither.
#[test]
fn the_collaboration_kernel_reaches_no_file_no_address_and_no_process() {
    let mut findings = Vec::new();
    for source in KERNEL_SOURCES {
        findings.extend(reachable_effects(
            source,
            &read(&format!("src/{source}")),
            &FORBIDDEN_MARKERS,
        ));
    }
    assert_eq!(
        findings,
        Vec::<String>::new(),
        "a source of the collaboration kernel names a way out of the process, so the plane could \
         act on what a payload asks for"
    );

    let obedient = "fn act(payload: &str) {\n    \
                    let _ = std::fs::read(payload);\n    \
                    let _ = std::net::TcpStream::connect(payload);\n    \
                    let _ = std::process::Command::new(payload);\n}\n";
    assert_eq!(
        reachable_effects("fixture.rs", obedient, &FORBIDDEN_MARKERS),
        vec![
            "fixture.rs:2: std::fs".to_owned(),
            "fixture.rs:3: std::net".to_owned(),
            "fixture.rs:4: std::process".to_owned(),
        ],
        "the check does not notice a source that opens a file, reaches an address and starts a \
         process, so it is not what establishes that this plane cannot"
    );
}

/// The durable-records source opens files and nothing else: it reaches no address and no child
/// process, and it extends no path with anything but its own two constants.
///
/// The second half is what keeps persistence from becoming the way out the kernel does not have.
/// A record naming its own file would let a participant decide what this plane writes and where;
/// the fixture below is exactly that source, and the check reports it.
#[test]
fn the_durable_records_open_files_named_by_this_crate_and_nothing_else() {
    let mut effects = Vec::new();
    let mut components = Vec::new();
    for source in PERSISTENCE_SOURCES {
        let text = read(&format!("src/{source}"));
        effects.extend(reachable_effects(source, &text, &FORBIDDEN_IN_PERSISTENCE));
        components.extend(unconstrained_path_components(source, &text));
    }
    assert_eq!(
        effects,
        Vec::<String>::new(),
        "the durable-records source names a way out of the process beyond the filesystem it is \
         for"
    );
    assert_eq!(
        components,
        Vec::<String>::new(),
        "the durable-records source builds a path out of something other than its own constants, \
         so a board record could decide what this plane writes and where"
    );

    let obedient = "fn write(record: &MessageRecord, directory: &Path) {\n    \
                    let path = directory.join(&record.author);\n    \
                    let _ = std::net::TcpStream::connect(&record.payload_digest);\n\
                    }\n";
    assert_eq!(
        reachable_effects("fixture.rs", obedient, &FORBIDDEN_IN_PERSISTENCE),
        vec!["fixture.rs:3: std::net".to_owned()],
        "the check does not notice a records source that reaches an address"
    );
    assert_eq!(
        unconstrained_path_components("fixture.rs", obedient),
        vec!["fixture.rs:2: &record.author".to_owned()],
        "the check does not notice a records source that names a file after something a \
         participant chose, so it is not what keeps a board record out of a path"
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
            KERNEL_SOURCES.contains(&module.as_str())
                || PERSISTENCE_SOURCES.contains(&module.as_str()),
            "the library declares the module {module}, and the audit does not cover it"
        );
    }
    assert_eq!(
        declared.len() + 1,
        KERNEL_SOURCES.len() + PERSISTENCE_SOURCES.len(),
        "the audited sources and the modules the library declares have drifted apart"
    );
}

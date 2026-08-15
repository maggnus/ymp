#![forbid(unsafe_code)]

//! What a project with no tests of its own is offered, and what it is still refused.
//!
//! A request in such a project reaches a check generated from its own words. What is measured here
//! is that the generated program decides something: it rejects the project as it stands, it
//! accepts that project carrying the artifact the request asks for, and it reads the bytes of a
//! picture rather than trusting the name given to them. The refusal is measured beside it, because
//! generation that never refuses would be invention: a request naming no artifact, and a request
//! whose artifact the project already carries, both end in a refusal that says which.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tempfile::TempDir;
use ymp_application::answer::{self, AnswerError, Generated};
use ymp_application::{AcceptanceCondition, RunRequest, prepare_contract};
use ymp_domain::digest_bytes;

/// The limit a demonstration runs under here. It is generous: what is being measured is the
/// decision, and a host under load must not turn a decision into a timeout.
const WALL_LIMIT: Duration = Duration::from_secs(60);

struct Workspace {
    _root: TempDir,
    source: PathBuf,
    drafts: PathBuf,
}

/// A project as an operator has one when nothing has been done yet: a directory, and nothing in it
/// that says how tests are run.
fn workspace() -> Workspace {
    let root = TempDir::new().expect("temporary root");
    let base = root.path().canonicalize().expect("resolve root");
    let source = base.join("work");
    let drafts = base.join("drafts");
    fs::create_dir_all(&source).expect("project directory");
    Workspace {
        _root: root,
        source,
        drafts,
    }
}

impl Workspace {
    fn generate(&self, prompt: &str) -> Result<Generated, AnswerError> {
        answer::generate(prompt, &self.source, &self.drafts)
    }

    /// A directory carrying exactly the files named, judged by the generated program directly.
    fn candidate(&self, name: &str, files: &[(&str, Vec<u8>)]) -> PathBuf {
        let path = self.drafts.join(name);
        fs::create_dir_all(&path).expect("candidate directory");
        for (relative, bytes) in files {
            fs::write(path.join(relative), bytes).expect("candidate file");
        }
        path
    }
}

/// Run the generated program against one directory, as the executor runs it: the directory is the
/// only argument, and only 0 and 1 mean anything.
fn decides(program: &Path, subject: &Path) -> bool {
    let status = Command::new(program)
        .arg(subject)
        .current_dir(subject)
        .status()
        .expect("run the generated program");
    match status.code() {
        Some(0) => true,
        Some(1) => false,
        code => panic!("the generated program exited with {code:?}, which decides nothing"),
    }
}

/// A 1×1 picture in each format the generated program reads the size out of. They were produced on
/// a host with an image converter and are carried here verbatim, so what the program is measured
/// against is a picture rather than a header written to satisfy it.
const PNG: &str = "89504e470d0a1a0a0000000d4948445200000001000000010806000000\
                   1f15c4890000000d4944415478da63fccfc0500f000485018084a98c2100000000\
                   49454e44ae426082";
const GIF: &str = "47494638396101000100800000000000ffffff21f90401000000002c0000\
                   00000100010000020144003b";
const BMP: &str = "424d8e000000000000008a0000007c00000001000000ffffffff01002000030000000400000000\
                   000000000000000000000000000000000000ff0000ff0000ff000000000000ff42475273000000\
                   00000000000000000000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000ff7f";
/// The interesting one: its frame header stands behind an application segment, an exif segment, a
/// photoshop segment and two quantisation tables, so accepting it means the walk over segments
/// works rather than the first bytes happening to line up.
const JPEG: &str = "ffd8ffe000104a46494600010100004800480000ffe1004c4578696600004d4d002a0000000800\
                    0187690004000000010000001a000000000003a00100030000000100010000a002000400000001\
                    00000001a0030004000000010000000100000000ffed003850686f746f73686f7020332e300038\
                    42494d04040000000000003842494d0425000000000010d41d8cd98f00b204e9800998ecf8427e\
                    ffc00011080001000103012200021101031101ffc4001f0000010501010101010100000000000000\
                    000102030405060708090a0bffc400b5100002010303020403050504040000017d010203000411\
                    05122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25\
                    262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475\
                    767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9\
                    bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9fa\
                    ffc4001f0100030101010101010101010000000000000102030405060708090a0bffc400b51100\
                    0201020404030407050404000102770001020311040521310612415107617113223281081442 91\
                    a1b1c109233352f0156272d10a162434e125f11718191a262728292a35363738393a4344454647\
                    48494a535455565758595a636465666768696a737475767778797a82838485868788898a929394\
                    95969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7\
                    d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9faffdb004300020202020202030202030503030\
                    305060505050506080606060606080a0808080808080a0a0a0a0a0a0a0a0c0c0c0c0c0c0e0e0e0\
                    e0e0f0f0f0f0f0f0f0f0f0fffdb0043010202020404040710 0b090b1010101010101010101010\
                    1010101010101010101010101010101010101010101010101010101010101010101010101010101\
                    0ffdd00040001ffda000c03010002110311003f00fa628a28afcacff400ffd9";

fn picture(hexadecimal: &str) -> Vec<u8> {
    let digits: Vec<char> = hexadecimal
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    digits
        .chunks(2)
        .map(|pair| {
            u8::from_str_radix(&pair.iter().collect::<String>(), 16).expect("hexadecimal byte")
        })
        .collect()
}

#[test]
fn a_request_for_a_file_generates_a_check_that_rejects_the_project_and_accepts_the_artifact() {
    let workspace = workspace();
    let generated = workspace
        .generate("create an empty html file")
        .expect("a request naming a file derives a check");

    assert!(
        generated.claim.contains("*.html"),
        "the claim does not name what satisfies it: {}",
        generated.claim
    );
    answer::decides_the_generated_check_within(&generated, &generated.negative_control, WALL_LIMIT)
        .expect("the generated check decides both halves");

    // And the same two decisions, made directly, so what the demonstration reported is what the
    // program does rather than what the demonstration concluded.
    assert!(!decides(&generated.program, &generated.negative_control));
    assert!(decides(&generated.program, &generated.positive_control));
}

/// The check the operator approves is the one the contract pins. The digest is stated in the draft
/// before authorization, and it is the digest the contract records for the oracle, so a program
/// edited between the two no longer belongs to the contract that was approved.
#[test]
fn the_generated_program_is_pinned_by_the_digest_the_contract_records() {
    let workspace = workspace();
    let generated = workspace
        .generate("create an empty html file")
        .expect("a request naming a file derives a check");

    let bytes = fs::read(&generated.program).expect("read the generated program");
    assert_eq!(generated.oracle_digest, digest_bytes(&bytes));
    assert_eq!(
        String::from_utf8(bytes).expect("the program is text"),
        generated.program_text,
        "the draft would show text the host does not hold"
    );

    let prepared = prepare_contract(&RunRequest {
        prompt: "create an empty html file".to_owned(),
        source: workspace.source.clone(),
        acceptance: Some(AcceptanceCondition::new(
            generated.program.clone(),
            generated.negative_control.clone(),
        )),
        ..RunRequest::default()
    })
    .expect("the generated program is a contract's oracle like any other");
    assert_eq!(prepared.oracle_digest(), generated.oracle_digest);
}

/// The mechanical claim of a request for a picture is that a picture was produced, which the bytes
/// answer and the name does not. A file that decodes is accepted whatever format it is in; a file
/// that only carries the name is not.
#[test]
fn a_request_for_an_image_is_decided_by_the_bytes_and_not_by_the_name() {
    let workspace = workspace();
    let generated = workspace
        .generate("create an image of the current architecture")
        .expect("a request naming a picture derives a check");
    assert!(
        generated.claim.contains("decode"),
        "the claim does not state that the bytes are read: {}",
        generated.claim
    );

    for (name, bytes) in [
        ("png", picture(PNG)),
        ("gif", picture(GIF)),
        ("bmp", picture(BMP)),
        ("jpeg", picture(JPEG)),
    ] {
        let carried = workspace.candidate(
            &format!("carries-{name}"),
            &[(&format!("drawing.{name}"), bytes)],
        );
        assert!(
            decides(&generated.program, &carried),
            "a candidate carrying a {name} picture was rejected"
        );
    }

    let named_only = workspace.candidate(
        "named-only",
        &[("drawing.png", b"this is not a picture\n".to_vec())],
    );
    assert!(
        !decides(&generated.program, &named_only),
        "a file that only carries the name of a picture was accepted as one"
    );

    let truncated = workspace.candidate(
        "truncated",
        &[("drawing.jpg", picture(JPEG)[..24].to_vec())],
    );
    assert!(
        !decides(&generated.program, &truncated),
        "a picture that stops before it states a size was accepted"
    );

    let empty = workspace.candidate("empty", &[("drawing.gif", Vec::new())]);
    assert!(
        !decides(&generated.program, &empty),
        "an empty file was accepted as a picture"
    );
}

/// The generated check is not offered where it would decide nothing. A project already carrying
/// the artifact satisfies the check before the work starts, so the demonstration refuses it and
/// says which claim was already true.
#[test]
fn a_project_already_carrying_the_artifact_is_refused_rather_than_given_a_check_that_decides_it() {
    let workspace = workspace();
    fs::write(workspace.source.join("index.html"), b"<!doctype html>\n").expect("existing file");
    let generated = workspace
        .generate("create an empty html file")
        .expect("a request naming a file derives a check");

    let refusal = answer::decides_the_generated_check_within(
        &generated,
        &generated.negative_control,
        WALL_LIMIT,
    )
    .expect_err("a check the project already satisfies was carried into a draft");
    let stated = refusal.to_string();
    assert!(stated.contains("already satisfied"), "{stated}");
    assert!(stated.contains("*.html"), "{stated}");
}

/// Nothing is invented. A request that asks for no observable result, and a request that names a
/// file type without asking for one to be produced, are both refused, and the refusal says what is
/// left to the operator.
#[test]
fn a_request_stating_no_artifact_is_refused_rather_than_guessed_at() {
    let workspace = workspace();
    for prompt in [
        "make the replay path idempotent under load",
        "fix the html escaping in the template layer",
        "",
    ] {
        let refusal = workspace
            .generate(prompt)
            .expect_err("a check was invented for a request that states no result");
        assert!(
            matches!(refusal, AnswerError::NoCheckDerivable { .. }),
            "{refusal}"
        );
        let stated = refusal.to_string();
        assert!(stated.contains("state a verifier of your own"), "{stated}");
    }
}

/// What the program says about itself is what the draft says about it, so an operator reading
/// either learns the same bound.
#[test]
fn the_generated_program_states_what_it_decides_and_what_it_leaves_to_the_operator() {
    let workspace = workspace();
    let generated = workspace
        .generate("generate empty html file")
        .expect("a request naming a file derives a check");

    assert!(
        generated.program_text.contains(&generated.claim),
        "the program does not carry the claim the draft states: {}",
        generated.program_text
    );
    assert!(
        generated.program_text.contains(&generated.remainder),
        "the program does not carry the boundary the draft states: {}",
        generated.program_text
    );
    assert!(
        generated.program_text.contains("no model wrote this"),
        "the program does not say how it was derived: {}",
        generated.program_text
    );
}

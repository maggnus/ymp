use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, process::Command};

fn files(root: &Path, areas: &[&str]) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for area in areas {
        let path = root.join(area);
        println!("cargo:rerun-if-changed={}", path.display());
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| e.file_name() != "target" && e.file_name() != "__pycache__")
        {
            let entry = entry.expect("read build input");
            if entry.file_type().is_file() {
                let bytes = std::fs::read(entry.path()).expect("read build input bytes");
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .display()
                        .to_string(),
                    format!("{:x}", Sha256::digest(bytes)),
                );
            }
        }
    }
    files
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Cannot bind evaluation build revision"
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).ancestors().nth(3).unwrap();
    let source = files(
        root,
        &[
            "Cargo.toml",
            "Cargo.lock",
            "ymp-rust/crates",
            "ymp-evals/driver",
        ],
    );
    let fixtures = files(
        root,
        &[
            "ymp-evals/scenarios",
            "ymp-evals/fixtures/universal",
            "ymp-evals/validators",
        ],
    );
    for args in [
        vec!["rev-parse", "--git-path", "HEAD"],
        vec!["rev-parse", "--git-path", "refs"],
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            root.join(git(root, &args)).display()
        );
    }
    let rustc = Command::new(std::env::var("RUSTC").unwrap())
        .arg("--version")
        .output()
        .unwrap();
    assert!(rustc.status.success());
    let snapshot = serde_json::json!({
        "schema_version": 1,
        "revision": git(root, &["rev-parse", "HEAD"]),
        "source_tree_sha256": format!("{:x}", Sha256::digest(serde_json::to_vec(&source).unwrap())),
        "source_files": source,
        "fixture_hashes": fixtures,
        "target": std::env::var("TARGET").unwrap(),
        "profile": std::env::var("PROFILE").unwrap(),
        "rustc": String::from_utf8(rustc.stdout).unwrap().trim(),
    });
    let output = Path::new(&std::env::var("OUT_DIR").unwrap()).join("native-source-snapshot.json");
    std::fs::write(output, serde_json::to_vec(&snapshot).unwrap()).unwrap();
}

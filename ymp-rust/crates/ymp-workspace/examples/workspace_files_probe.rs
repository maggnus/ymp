//! Offline release-profile enumeration measurements for YMP-105.
use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};
use ymp_workspace::Workspace;

fn measure(root: &Path, metadata: &Path, mut names: Vec<String>, bytes: u64) -> Result<Value> {
    names.sort();
    let workspace = Workspace::open(root, metadata)?;
    let expected: Vec<_> = names
        .iter()
        .map(|name| workspace.directory.join(name))
        .collect();
    let manifest = serde_json::to_value(&workspace)?;
    let mut fingerprint_names: Vec<_> = manifest["initial"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    fingerprint_names.sort();
    assert_eq!(fingerprint_names, names);
    assert!(workspace.changes()?.is_empty());

    let mut seconds = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let actual = workspace.files()?;
        seconds.push(start.elapsed().as_secs_f64());
        assert_eq!(actual, expected);
    }
    let mut sorted_seconds = seconds.clone();
    sorted_seconds.sort_by(f64::total_cmp);
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&names)?);
    Ok(json!({
        "files": expected.len(),
        "regular_file_bytes": bytes,
        "workspace_files_seconds": seconds,
        "median_seconds": sorted_seconds[sorted_seconds.len() / 2],
        "expected_sorted_paths_equal_every_sample": true,
        "initial_fingerprint_path_set_equal": true,
        "unchanged_hash_verification_passed": true,
        "relative_path_set_sha256": format!("{:x}", hash.finalize()),
    }))
}

fn main() -> Result<()> {
    let scratch = tempfile::Builder::new()
        .prefix("ymp-files-probe-")
        .tempdir()?;
    let small = scratch.path().join("files-10000");
    fs::create_dir(&small)?;
    let mut small_names = Vec::new();
    for i in 0..10_000 {
        let name = format!("document-{i:05}.md");
        fs::write(small.join(&name), vec![b'x'; 1024])?;
        small_names.push(name);
    }
    let required = "The requested artifact.\n";
    fs::write(small.join("zzz-required.md"), required)?;
    small_names.push("zzz-required.md".into());

    let large = scratch.path().join("large-file");
    fs::create_dir(&large)?;
    fs::write(large.join("large.bin"), vec![b'x'; 64 * 1024 * 1024])?;
    fs::write(large.join("zzz-required.md"), required)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "variant": std::env::args().nth(1).unwrap_or_else(|| "names-only".into()),
            "samples_per_fixture": 5,
            "fixtures": [
                {
                    "fixture": "10000 small files plus required artifact",
                    "measurement": measure(
                        &small,
                        &scratch.path().join("meta-small"),
                        small_names,
                        10_000 * 1024 + required.len() as u64,
                    )?,
                },
                {
                    "fixture": "64 MiB regular file plus required artifact",
                    "measurement": measure(
                        &large,
                        &scratch.path().join("meta-large"),
                        vec!["large.bin".into(), "zzz-required.md".into()],
                        64 * 1024 * 1024 + required.len() as u64,
                    )?,
                },
            ],
            "caveat": "Offline warm-cache observations on one machine; no cache eviction, percentile guarantees, provider inference, or model-quality claims. Workspace open and hash verification occur before enumeration timing. Metadata and synthetic fixtures use a temporary directory that is removed on exit.",
        }))?
    );
    Ok(())
}

#![forbid(unsafe_code)]

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

const CASE_IDS: [&str; 3] = ["L1-line-endings", "L2-size-parser", "L3-command-ledger"];

#[derive(Debug, Parser)]
#[command(
    name = "ymp-calibration",
    about = "Development agent calibration ladder"
)]
struct Cli {
    #[command(subcommand)]
    command: CalibrationCommand,
}

#[derive(Debug, Subcommand)]
enum CalibrationCommand {
    List,
    Prepare {
        case_id: String,
        output: PathBuf,
    },
    Verify {
        case_id: String,
        candidate: PathBuf,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    ValidateCases,
}

#[derive(Debug, Serialize)]
struct CheckResult {
    command: String,
    success: bool,
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
struct VerificationReport {
    schema_version: u32,
    case_id: String,
    passed: bool,
    candidate_digest: String,
    manifest_unchanged: bool,
    changed_paths: Vec<String>,
    duration_ms: u128,
    checks: Vec<CheckResult>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CalibrationCommand::List => {
            println!("{}", serde_json::to_string_pretty(&CASE_IDS)?);
            Ok(())
        }
        CalibrationCommand::Prepare { case_id, output } => prepare(&case_id, &output),
        CalibrationCommand::Verify {
            case_id,
            candidate,
            report,
        } => {
            let result = verify(&case_id, &candidate)?;
            let encoded = serde_json::to_string_pretty(&result)?;
            println!("{encoded}");
            if let Some(path) = report {
                fs::write(&path, format!("{encoded}\n"))
                    .with_context(|| format!("write report {}", path.display()))?;
            }
            if !result.passed {
                bail!("candidate failed protected calibration checks");
            }
            Ok(())
        }
        CalibrationCommand::ValidateCases => validate_cases(),
    }
}

fn case_root(case_id: &str) -> Result<PathBuf> {
    if !CASE_IDS.contains(&case_id) {
        bail!("unknown calibration case {case_id}");
    }
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cases")
        .join(case_id))
}

fn prepare(case_id: &str, output: &Path) -> Result<()> {
    let source = case_root(case_id)?.join("public");
    if output.exists() {
        bail!("refusing to overwrite existing path {}", output.display());
    }
    copy_tree(&source, output)?;
    run_required(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(output),
        "git init",
    )?;
    run_required(
        Command::new("git").args(["add", "."]).current_dir(output),
        "git add",
    )?;
    run_required(
        Command::new("git")
            .args([
                "-c",
                "user.name=ymp calibration",
                "-c",
                "user.email=calibration@invalid",
                "commit",
                "--quiet",
                "-m",
                "fixture: seed calibration case",
            ])
            .current_dir(output),
        "git commit",
    )?;
    println!("{}", output.display());
    Ok(())
}

fn verify(case_id: &str, candidate: &Path) -> Result<VerificationReport> {
    let started = Instant::now();
    let root = case_root(case_id)?;
    let digest = tree_digest(candidate)?;
    let public = root.join("public");
    let changed_paths = changed_paths(&public, candidate)?;
    let manifest_unchanged = fs::read(public.join("Cargo.toml")).context("read public manifest")?
        == fs::read(candidate.join("Cargo.toml")).context("read candidate manifest")?;

    let private = tempfile::tempdir().context("create private verifier directory")?;
    copy_tree(candidate, private.path())?;
    let protected = root.join("protected").join("protected.rs");
    let tests = private.path().join("tests");
    fs::create_dir_all(&tests).context("create private test directory")?;
    let package = package_name(candidate)?;
    let protected_source = fs::read_to_string(&protected)
        .with_context(|| format!("read protected oracle for {case_id}"))?
        .replace("ymp_calibration_case", &package.replace('-', "_"));
    fs::write(tests.join("protected.rs"), protected_source)
        .with_context(|| format!("inject protected oracle for {case_id}"))?;

    let specifications: [(&str, &[&str]); 3] = [
        ("cargo fmt --check", &["fmt", "--check"]),
        ("cargo test --all-targets", &["test", "--all-targets"]),
        (
            "cargo clippy --all-targets -- -D warnings",
            &["clippy", "--all-targets", "--", "-D", "warnings"],
        ),
    ];
    let mut checks = Vec::new();
    for (label, arguments) in specifications {
        let output = Command::new("cargo")
            .args(arguments)
            .current_dir(private.path())
            .output()
            .with_context(|| format!("run {label}"))?;
        checks.push(check_result(label, output));
        if !checks.last().is_some_and(|check| check.success) {
            break;
        }
    }
    let passed = manifest_unchanged
        && checks.len() == specifications.len()
        && checks.iter().all(|check| check.success);
    Ok(VerificationReport {
        schema_version: 1,
        case_id: case_id.to_owned(),
        passed,
        candidate_digest: digest,
        manifest_unchanged,
        changed_paths,
        duration_ms: started.elapsed().as_millis(),
        checks,
    })
}

fn validate_cases() -> Result<()> {
    let outer = tempfile::tempdir().context("create corpus validation directory")?;
    let mut rejected = Vec::new();
    for case_id in CASE_IDS {
        let candidate = outer.path().join(case_id);
        prepare(case_id, &candidate)?;
        let report = verify(case_id, &candidate)?;
        rejected.push(serde_json::json!({
            "case_id": case_id,
            "seeded_defect_rejected": !report.passed,
            "candidate_digest": report.candidate_digest,
        }));
        if report.passed {
            bail!("protected checks failed to reject seeded defect in {case_id}");
        }
    }
    println!("{}", serde_json::to_string_pretty(&rejected)?);
    Ok(())
}

fn check_result(label: &str, output: Output) -> CheckResult {
    CheckResult {
        command: label.to_owned(),
        success: output.status.success(),
        status: output.status.code(),
        stdout: bounded_text(&output.stdout),
        stderr: bounded_text(&output.stderr),
    }
}

fn bounded_text(bytes: &[u8]) -> String {
    const LIMIT: usize = 16 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn run_required(command: &mut Command, label: &str) -> Result<()> {
    let output = command.output().with_context(|| format!("run {label}"))?;
    if !output.status.success() {
        bail!(
            "{label} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn changed_paths(base: &Path, candidate: &Path) -> Result<Vec<String>> {
    let mut base_files = Vec::new();
    let mut candidate_files = Vec::new();
    collect_files(base, base, &mut base_files)?;
    collect_files(candidate, candidate, &mut candidate_files)?;
    let base_files: BTreeSet<_> = base_files.into_iter().collect();
    let candidate_files: BTreeSet<_> = candidate_files.into_iter().collect();
    let paths: BTreeSet<_> = base_files.union(&candidate_files).cloned().collect();
    let mut changed = Vec::new();
    for path in paths {
        let status = match (base_files.contains(&path), candidate_files.contains(&path)) {
            (false, true) => "A",
            (true, false) => "D",
            (true, true) if fs::read(base.join(&path))? != fs::read(candidate.join(&path))? => "M",
            _ => continue,
        };
        changed.push(format!("{status} {}", path.display()));
    }
    Ok(changed)
}

fn package_name(candidate: &Path) -> Result<String> {
    let manifest =
        fs::read_to_string(candidate.join("Cargo.toml")).context("read candidate Cargo.toml")?;
    manifest
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("name = ")
                .and_then(|value| value.strip_prefix('"'))
                .and_then(|value| value.strip_suffix('"'))
                .map(str::to_owned)
        })
        .context("candidate Cargo.toml has no package name")
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("create directory {}", destination.display()))?;
    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let kind = entry.file_type()?;
        let target = destination.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copy {}", entry.path().display()))?;
        } else {
            bail!("calibration trees must not contain symbolic links");
        }
    }
    Ok(())
}

fn tree_digest(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut digest = Sha256::new();
    for relative in files {
        digest.update(relative.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(fs::read(root.join(&relative))?);
        digest.update([0]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path().strip_prefix(root)?.to_owned());
        } else {
            bail!("candidate trees must not contain symbolic links");
        }
    }
    Ok(())
}

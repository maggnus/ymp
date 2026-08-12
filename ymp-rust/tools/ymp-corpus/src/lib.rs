#![forbid(unsafe_code)]

pub mod study;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile, TempDir};

const OUTPUT_LIMIT: usize = 4096;
const TREE_DOMAIN: &[u8] = b"TREEv1\0";
const PUBLIC_BINDING_BEGIN: &str = "<!-- YMP-CORPUS-BINDING-V1-BEGIN -->";
const PUBLIC_BINDING_END: &str = "<!-- YMP-CORPUS-BINDING-V1-END -->";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlObservation {
    ExpectedFailure,
    UnexpectedPass,
    InfrastructureError,
}

pub fn negative_control_keeps_package_usable(observation: ControlObservation) -> bool {
    observation == ControlObservation::ExpectedFailure
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub schema_version: u32,
    pub corpus_id: String,
    pub corpus_role: String,
    pub edition_status: String,
    pub statistically_sufficient: bool,
    pub study_frozen: bool,
    pub calibration_cases_excluded: Vec<String>,
    pub preparation_model_calls: u64,
    pub reproduction_model_calls: u64,
    pub expansion_policy: ArtifactRef,
    pub approval_policy: ArtifactRef,
    pub policies: Vec<ArtifactRef>,
    pub tasks: Vec<TaskEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEntry {
    pub id: String,
    pub manifest: ArtifactRef,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub schema_version: u32,
    pub id: String,
    pub admission_status: String,
    pub verified_at: String,
    pub upstream: Upstream,
    pub classification: Classification,
    pub requirements: Vec<Requirement>,
    pub environment: Environment,
    pub snapshots: Snapshots,
    pub artifacts: TaskArtifacts,
    pub control_variants: Vec<ControlVariant>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub repository_url: String,
    pub source_commit: String,
    pub fixed_commit: String,
    pub source_commit_url: String,
    pub fixed_commit_url: String,
    pub fix_sources: Vec<String>,
    pub license_expression: String,
    pub license_files: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Decomposable,
    StronglySequential,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub public_summary: String,
    pub protected_test: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub rustc_version: String,
    pub cargo_version: String,
    pub host: String,
    pub git_version: String,
    pub archive_tool_version: String,
    pub timeout_seconds: u64,
    pub oracle_path: PathBuf,
    pub visible_command: CommandSpec,
    pub protected_command: CommandSpec,
    pub vendor_tree_sha256: String,
    pub vendor_git_sources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshots {
    pub source: Snapshot,
    pub fixed: Snapshot,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub commit: String,
    pub archive_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskArtifacts {
    pub public_contract: ArtifactRef,
    pub technical_review: ArtifactRef,
    pub requirement_matrix: ArtifactRef,
    pub oracle: ArtifactRef,
    pub source_lock: ArtifactRef,
    pub fixed_lock: ArtifactRef,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlVariant {
    pub id: String,
    pub requirement_ids: Vec<String>,
    pub reverse_patch: ArtifactRef,
}

#[derive(Debug)]
pub struct LoadedCorpus {
    pub root: PathBuf,
    pub registry: Registry,
    pub registry_sha256: String,
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicBinding {
    schema_version: u32,
    task_id: String,
    artifacts: Vec<PublicArtifactDigest>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicArtifactDigest {
    role: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PreparationReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub registry_sha256: String,
    pub model_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub provider_cost_minor_units: u64,
    pub provider_cost_currency: Option<String>,
    pub wall_time_ms: u128,
    pub prepared_tasks: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReproductionReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub registry_sha256: String,
    pub approved_set_root_sha256: String,
    pub authorization_scope: AuthorizationScope,
    pub started_unix_seconds: u64,
    pub offline: bool,
    pub model_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub provider_cost_minor_units: u64,
    pub provider_cost_currency: Option<String>,
    pub protected_queries: u64,
    pub wall_time_ms: u128,
    pub usable: bool,
    pub environment: ReproductionEnvironment,
    pub tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationScope {
    TechnicalVerificationOnly,
}

#[derive(Debug, Serialize)]
pub struct ReproductionEnvironment {
    pub rustc_version: String,
    pub cargo_version: String,
    pub host: String,
    pub git_version: String,
    pub archive_tool_version: String,
}

#[derive(Debug, Serialize)]
pub struct TaskReport {
    pub id: String,
    pub classification: Classification,
    pub usable: bool,
    pub checks: Vec<CheckReport>,
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub name: String,
    pub candidate: String,
    pub requirement: Option<String>,
    pub command: Vec<String>,
    pub expectation: String,
    pub observation: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

#[derive(Debug)]
struct CommandResult {
    status: Option<ExitStatus>,
    timed_out: bool,
    duration: Duration,
    stdout_tail: String,
    stderr_tail: String,
}

struct PreparedCandidate {
    _temp: TempDir,
    root: PathBuf,
    cargo_home: PathBuf,
    target_dir: PathBuf,
}

pub fn load_corpus(root: &Path) -> Result<LoadedCorpus> {
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalize corpus root {}", root.display()))?;
    let registry_path = root.join("registry.json");
    let registry_bytes = fs::read(&registry_path)
        .with_context(|| format!("read registry {}", registry_path.display()))?;
    let registry_sha256 = sha256_bytes(&registry_bytes);
    let registry: Registry = serde_json::from_slice(&registry_bytes).context("parse registry")?;
    ensure!(registry.schema_version == 2, "unsupported registry schema");
    ensure!(
        registry.corpus_role == "primary_corpus_candidate",
        "registry has an unsupported corpus role"
    );
    ensure!(
        registry.edition_status == "reproducible_first_edition",
        "registry has an unsupported edition status"
    );
    ensure!(
        !registry.statistically_sufficient && !registry.study_frozen,
        "this first edition must not claim statistical sufficiency or study freeze"
    );
    ensure!(
        registry.preparation_model_calls == 0 && registry.reproduction_model_calls == 0,
        "corpus preparation and reproduction must not use model calls"
    );
    ensure!(
        registry
            .calibration_cases_excluded
            .iter()
            .any(|value| value == "L1-L3"),
        "registry must explicitly exclude L1-L3"
    );
    verify_artifact(&root, &registry.expansion_policy)?;
    verify_artifact(&root, &registry.approval_policy)?;
    for policy in &registry.policies {
        verify_artifact(&root, policy)?;
    }

    let mut seen = BTreeSet::new();
    let mut tasks = Vec::with_capacity(registry.tasks.len());
    for entry in &registry.tasks {
        ensure!(seen.insert(entry.id.clone()), "duplicate task {}", entry.id);
        let bytes = verify_artifact(&root, &entry.manifest)?;
        let task: Task = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse task manifest {}", entry.manifest.path.display()))?;
        validate_task(&root, &registry, &task, &entry.id)?;
        tasks.push(task);
    }
    ensure!(!tasks.is_empty(), "corpus edition contains no tasks");
    ensure!(
        tasks
            .iter()
            .any(|task| matches!(task.classification, Classification::Decomposable)),
        "corpus has no decomposable task"
    );
    ensure!(
        tasks
            .iter()
            .any(|task| matches!(task.classification, Classification::StronglySequential)),
        "corpus has no strongly sequential task"
    );
    Ok(LoadedCorpus {
        root,
        registry,
        registry_sha256,
        tasks,
    })
}

fn validate_task(root: &Path, registry: &Registry, task: &Task, expected_id: &str) -> Result<()> {
    ensure!(task.schema_version == 2, "unsupported task schema");
    ensure!(task.id == expected_id, "task id does not match registry");
    ensure!(
        task.admission_status == "candidate_technically_verified",
        "task is not technically verified"
    );
    ensure!(
        !task.verified_at.is_empty(),
        "task has no verification date"
    );
    validate_git_sha(&task.upstream.source_commit, "source commit")?;
    validate_git_sha(&task.upstream.fixed_commit, "fixed commit")?;
    ensure!(
        task.upstream.repository_url.starts_with("https://"),
        "upstream repository URL must use HTTPS"
    );
    ensure!(
        task.upstream.source_commit_url.starts_with("https://")
            && task.upstream.fixed_commit_url.starts_with("https://")
            && task
                .upstream
                .fix_sources
                .iter()
                .all(|source| source.starts_with("https://")),
        "upstream evidence URLs must use HTTPS"
    );
    ensure!(
        !task.upstream.fix_sources.is_empty(),
        "task has no upstream fix sources"
    );
    ensure!(
        !task.upstream.license_expression.trim().is_empty()
            && !task.upstream.license_files.is_empty(),
        "task has no license evidence"
    );
    ensure!(
        task.snapshots.source.commit == task.upstream.source_commit,
        "source snapshot commit mismatch"
    );
    ensure!(
        task.snapshots.fixed.commit == task.upstream.fixed_commit,
        "fixed snapshot commit mismatch"
    );
    validate_sha256(&task.snapshots.source.archive_sha256, "source archive")?;
    validate_sha256(&task.snapshots.fixed.archive_sha256, "fixed archive")?;
    validate_sha256(&task.environment.vendor_tree_sha256, "vendor tree")?;
    validate_relative_path(&task.environment.oracle_path)?;
    ensure!(
        task.environment.timeout_seconds > 0,
        "timeout must be positive"
    );
    ensure!(
        !task.environment.archive_tool_version.trim().is_empty(),
        "archive tool version is empty"
    );
    ensure!(
        task.environment
            .vendor_git_sources
            .iter()
            .all(|source| source.starts_with("https://")),
        "vendored Git source URLs must use HTTPS"
    );
    ensure!(!task.requirements.is_empty(), "task has no requirements");
    ensure!(
        !task.control_variants.is_empty(),
        "task has no negative controls"
    );

    let requirement_ids = task
        .requirements
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        requirement_ids.len() == task.requirements.len(),
        "duplicate requirement id"
    );
    for requirement in &task.requirements {
        ensure!(
            !requirement.public_summary.trim().is_empty(),
            "requirement {} has no public summary",
            requirement.id
        );
        ensure!(
            !requirement.protected_test.trim().is_empty(),
            "requirement {} has no protected test",
            requirement.id
        );
    }

    let mut controlled = BTreeSet::new();
    for variant in &task.control_variants {
        ensure!(
            !variant.requirement_ids.is_empty(),
            "variant {} covers no requirements",
            variant.id
        );
        for requirement_id in &variant.requirement_ids {
            ensure!(
                requirement_ids.contains(requirement_id.as_str()),
                "variant {} names unknown requirement {}",
                variant.id,
                requirement_id
            );
            controlled.insert(requirement_id.as_str());
        }
        verify_artifact(root, &variant.reverse_patch)?;
    }
    ensure!(
        controlled == requirement_ids,
        "not every requirement has an invalid control variant"
    );
    for artifact in [
        &task.artifacts.public_contract,
        &task.artifacts.technical_review,
        &task.artifacts.requirement_matrix,
        &task.artifacts.oracle,
        &task.artifacts.source_lock,
        &task.artifacts.fixed_lock,
    ] {
        verify_artifact(root, artifact)?;
    }
    validate_public_binding(root, registry, task)?;
    Ok(())
}

fn validate_public_binding(root: &Path, registry: &Registry, task: &Task) -> Result<()> {
    let bytes = fs::read(root.join(&task.artifacts.public_contract.path))?;
    let contract = std::str::from_utf8(&bytes).context("public contract is not UTF-8")?;
    let binding_json = contract
        .split_once(PUBLIC_BINDING_BEGIN)
        .and_then(|(_, tail)| tail.split_once(PUBLIC_BINDING_END).map(|(json, _)| json))
        .context("public contract has no canonical artifact binding")?;
    let binding: PublicBinding =
        serde_json::from_str(binding_json.trim()).context("parse public artifact binding")?;
    ensure!(
        binding.schema_version == 1,
        "unsupported public binding schema"
    );
    ensure!(
        binding.task_id == task.id,
        "public binding task id mismatch"
    );
    let mut sorted = binding.artifacts.clone();
    sorted.sort();
    ensure!(
        sorted == binding.artifacts,
        "public artifact binding must be sorted by role and digest"
    );
    ensure!(
        binding
            .artifacts
            .iter()
            .map(|artifact| artifact.role.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            == binding.artifacts.len(),
        "public artifact binding contains duplicate roles"
    );
    for artifact in &binding.artifacts {
        validate_sha256(&artifact.sha256, &artifact.role)?;
    }
    ensure!(
        binding.artifacts == expected_public_binding(registry, task),
        "public artifact binding does not match the task manifest and corpus policies"
    );
    Ok(())
}

fn expected_public_binding(registry: &Registry, task: &Task) -> Vec<PublicArtifactDigest> {
    let mut artifacts = vec![
        public_digest("corpus.approval_policy", &registry.approval_policy.sha256),
        public_digest("corpus.expansion_policy", &registry.expansion_policy.sha256),
        public_digest("task.fixed_archive", &task.snapshots.fixed.archive_sha256),
        public_digest("task.fixed_lock", &task.artifacts.fixed_lock.sha256),
        public_digest(
            "task.requirement_matrix",
            &task.artifacts.requirement_matrix.sha256,
        ),
        public_digest("task.source_archive", &task.snapshots.source.archive_sha256),
        public_digest("task.source_lock", &task.artifacts.source_lock.sha256),
        public_digest(
            "task.technical_review",
            &task.artifacts.technical_review.sha256,
        ),
        public_digest("task.vendor_tree", &task.environment.vendor_tree_sha256),
        public_digest("protected.oracle", &task.artifacts.oracle.sha256),
    ];
    for policy in &registry.policies {
        let name = policy
            .path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown");
        artifacts.push(public_digest(
            &format!("corpus.{name}_policy"),
            &policy.sha256,
        ));
    }
    for variant in &task.control_variants {
        artifacts.push(public_digest(
            &format!("protected.control.{}", variant.id),
            &variant.reverse_patch.sha256,
        ));
    }
    artifacts.sort();
    artifacts
}

fn public_digest(role: &str, sha256: &str) -> PublicArtifactDigest {
    PublicArtifactDigest {
        role: role.to_owned(),
        sha256: sha256.to_owned(),
    }
}

fn verify_artifact(root: &Path, artifact: &ArtifactRef) -> Result<Vec<u8>> {
    validate_sha256(&artifact.sha256, "artifact")?;
    validate_relative_path(&artifact.path)?;
    let path = root.join(&artifact.path);
    let bytes = fs::read(&path).with_context(|| format!("read artifact {}", path.display()))?;
    ensure!(
        sha256_bytes(&bytes) == artifact.sha256,
        "artifact digest mismatch: {}",
        artifact.path.display()
    );
    Ok(bytes)
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "empty relative path");
    ensure!(!path.is_absolute(), "absolute path is forbidden");
    for component in path.components() {
        ensure!(
            matches!(component, Component::Normal(_)),
            "non-normal path component in {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_git_sha(value: &str, label: &str) -> Result<()> {
    validate_lower_hex(value, 40, label, "full Git SHA")
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    validate_lower_hex(value, 64, label, "SHA-256")
}

fn validate_lower_hex(value: &str, length: usize, label: &str, kind: &str) -> Result<()> {
    ensure!(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not a lowercase {kind}"
    );
    Ok(())
}

pub fn prepare(corpus: &LoadedCorpus, cache: &Path) -> Result<PreparationReport> {
    let started = Instant::now();
    check_toolchain(corpus)?;
    fs::create_dir_all(cache).with_context(|| format!("create cache {}", cache.display()))?;
    for task in &corpus.tasks {
        prepare_archive(cache, &task.upstream.repository_url, &task.snapshots.source)?;
        prepare_archive(cache, &task.upstream.repository_url, &task.snapshots.fixed)?;
        prepare_vendor(corpus, cache, task)?;
    }
    check_cache(corpus, cache)?;
    Ok(PreparationReport {
        schema_version: 1,
        corpus_id: corpus.registry.corpus_id.clone(),
        registry_sha256: corpus.registry_sha256.clone(),
        model_calls: 0,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
        provider_cost_minor_units: 0,
        provider_cost_currency: None,
        wall_time_ms: started.elapsed().as_millis(),
        prepared_tasks: corpus.tasks.iter().map(|task| task.id.clone()).collect(),
    })
}

pub fn check_cache(corpus: &LoadedCorpus, cache: &Path) -> Result<()> {
    for task in &corpus.tasks {
        for snapshot in [&task.snapshots.source, &task.snapshots.fixed] {
            let path = cache_object_path(cache, &snapshot.archive_sha256);
            let actual = sha256_file(&path)
                .with_context(|| format!("verify cache object {}", path.display()))?;
            ensure!(
                actual == snapshot.archive_sha256,
                "cache object digest mismatch: {}",
                path.display()
            );
        }
        let vendor = cache_tree_path(cache, &task.environment.vendor_tree_sha256);
        ensure!(vendor.is_dir(), "missing vendor tree {}", vendor.display());
        ensure!(
            tree_digest(&vendor)? == task.environment.vendor_tree_sha256,
            "vendor tree digest mismatch: {}",
            vendor.display()
        );
    }
    Ok(())
}

fn prepare_archive(cache: &Path, repository: &str, snapshot: &Snapshot) -> Result<()> {
    let destination = cache_object_path(cache, &snapshot.archive_sha256);
    if destination.exists() {
        ensure!(
            sha256_file(&destination)? == snapshot.archive_sha256,
            "existing cache object has wrong digest: {}",
            destination.display()
        );
        return Ok(());
    }
    let parent = destination.parent().context("cache object has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = Builder::new().prefix(".archive-").tempdir_in(parent)?;
    let git_dir = temp.path().join("repository.git");
    run_setup(
        Command::new("git")
            .arg("init")
            .arg("--bare")
            .arg("--quiet")
            .arg(&git_dir),
        "initialize source repository",
    )?;
    run_setup(
        Command::new("git")
            .arg("--git-dir")
            .arg(&git_dir)
            .arg("-c")
            .arg("protocol.version=2")
            .arg("fetch")
            .arg("--quiet")
            .arg("--depth=1")
            .arg("--no-tags")
            .arg(repository)
            .arg(&snapshot.commit)
            .env("GIT_TERMINAL_PROMPT", "0"),
        "fetch source revision",
    )?;
    let archive = temp.path().join("archive.tar");
    run_setup(
        Command::new("git")
            .arg("--git-dir")
            .arg(&git_dir)
            .arg("archive")
            .arg("--format=tar")
            .arg(format!("--output={}", archive.display()))
            .arg("FETCH_HEAD"),
        "archive source revision",
    )?;
    ensure!(
        sha256_file(&archive)? == snapshot.archive_sha256,
        "archive digest differs from registry for {}",
        snapshot.commit
    );
    fs::rename(&archive, &destination)
        .with_context(|| format!("install cache object {}", destination.display()))?;
    Ok(())
}

fn prepare_vendor(corpus: &LoadedCorpus, cache: &Path, task: &Task) -> Result<()> {
    let destination = cache_tree_path(cache, &task.environment.vendor_tree_sha256);
    if destination.exists() {
        ensure!(
            tree_digest(&destination)? == task.environment.vendor_tree_sha256,
            "existing vendor tree has wrong digest"
        );
        return Ok(());
    }
    let parent = destination.parent().context("vendor tree has no parent")?;
    fs::create_dir_all(parent)?;
    let staging = Builder::new().prefix(".vendor-").tempdir_in(parent)?;
    let candidate = prepare_candidate(corpus, cache, task, CandidateBase::Fixed, false)?;
    let staged_tree = staging.path().join("tree");
    run_setup(
        Command::new("cargo")
            .arg("vendor")
            .arg("--locked")
            .arg("--versioned-dirs")
            .arg(&staged_tree)
            .current_dir(&candidate.root),
        "vendor task dependencies",
    )?;
    ensure!(
        tree_digest(&staged_tree)? == task.environment.vendor_tree_sha256,
        "vendored dependency digest differs from registry for {}",
        task.id
    );
    fs::rename(&staged_tree, &destination)
        .with_context(|| format!("install vendor tree {}", destination.display()))?;
    Ok(())
}

fn run_setup(command: &mut Command, label: &str) -> Result<()> {
    let output = command.output().with_context(|| label.to_owned())?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

#[derive(Clone, Copy)]
enum CandidateBase {
    Source,
    Fixed,
}

pub fn reproduce(corpus: &LoadedCorpus, cache: &Path) -> Result<ReproductionReport> {
    let started = Instant::now();
    let environment = check_toolchain(corpus)?;
    check_cache(corpus, cache)?;
    let started_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock precedes Unix epoch")?
        .as_secs();
    let mut reports = Vec::with_capacity(corpus.tasks.len());
    for task in &corpus.tasks {
        let report = match reproduce_task(corpus, cache, task) {
            Ok(report) => report,
            Err(error) => TaskReport {
                id: task.id.clone(),
                classification: task.classification,
                usable: false,
                checks: vec![CheckReport {
                    name: "harness".to_owned(),
                    candidate: "package".to_owned(),
                    requirement: None,
                    command: Vec::new(),
                    expectation: "harness completes".to_owned(),
                    observation: format!("infrastructure_error: {error:#}"),
                    exit_code: None,
                    duration_ms: 0,
                    stdout_tail: String::new(),
                    stderr_tail: String::new(),
                }],
            },
        };
        reports.push(report);
    }
    check_cache(corpus, cache).context("cache changed during reproduction")?;
    let usable = reports.iter().all(|report| report.usable);
    let protected_queries = reports
        .iter()
        .flat_map(|report| &report.checks)
        .filter(|check| {
            check.name == "fixed-protected" || check.expectation == "fail_in_named_protected_test"
        })
        .count() as u64;
    Ok(ReproductionReport {
        schema_version: 3,
        corpus_id: corpus.registry.corpus_id.clone(),
        registry_sha256: corpus.registry_sha256.clone(),
        approved_set_root_sha256: corpus.registry_sha256.clone(),
        authorization_scope: AuthorizationScope::TechnicalVerificationOnly,
        started_unix_seconds,
        offline: true,
        model_calls: 0,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
        provider_cost_minor_units: 0,
        provider_cost_currency: None,
        protected_queries,
        wall_time_ms: started.elapsed().as_millis(),
        usable,
        environment,
        tasks: reports,
    })
}

fn reproduce_task(corpus: &LoadedCorpus, cache: &Path, task: &Task) -> Result<TaskReport> {
    let mut checks = Vec::new();
    let timeout = Duration::from_secs(task.environment.timeout_seconds);

    let source = prepare_candidate(corpus, cache, task, CandidateBase::Source, true)?;
    checks.push(run_expected_pass(
        "source-visible",
        "source",
        None,
        &task.environment.visible_command,
        &source,
        timeout,
    )?);
    inject_oracle(corpus, task, &source.root)?;
    for requirement in &task.requirements {
        checks.push(run_expected_failure(
            &format!("source-control-{}", requirement.id),
            "source",
            requirement,
            &task.environment.protected_command,
            &source,
            timeout,
        )?);
    }
    drop(source);

    let fixed = prepare_candidate(corpus, cache, task, CandidateBase::Fixed, true)?;
    checks.push(run_expected_pass(
        "fixed-visible",
        "fixed",
        None,
        &task.environment.visible_command,
        &fixed,
        timeout,
    )?);
    inject_oracle(corpus, task, &fixed.root)?;
    checks.push(run_expected_pass(
        "fixed-protected",
        "fixed",
        None,
        &task.environment.protected_command,
        &fixed,
        timeout,
    )?);
    drop(fixed);

    for variant in &task.control_variants {
        let candidate = prepare_candidate(corpus, cache, task, CandidateBase::Fixed, true)?;
        inject_oracle(corpus, task, &candidate.root)?;
        reverse_patch(corpus, &candidate.root, &variant.reverse_patch)?;
        for requirement_id in &variant.requirement_ids {
            let requirement = task
                .requirements
                .iter()
                .find(|requirement| &requirement.id == requirement_id)
                .context("validated requirement disappeared")?;
            checks.push(run_expected_failure(
                &format!("invalid-{}-{}", variant.id, requirement.id),
                &format!("invalid:{}", variant.id),
                requirement,
                &task.environment.protected_command,
                &candidate,
                timeout,
            )?);
        }
    }

    let usable = checks.iter().all(check_is_usable);
    Ok(TaskReport {
        id: task.id.clone(),
        classification: task.classification,
        usable,
        checks,
    })
}

fn check_is_usable(check: &CheckReport) -> bool {
    match check.observation.as_str() {
        "passed" => true,
        "expected_failure" => {
            negative_control_keeps_package_usable(ControlObservation::ExpectedFailure)
        }
        "unexpected_pass" => {
            negative_control_keeps_package_usable(ControlObservation::UnexpectedPass)
        }
        "infrastructure_error" => {
            negative_control_keeps_package_usable(ControlObservation::InfrastructureError)
        }
        _ => false,
    }
}

fn prepare_candidate(
    corpus: &LoadedCorpus,
    cache: &Path,
    task: &Task,
    base: CandidateBase,
    configure_offline: bool,
) -> Result<PreparedCandidate> {
    let temp = Builder::new().prefix("ymp-corpus-").tempdir()?;
    let root = temp.path().join("candidate");
    fs::create_dir(&root)?;
    let (snapshot, lock) = match base {
        CandidateBase::Source => (&task.snapshots.source, &task.artifacts.source_lock),
        CandidateBase::Fixed => (&task.snapshots.fixed, &task.artifacts.fixed_lock),
    };
    let archive = cache_object_path(cache, &snapshot.archive_sha256);
    validate_archive_entries(&archive)?;
    run_setup(
        Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&root),
        "extract source archive",
    )?;
    reject_symlinks(&root)?;
    fs::copy(corpus.root.join(&lock.path), root.join("Cargo.lock"))
        .context("install pinned Cargo.lock")?;

    let cargo_home = temp.path().join("cargo-home");
    let target_dir = temp.path().join("target");
    fs::create_dir(&cargo_home)?;
    if configure_offline {
        let config_dir = root.join(".cargo");
        fs::create_dir_all(&config_dir)?;
        let vendor = cache_tree_path(cache, &task.environment.vendor_tree_sha256)
            .canonicalize()
            .context("canonicalize vendor tree")?;
        let vendor_string = vendor.to_string_lossy();
        let quoted = serde_json::to_string(vendor_string.as_ref())?;
        let mut config = String::from("[source.crates-io]\nreplace-with = \"vendored-sources\"\n");
        for source in &task.environment.vendor_git_sources {
            let source_key = serde_json::to_string(&format!("git+{source}"))?;
            let source_url = serde_json::to_string(source)?;
            config.push_str(&format!(
                "\n[source.{source_key}]\ngit = {source_url}\nreplace-with = \"vendored-sources\"\n"
            ));
        }
        config.push_str(&format!(
            "\n[source.vendored-sources]\ndirectory = {quoted}\n\n[net]\noffline = true\n"
        ));
        fs::write(config_dir.join("config.toml"), config)?;
    }
    Ok(PreparedCandidate {
        _temp: temp,
        root,
        cargo_home,
        target_dir,
    })
}

fn validate_archive_entries(archive: &Path) -> Result<()> {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(archive)
        .output()
        .context("list source archive")?;
    ensure!(output.status.success(), "cannot list source archive");
    let listing = String::from_utf8(output.stdout).context("archive paths are not UTF-8")?;
    for entry in listing.lines() {
        validate_relative_path(Path::new(entry))
            .with_context(|| format!("unsafe archive entry {entry:?}"))?;
    }
    Ok(())
}

fn reject_symlinks(root: &Path) -> Result<()> {
    for path in recursive_paths(root)? {
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "source archive contains symlink {}",
            path.display()
        );
    }
    Ok(())
}

fn inject_oracle(corpus: &LoadedCorpus, task: &Task, candidate: &Path) -> Result<()> {
    let destination = candidate.join(&task.environment.oracle_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(corpus.root.join(&task.artifacts.oracle.path), &destination)
        .with_context(|| format!("inject protected oracle at {}", destination.display()))?;
    Ok(())
}

fn reverse_patch(corpus: &LoadedCorpus, candidate: &Path, patch: &ArtifactRef) -> Result<()> {
    let patch = corpus.root.join(&patch.path);
    run_setup(
        Command::new("git")
            .arg("apply")
            .arg("--unidiff-zero")
            .arg("--reverse")
            .arg("--check")
            .arg(&patch)
            .current_dir(candidate),
        "check invalid control patch",
    )?;
    run_setup(
        Command::new("git")
            .arg("apply")
            .arg("--unidiff-zero")
            .arg("--reverse")
            .arg(&patch)
            .current_dir(candidate),
        "apply invalid control patch",
    )
}

fn run_expected_pass(
    name: &str,
    candidate_name: &str,
    requirement: Option<&Requirement>,
    spec: &CommandSpec,
    candidate: &PreparedCandidate,
    timeout: Duration,
) -> Result<CheckReport> {
    let command = command_vector(spec, None);
    let result = run_task_command(spec, None, candidate, timeout)?;
    let observation = if result.timed_out {
        "infrastructure_error"
    } else if result.status.is_some_and(|status| status.success()) {
        "passed"
    } else {
        "failed"
    };
    Ok(check_report(
        name,
        candidate_name,
        requirement.map(|value| value.id.clone()),
        command,
        "pass",
        observation,
        result,
    ))
}

fn run_expected_failure(
    name: &str,
    candidate_name: &str,
    requirement: &Requirement,
    spec: &CommandSpec,
    candidate: &PreparedCandidate,
    timeout: Duration,
) -> Result<CheckReport> {
    let test = requirement.protected_test.as_str();
    let command = command_vector(spec, Some(test));
    let result = run_task_command(spec, Some(test), candidate, timeout)?;
    let combined = format!("{}\n{}", result.stdout_tail, result.stderr_tail);
    let control = if result.timed_out || result.status.is_none() {
        ControlObservation::InfrastructureError
    } else if result.status.is_some_and(|status| status.success()) {
        ControlObservation::UnexpectedPass
    } else if combined.contains(test) {
        ControlObservation::ExpectedFailure
    } else {
        ControlObservation::InfrastructureError
    };
    let observation = match control {
        ControlObservation::ExpectedFailure => "expected_failure",
        ControlObservation::UnexpectedPass => "unexpected_pass",
        ControlObservation::InfrastructureError => "infrastructure_error",
    };
    Ok(check_report(
        name,
        candidate_name,
        Some(requirement.id.clone()),
        command,
        "fail_in_named_protected_test",
        observation,
        result,
    ))
}

fn command_vector(spec: &CommandSpec, test: Option<&str>) -> Vec<String> {
    let mut command = vec![spec.program.clone()];
    command.extend(spec.args.iter().cloned());
    if let Some(test) = test {
        command.extend(["--".to_owned(), test.to_owned(), "--exact".to_owned()]);
    }
    command
}

fn run_task_command(
    spec: &CommandSpec,
    test: Option<&str>,
    candidate: &PreparedCandidate,
    timeout: Duration,
) -> Result<CommandResult> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&candidate.root)
        .env("CARGO_HOME", &candidate.cargo_home)
        .env("CARGO_TARGET_DIR", &candidate.target_dir)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(test) = test {
        command.arg("--").arg(test).arg("--exact");
    }
    run_bounded(&mut command, timeout)
}

fn run_bounded(command: &mut Command, timeout: Duration) -> Result<CommandResult> {
    let stdout = NamedTempFile::new().context("create stdout capture")?;
    let stderr = NamedTempFile::new().context("create stderr capture")?;
    command.stdout(Stdio::from(stdout.reopen()?));
    command.stderr(Stdio::from(stderr.reopen()?));
    let started = Instant::now();
    let mut child = command.spawn().context("start task command")?;
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().context("poll task command")? {
            break (Some(status), false);
        }
        if started.elapsed() >= timeout {
            child.kill().context("terminate timed-out task command")?;
            let status = child.wait().context("reap timed-out task command")?;
            break (Some(status), true);
        }
        thread::sleep(Duration::from_millis(50));
    };
    Ok(CommandResult {
        status,
        timed_out,
        duration: started.elapsed(),
        stdout_tail: read_tail(stdout.path(), OUTPUT_LIMIT)?,
        stderr_tail: read_tail(stderr.path(), OUTPUT_LIMIT)?,
    })
}

fn read_tail(path: &Path, limit: usize) -> Result<String> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(limit);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

#[allow(clippy::too_many_arguments)]
fn check_report(
    name: &str,
    candidate: &str,
    requirement: Option<String>,
    command: Vec<String>,
    expectation: &str,
    observation: &str,
    result: CommandResult,
) -> CheckReport {
    CheckReport {
        name: name.to_owned(),
        candidate: candidate.to_owned(),
        requirement,
        command,
        expectation: expectation.to_owned(),
        observation: observation.to_owned(),
        exit_code: result.status.and_then(|status| status.code()),
        duration_ms: result.duration.as_millis(),
        stdout_tail: result.stdout_tail,
        stderr_tail: result.stderr_tail,
    }
}

fn check_toolchain(corpus: &LoadedCorpus) -> Result<ReproductionEnvironment> {
    let rustc = command_stdout("rustc", ["--version", "--verbose"])?;
    let cargo = command_stdout("cargo", ["--version", "--verbose"])?;
    let git = command_stdout("git", ["--version"])?;
    let archive_tool = command_stdout("tar", ["--version"])?;
    let host = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .context("rustc did not report a host")?;
    for task in &corpus.tasks {
        ensure!(
            rustc.lines().next() == Some(task.environment.rustc_version.as_str()),
            "rustc version does not match {}",
            task.id
        );
        ensure!(
            cargo.lines().next() == Some(task.environment.cargo_version.as_str()),
            "cargo version does not match {}",
            task.id
        );
        ensure!(
            host == task.environment.host,
            "host does not match {}",
            task.id
        );
        ensure!(
            git.trim() == task.environment.git_version,
            "git version does not match {}",
            task.id
        );
        ensure!(
            archive_tool.lines().next().map(str::trim)
                == Some(task.environment.archive_tool_version.as_str()),
            "archive tool version does not match {}",
            task.id
        );
    }
    Ok(ReproductionEnvironment {
        rustc_version: rustc.lines().next().unwrap_or_default().to_owned(),
        cargo_version: cargo.lines().next().unwrap_or_default().to_owned(),
        host: host.to_owned(),
        git_version: git.trim().to_owned(),
        archive_tool_version: archive_tool
            .lines()
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned(),
    })
}

fn command_stdout<I, S>(program: &str, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program).args(args).output()?;
    ensure!(output.status.success(), "{program} version probe failed");
    String::from_utf8(output.stdout).with_context(|| format!("{program} output is not UTF-8"))
}

fn cache_object_path(cache: &Path, digest: &str) -> PathBuf {
    cache.join("objects").join("sha256").join(digest)
}

fn cache_tree_path(cache: &Path, digest: &str) -> PathBuf {
    cache.join("trees").join("sha256").join(digest)
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn tree_digest(root: &Path) -> Result<String> {
    ensure!(root.is_dir(), "tree root is not a directory");
    let mut files = recursive_paths(root)?
        .into_iter()
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort_by_key(|path| normalized_relative(root, path));
    let mut hasher = Sha256::new();
    hasher.update(TREE_DOMAIN);
    for path in files {
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(!metadata.file_type().is_symlink(), "tree contains symlink");
        let relative = normalized_relative(root, &path);
        let bytes = fs::read(&path)?;
        hasher.update(b"F\0");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes.len().to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn normalized_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("recursive path has root prefix")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn recursive_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path)?;
            result.push(path.clone());
            if metadata.is_dir() {
                pending.push(path);
            }
        }
    }
    Ok(result)
}

pub fn write_report(path: &Path, report: &ReproductionReport) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("write report {}", path.display()))
}

pub fn report_json<T: Serialize>(report: &T) -> Result<String> {
    serde_json::to_string_pretty(report).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        ControlObservation, negative_control_keeps_package_usable, sha256_bytes, tree_digest,
        validate_relative_path,
    };

    #[test]
    fn unexpected_negative_pass_invalidates_package() {
        assert!(!negative_control_keeps_package_usable(
            ControlObservation::UnexpectedPass
        ));
    }

    #[test]
    fn expected_negative_failure_preserves_package() {
        assert!(negative_control_keeps_package_usable(
            ControlObservation::ExpectedFailure
        ));
    }

    #[test]
    fn infrastructure_error_invalidates_package() {
        assert!(!negative_control_keeps_package_usable(
            ControlObservation::InfrastructureError
        ));
    }

    #[test]
    fn artifact_paths_cannot_escape_corpus() {
        assert!(validate_relative_path("tasks/a.json".as_ref()).is_ok());
        assert!(validate_relative_path("../oracle".as_ref()).is_err());
        assert!(validate_relative_path("/tmp/oracle".as_ref()).is_err());
    }

    #[test]
    fn tree_digest_is_content_and_path_sensitive() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        fs::create_dir(first.path().join("nested")).unwrap();
        fs::create_dir(second.path().join("nested")).unwrap();
        fs::write(first.path().join("nested/file"), b"bytes").unwrap();
        fs::write(second.path().join("nested/file"), b"bytes").unwrap();
        assert_eq!(
            tree_digest(first.path()).unwrap(),
            tree_digest(second.path()).unwrap()
        );
        fs::write(second.path().join("nested/file"), b"changed").unwrap();
        assert_ne!(
            tree_digest(first.path()).unwrap(),
            tree_digest(second.path()).unwrap()
        );
    }

    #[test]
    fn committed_registry_is_internally_consistent() {
        let root = committed_corpus();
        let corpus = super::load_corpus(&root).unwrap();
        assert_eq!(corpus.tasks.len(), 4);
        assert!(!corpus.registry.statistically_sufficient);
        assert!(!corpus.registry.study_frozen);
    }

    #[test]
    fn changed_linked_artifact_is_rejected_by_public_binding() {
        let temp = tempdir().unwrap();
        copy_tree(&committed_corpus(), temp.path());
        let oracle_path = temp
            .path()
            .join("tasks/bstr-eof-and-debug/protected/oracle.rs");
        let mut oracle = fs::read(&oracle_path).unwrap();
        oracle.push(b' ');
        fs::write(&oracle_path, &oracle).unwrap();

        let manifest_path = temp.path().join("tasks/bstr-eof-and-debug/manifest.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["artifacts"]["oracle"]["sha256"] = Value::String(sha256_bytes(&oracle));
        write_json(&manifest_path, &manifest);

        let registry_path = temp.path().join("registry.json");
        let mut registry: Value =
            serde_json::from_slice(&fs::read(&registry_path).unwrap()).unwrap();
        registry["tasks"][0]["manifest"]["sha256"] =
            Value::String(sha256_bytes(&fs::read(&manifest_path).unwrap()));
        write_json(&registry_path, &registry);

        let error = super::load_corpus(temp.path()).unwrap_err();
        assert!(error.to_string().contains("public artifact binding"));
    }

    #[test]
    fn one_byte_change_fails_before_candidate_execution() {
        let temp = tempdir().unwrap();
        copy_tree(&committed_corpus(), temp.path());
        let oracle_path = temp
            .path()
            .join("tasks/bstr-eof-and-debug/protected/oracle.rs");
        let mut oracle = fs::read(&oracle_path).unwrap();
        oracle[0] ^= 1;
        fs::write(&oracle_path, oracle).unwrap();
        let error = super::load_corpus(temp.path()).unwrap_err();
        assert!(error.to_string().contains("artifact digest mismatch"));
    }

    fn committed_corpus() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
    }

    fn copy_tree(source: &Path, destination: &Path) {
        for path in super::recursive_paths(source).unwrap() {
            let relative = path.strip_prefix(source).unwrap();
            let target = destination.join(relative);
            if path.is_dir() {
                fs::create_dir_all(&target).unwrap();
            } else {
                fs::copy(path, target).unwrap();
            }
        }
    }

    fn write_json(path: &Path, value: &Value) {
        let mut bytes = serde_json::to_vec_pretty(value).unwrap();
        bytes.push(b'\n');
        fs::write(path, bytes).unwrap();
    }
}

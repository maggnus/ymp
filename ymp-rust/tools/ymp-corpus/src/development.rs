use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile};

use crate::{report_json, sha256_file, tree_digest};

const CORPUS_ID: &str = "weak-diagnostic-v1-l4plus";
const SEED_ROOT: &str = "ymp-weak-diagnostic-v1-l4plus";
const FROZEN_MANIFEST_SHA256: &str =
    "085bdd3b915faf61545283ad1ec13e7e693398ebf28eded0460e51c122243696";
const FROZEN_PROTECTED_MANIFEST_SHA256: &str =
    "33b0559d85ef27ac47d1188c13093a70f4365247d3652421930fa29d951fabd1";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const OUTPUT_LIMIT: usize = 16 * 1024;

#[derive(Debug, Subcommand)]
pub enum DevelopmentCommand {
    Check {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
    },
    Prepare {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    Verify {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        project_root: PathBuf,
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        ymp_home: PathBuf,
        #[arg(long)]
        tmp: PathBuf,
        #[arg(long)]
        build: PathBuf,
        #[arg(long)]
        export: PathBuf,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    Mutation {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        case: String,
        #[arg(long)]
        output: PathBuf,
    },
    Inventory {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        paths: PathBuf,
    },
    Validate {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
        #[arg(long)]
        root: PathBuf,
    },
}

impl DevelopmentCommand {
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Check { manifest, digest } => {
                let package = load_development_package(manifest, digest)?;
                println!("{}", report_json(&compliance_report(&package))?);
            }
            Self::Prepare {
                manifest,
                digest,
                task_id,
                output,
            } => {
                let package = load_development_package(manifest, digest)?;
                println!(
                    "{}",
                    report_json(&prepare_task(&package, task_id, output)?)?
                );
            }
            Self::Verify {
                manifest,
                digest,
                task_id,
                candidate,
                project_root,
                home,
                ymp_home,
                tmp,
                build,
                export,
                report,
            } => {
                let package = load_development_package(manifest, digest)?;
                let roots = IsolationRoots {
                    project: project_root.clone(),
                    home: home.clone(),
                    ymp_home: ymp_home.clone(),
                    tmp: tmp.clone(),
                    build: build.clone(),
                    export: export.clone(),
                };
                let result =
                    verify_candidate(&package, task_id, candidate, &roots, report.as_deref())?;
                println!("{}", report_json(&result)?);
                if !result.passed {
                    bail!(
                        "{}: exact protected verifier rejected candidate",
                        result
                            .failure_reason
                            .as_deref()
                            .unwrap_or("UNKNOWN_FAILURE")
                    );
                }
            }
            Self::Mutation {
                manifest,
                digest,
                case,
                output,
            } => {
                let package = load_development_package(manifest, digest)?;
                println!(
                    "{}",
                    report_json(&materialize_mutation(&package, case, output)?)?
                );
            }
            Self::Inventory {
                manifest,
                digest,
                paths,
            } => {
                let package = load_development_package(manifest, digest)?;
                println!("{}", report_json(&verify_inventory(&package, paths)?)?);
            }
            Self::Validate {
                manifest,
                digest,
                root,
            } => {
                let package = load_development_package(manifest, digest)?;
                println!("{}", report_json(&validate_all(&package, root)?)?);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentManifest {
    schema_version: u32,
    corpus_id: String,
    artifact_role: String,
    freeze_state: String,
    model_calls_at_freeze: u64,
    primary_tasks_or_seeds_consumed: bool,
    seed_namespace_root: String,
    protected_manifest_sha256: String,
    splits: FrozenSplits,
    write_zone: WriteZone,
    tasks: Vec<DevelopmentTask>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenSplits {
    development: Vec<String>,
    transfer: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WriteZone {
    new_prefix: String,
    new_files: Vec<String>,
    additive_seams: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DevelopmentTask {
    id: String,
    source_sha256: String,
    public_contract_sha256: String,
    protected_oracle_sha256: String,
    reference_candidate_sha256: String,
    stratum: String,
    split: String,
    seed_namespace: String,
    dependency_order: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedManifest {
    schema_version: u32,
    corpus_id: String,
    task_bindings: Vec<ProtectedTask>,
    mutations: Vec<Mutation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedTask {
    task_id: String,
    frozen_stratum: String,
    oracle: ArtifactRef,
    reference: ArtifactRef,
    oracles: Vec<OracleSpec>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OracleSpec {
    id: String,
    test: String,
    failure_reason: String,
    blind_spot: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRef {
    path: PathBuf,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    id: String,
    kind: String,
    task_id: String,
    target: PathBuf,
    artifact: ArtifactRef,
    expected: ArtifactRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFailure {
    schema_version: u32,
    mutation_id: String,
    command: String,
    expected_exit: String,
    reason: String,
}

#[derive(Debug)]
pub struct LoadedDevelopmentPackage {
    root: PathBuf,
    digest: String,
    manifest: DevelopmentManifest,
    protected: ProtectedManifest,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlindSpot {
    pub task_id: String,
    pub oracle_id: String,
    pub statement: String,
}

#[derive(Debug, Serialize)]
pub struct ComplianceReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub manifest_sha256: String,
    pub protected_manifest_sha256: String,
    pub freeze_state: String,
    pub model_calls: u64,
    pub primary_tasks_or_seeds_consumed: bool,
    pub development_tasks: Vec<String>,
    pub transfer_tasks: Vec<String>,
    pub seed_namespaces: Vec<String>,
    pub blind_spots: Vec<BlindSpot>,
}

#[derive(Clone, Debug)]
pub struct IsolationRoots {
    pub project: PathBuf,
    pub home: PathBuf,
    pub ymp_home: PathBuf,
    pub tmp: PathBuf,
    pub build: PathBuf,
    pub export: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct PreparationReport {
    pub schema_version: u32,
    pub task_id: String,
    pub source_sha256: String,
    pub output: PathBuf,
    pub model_calls: u64,
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub command: String,
    pub success: bool,
    pub status: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct VerificationReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub manifest_sha256: String,
    pub task_id: String,
    pub candidate_sha256: String,
    pub passed: bool,
    pub failure_reason: Option<String>,
    pub model_calls: u64,
    pub protected_files_in_project: bool,
    pub checks: Vec<CheckResult>,
    pub blind_spots: Vec<BlindSpot>,
}

#[derive(Debug, Serialize)]
pub struct MutationMaterialization {
    pub schema_version: u32,
    pub mutation_id: String,
    pub task_id: String,
    pub kind: String,
    pub mutation_sha256: String,
    pub expected_result_sha256: String,
    pub expected_reason: String,
    pub output: PathBuf,
    pub model_calls: u64,
}

#[derive(Debug, Serialize)]
pub struct InventoryReport {
    pub schema_version: u32,
    pub accepted: bool,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MutationObservation {
    mutation_id: String,
    mutation_sha256: String,
    expected_result_sha256: String,
    observed_nonzero: bool,
    observed_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub manifest_sha256: String,
    pub model_calls: u64,
    pub roots: BTreeMap<String, PathBuf>,
    pub prepared_source_digests: BTreeMap<String, String>,
    pub seeded_source_failures: BTreeMap<String, String>,
    pub reference_results: BTreeMap<String, bool>,
    pub mutations: Vec<MutationObservation>,
    pub blind_spots: Vec<BlindSpot>,
}

pub fn load_development_package(
    manifest_path: &Path,
    digest_path: &Path,
) -> Result<LoadedDevelopmentPackage> {
    let bytes = fs::read(manifest_path)
        .with_context(|| format!("read development manifest {}", manifest_path.display()))?;
    let supplied = fs::read_to_string(digest_path)
        .with_context(|| format!("read development digest {}", digest_path.display()))?;
    let digest = sha256_bytes(&bytes);
    validate_sha256(supplied.trim(), "development manifest sidecar")?;
    ensure!(
        supplied.trim() == digest,
        "development manifest digest mismatch"
    );
    let manifest: DevelopmentManifest =
        serde_json::from_slice(&bytes).context("parse development manifest")?;
    let root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let protected_path = root.join("protected/manifest.json");
    let protected_bytes = fs::read(&protected_path)
        .with_context(|| format!("read protected manifest {}", protected_path.display()))?;
    let protected: ProtectedManifest =
        serde_json::from_slice(&protected_bytes).context("parse protected development manifest")?;
    for task in &manifest.tasks {
        if let Some(binding) = protected
            .task_bindings
            .iter()
            .find(|binding| binding.task_id == task.id)
        {
            ensure!(
                binding.frozen_stratum == task.stratum,
                "STRATUM_RELABEL: task {} is frozen as {}",
                task.id,
                binding.frozen_stratum
            );
        }
    }
    ensure!(
        digest == FROZEN_MANIFEST_SHA256,
        "development manifest changed after freeze"
    );
    ensure!(
        manifest.protected_manifest_sha256 == FROZEN_PROTECTED_MANIFEST_SHA256,
        "protected manifest binding changed after freeze"
    );
    ensure!(
        sha256_bytes(&protected_bytes) == FROZEN_PROTECTED_MANIFEST_SHA256,
        "protected manifest changed after freeze"
    );
    let package = LoadedDevelopmentPackage {
        root,
        digest,
        manifest,
        protected,
    };
    validate_package(&package)?;
    Ok(package)
}

pub fn compliance_report(package: &LoadedDevelopmentPackage) -> ComplianceReport {
    ComplianceReport {
        schema_version: 1,
        corpus_id: package.manifest.corpus_id.clone(),
        manifest_sha256: package.digest.clone(),
        protected_manifest_sha256: package.manifest.protected_manifest_sha256.clone(),
        freeze_state: package.manifest.freeze_state.clone(),
        model_calls: 0,
        primary_tasks_or_seeds_consumed: package.manifest.primary_tasks_or_seeds_consumed,
        development_tasks: package.manifest.splits.development.clone(),
        transfer_tasks: package.manifest.splits.transfer.clone(),
        seed_namespaces: package
            .manifest
            .tasks
            .iter()
            .map(|task| task.seed_namespace.clone())
            .collect(),
        blind_spots: blind_spots(package),
    }
}

fn validate_package(package: &LoadedDevelopmentPackage) -> Result<()> {
    let manifest = &package.manifest;
    ensure!(
        manifest.schema_version == 1,
        "unsupported development schema"
    );
    ensure!(
        manifest.corpus_id == CORPUS_ID,
        "unexpected development corpus id"
    );
    ensure!(
        manifest.artifact_role == "development_only_held_out_corpus",
        "development corpus claims an unsupported role"
    );
    ensure!(
        manifest.freeze_state == "frozen_before_model_call" && manifest.model_calls_at_freeze == 0,
        "development corpus was not frozen before model use"
    );
    ensure!(
        !manifest.primary_tasks_or_seeds_consumed,
        "development corpus consumes a primary task or seed"
    );
    ensure!(
        manifest.seed_namespace_root == SEED_ROOT,
        "unexpected development seed namespace"
    );
    ensure!(
        package.protected.schema_version == 1 && package.protected.corpus_id == CORPUS_ID,
        "protected manifest identity mismatch"
    );
    validate_write_zone_declaration(&manifest.write_zone)?;

    let mut ids = BTreeSet::new();
    let mut seeds = BTreeSet::new();
    let mut strata = BTreeSet::new();
    for task in &manifest.tasks {
        validate_id(&task.id)?;
        ensure!(ids.insert(task.id.clone()), "duplicate development task id");
        if let Some(binding) = package
            .protected
            .task_bindings
            .iter()
            .find(|binding| binding.task_id == task.id)
        {
            ensure!(
                binding.frozen_stratum == task.stratum,
                "STRATUM_RELABEL: task {} is frozen as {}",
                task.id,
                binding.frozen_stratum
            );
        }
        validate_sha256(&task.source_sha256, "task source digest")?;
        validate_sha256(&task.public_contract_sha256, "public contract digest")?;
        validate_sha256(&task.protected_oracle_sha256, "protected oracle digest")?;
        validate_sha256(&task.reference_candidate_sha256, "reference digest")?;
        ensure!(
            task.seed_namespace == format!("{SEED_ROOT}/{}/{}", task.split, task.id),
            "task seed namespace is not derived from the development namespace"
        );
        ensure!(
            seeds.insert(task.seed_namespace.clone()),
            "seed namespace collision"
        );
        ensure!(
            !task.seed_namespace.contains("calibration")
                && !task.seed_namespace.contains("primary")
                && !task.seed_namespace.contains("ymp-study-seed-v1"),
            "development seed namespace collides with calibration or primary use"
        );
        ensure!(
            matches!(task.split.as_str(), "development" | "transfer"),
            "unknown development split"
        );
        ensure!(
            matches!(
                task.stratum.as_str(),
                "decomposable" | "strongly_sequential_expected_null"
            ),
            "unknown L4+ stratum"
        );
        if task.stratum == "decomposable" {
            ensure!(
                task.dependency_order.is_empty(),
                "decomposable task declares a dependency order"
            );
        } else {
            ensure!(
                task.dependency_order
                    == [
                        "decode",
                        "predecessor_validation",
                        "transition_validation",
                        "commit_identity",
                    ],
                "sequential/null dependency order changed"
            );
        }
        strata.insert(task.stratum.clone());
        let public = package.root.join("public").join(&task.id);
        ensure!(
            tree_digest(&public)? == task.source_sha256,
            "task source digest mismatch: {}",
            task.id
        );
        ensure!(
            sha256_file(&public.join("TASK.md"))? == task.public_contract_sha256,
            "public contract digest mismatch: {}",
            task.id
        );
        ensure!(
            fs::read(public.join("TASK.md"))? == fs::read(public.join("project/TASK.md"))?,
            "prepared project contract differs from package contract: {}",
            task.id
        );
    }
    ensure!(
        strata
            == BTreeSet::from([
                "decomposable".to_owned(),
                "strongly_sequential_expected_null".to_owned(),
            ]),
        "development corpus lacks a required L4+ stratum"
    );
    validate_splits(manifest, &ids)?;
    validate_protected_bindings(package, &ids)?;
    validate_mutations(package)?;
    Ok(())
}

fn validate_splits(manifest: &DevelopmentManifest, ids: &BTreeSet<String>) -> Result<()> {
    ensure!(
        !manifest.splits.development.is_empty() && !manifest.splits.transfer.is_empty(),
        "development/transfer split must have both partitions"
    );
    let development = manifest
        .splits
        .development
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let transfer = manifest
        .splits
        .transfer
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        development.len() == manifest.splits.development.len()
            && transfer.len() == manifest.splits.transfer.len(),
        "duplicate task in frozen split"
    );
    ensure!(
        development.is_disjoint(&transfer),
        "task occurs in both frozen splits"
    );
    ensure!(
        development
            .union(&transfer)
            .cloned()
            .collect::<BTreeSet<_>>()
            == *ids,
        "frozen splits do not partition tasks"
    );
    for task in &manifest.tasks {
        let expected = if development.contains(&task.id) {
            "development"
        } else {
            "transfer"
        };
        ensure!(
            task.split == expected,
            "task split label differs from frozen partition"
        );
    }
    Ok(())
}

fn validate_protected_bindings(
    package: &LoadedDevelopmentPackage,
    ids: &BTreeSet<String>,
) -> Result<()> {
    let mut protected_ids = BTreeSet::new();
    for binding in &package.protected.task_bindings {
        ensure!(
            protected_ids.insert(binding.task_id.clone()),
            "duplicate protected task binding"
        );
        let task = task(package, &binding.task_id)?;
        ensure!(
            binding.frozen_stratum == task.stratum,
            "STRATUM_RELABEL: task {} is frozen as {}",
            task.id,
            binding.frozen_stratum
        );
        verify_file_artifact(&package.root, &binding.oracle)?;
        verify_tree_artifact(&package.root, &binding.reference)?;
        ensure!(
            binding.oracle.sha256 == task.protected_oracle_sha256,
            "task oracle binding differs from protected manifest"
        );
        ensure!(
            binding.reference.sha256 == task.reference_candidate_sha256,
            "task reference binding differs from protected manifest"
        );
        let oracle_ids = binding
            .oracles
            .iter()
            .map(|oracle| oracle.id.as_str())
            .collect::<BTreeSet<_>>();
        if task.stratum == "decomposable" {
            ensure!(
                oracle_ids == BTreeSet::from(["branch_a", "branch_b", "integration"]),
                "decomposable task must expose two branch oracles and one integration oracle"
            );
        } else {
            ensure!(
                oracle_ids.contains("dependency_order"),
                "sequential/null task has no dependency-order oracle"
            );
        }
        for oracle in &binding.oracles {
            validate_oracle_id(&oracle.id)?;
            ensure!(
                !oracle.test.is_empty()
                    && !oracle.failure_reason.is_empty()
                    && !oracle.blind_spot.is_empty(),
                "protected oracle metadata is incomplete"
            );
        }
    }
    ensure!(
        protected_ids == *ids,
        "protected task bindings do not match frozen tasks"
    );
    Ok(())
}

fn validate_mutations(package: &LoadedDevelopmentPackage) -> Result<()> {
    let expected_ids = BTreeSet::from([
        "branch-a-omission",
        "branch-b-omission",
        "integration-bypass",
        "stratum-relabel",
    ]);
    let actual = package
        .protected
        .mutations
        .iter()
        .map(|mutation| mutation.id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(actual == expected_ids, "protected mutation set changed");
    let mut expected_digests = BTreeSet::new();
    for mutation in &package.protected.mutations {
        verify_file_artifact(&package.root, &mutation.artifact)?;
        verify_file_artifact(&package.root, &mutation.expected)?;
        ensure!(
            expected_digests.insert(mutation.expected.sha256.clone()),
            "mutations share an expected-result digest"
        );
        let expected = expected_failure(package, mutation)?;
        ensure!(
            expected.schema_version == 1 && expected.mutation_id == mutation.id,
            "expected result identity mismatch"
        );
        ensure!(
            expected.expected_exit == "nonzero",
            "negative control does not require nonzero exit"
        );
        let (kind, command, target, reason) = match mutation.id.as_str() {
            "branch-a-omission" => (
                "candidate_file_replacement",
                "development verify",
                "src/inventory.rs",
                "BRANCH_A_OMISSION",
            ),
            "branch-b-omission" => (
                "candidate_file_replacement",
                "development verify",
                "src/routing.rs",
                "BRANCH_B_OMISSION",
            ),
            "integration-bypass" => (
                "candidate_file_replacement",
                "development verify",
                "src/lib.rs",
                "INTEGRATION_BYPASS",
            ),
            "stratum-relabel" => (
                "manifest_stratum_relabel",
                "development check",
                "tasks[].stratum",
                "STRATUM_RELABEL",
            ),
            _ => unreachable!(),
        };
        ensure!(
            mutation.kind == kind
                && expected.command == command
                && mutation.target == Path::new(target)
                && expected.reason == reason,
            "mutation contract changed: {}",
            mutation.id
        );
    }
    Ok(())
}

pub fn prepare_task(
    package: &LoadedDevelopmentPackage,
    task_id: &str,
    output: &Path,
) -> Result<PreparationReport> {
    let task = task(package, task_id)?;
    ensure!(
        !output.exists(),
        "refusing to overwrite prepared path {}",
        output.display()
    );
    copy_tree(
        &package.root.join("public").join(task_id).join("project"),
        output,
    )?;
    Ok(PreparationReport {
        schema_version: 1,
        task_id: task_id.to_owned(),
        source_sha256: task.source_sha256.clone(),
        output: output.to_path_buf(),
        model_calls: 0,
    })
}

pub fn verify_candidate(
    package: &LoadedDevelopmentPackage,
    task_id: &str,
    candidate: &Path,
    roots: &IsolationRoots,
    report_path: Option<&Path>,
) -> Result<VerificationReport> {
    validate_isolation(roots, candidate, report_path)?;
    let binding = protected_task(package, task_id)?;
    validate_candidate_paths(package, task_id, candidate)?;
    let digest = tree_digest(candidate)?;
    let private = Builder::new()
        .prefix("ymp-l4-verifier-")
        .tempdir_in(&roots.tmp)
        .context("create sibling protected verifier root")?;
    copy_tree(candidate, private.path())?;
    let oracle_target = private.path().join("tests/ymp_protected.rs");
    fs::create_dir_all(oracle_target.parent().expect("oracle target has parent"))?;
    fs::copy(package.root.join(&binding.oracle.path), &oracle_target)
        .with_context(|| format!("inject protected oracle for {task_id}"))?;

    let target = roots.build.join(task_id).join(&digest);
    fs::create_dir_all(&target)?;
    let mut checks = Vec::new();
    let mut failure_reason = None;
    for (label, args, reason) in [
        ("cargo fmt --check", vec!["fmt", "--check"], "FORMAT_CHECK"),
        (
            "cargo test --locked --offline --test visible",
            vec!["test", "--locked", "--offline", "--test", "visible"],
            "VISIBLE_CHECK",
        ),
    ] {
        let check = run_cargo(private.path(), roots, &target, label, &args)?;
        if !check.success {
            failure_reason = Some(reason.to_owned());
            checks.push(check);
            break;
        }
        checks.push(check);
    }
    if failure_reason.is_none() {
        for oracle in &binding.oracles {
            let label = format!("protected oracle {}", oracle.id);
            let args = [
                "test",
                "--locked",
                "--offline",
                "--test",
                "ymp_protected",
                &oracle.test,
                "--",
                "--exact",
            ];
            let check = run_cargo(private.path(), roots, &target, &label, &args)?;
            if !check.success {
                failure_reason = Some(oracle.failure_reason.clone());
                checks.push(check);
                break;
            }
            checks.push(check);
        }
    }
    let report = VerificationReport {
        schema_version: 1,
        corpus_id: CORPUS_ID.to_owned(),
        manifest_sha256: package.digest.clone(),
        task_id: task_id.to_owned(),
        candidate_sha256: digest,
        passed: failure_reason.is_none(),
        failure_reason,
        model_calls: 0,
        protected_files_in_project: false,
        checks,
        blind_spots: binding
            .oracles
            .iter()
            .map(|oracle| BlindSpot {
                task_id: task_id.to_owned(),
                oracle_id: oracle.id.clone(),
                statement: oracle.blind_spot.clone(),
            })
            .collect(),
    };
    if let Some(path) = report_path {
        write_json(path, &report)?;
    }
    Ok(report)
}

pub fn materialize_mutation(
    package: &LoadedDevelopmentPackage,
    mutation_id: &str,
    output: &Path,
) -> Result<MutationMaterialization> {
    ensure!(
        !output.exists(),
        "refusing to overwrite mutation output {}",
        output.display()
    );
    let mutation = mutation(package, mutation_id)?;
    let expected = expected_failure(package, mutation)?;
    if mutation.kind == "candidate_file_replacement" {
        let binding = protected_task(package, &mutation.task_id)?;
        copy_tree(&package.root.join(&binding.reference.path), output)?;
        let target = output.join(&mutation.target);
        ensure!(target.is_file(), "mutation target does not exist");
        fs::copy(package.root.join(&mutation.artifact.path), &target)
            .with_context(|| format!("apply mutation {}", mutation.id))?;
    } else {
        copy_tree(&package.root, output)?;
        let mutation_value: Value =
            serde_json::from_slice(&fs::read(package.root.join(&mutation.artifact.path))?)?;
        let task_id = mutation_value
            .pointer("/task_id")
            .and_then(Value::as_str)
            .context("stratum mutation task id is missing")?;
        let replacement = mutation_value
            .pointer("/to")
            .and_then(Value::as_str)
            .context("stratum mutation replacement is missing")?;
        let manifest_path = output.join("manifest.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        let tasks = value
            .pointer_mut("/tasks")
            .and_then(Value::as_array_mut)
            .context("development tasks are missing")?;
        let task = tasks
            .iter_mut()
            .find(|task| task.pointer("/id").and_then(Value::as_str) == Some(task_id))
            .context("stratum mutation task is absent")?;
        task.as_object_mut()
            .context("mutated task is not an object")?
            .insert("stratum".to_owned(), Value::String(replacement.to_owned()));
        let mut bytes = serde_json::to_vec_pretty(&value)?;
        bytes.push(b'\n');
        fs::write(&manifest_path, &bytes)?;
        fs::write(
            output.join("manifest.sha256"),
            format!("{}\n", sha256_bytes(&bytes)),
        )?;
    }
    Ok(MutationMaterialization {
        schema_version: 1,
        mutation_id: mutation.id.clone(),
        task_id: mutation.task_id.clone(),
        kind: mutation.kind.clone(),
        mutation_sha256: mutation.artifact.sha256.clone(),
        expected_result_sha256: mutation.expected.sha256.clone(),
        expected_reason: expected.reason,
        output: output.to_path_buf(),
        model_calls: 0,
    })
}

pub fn verify_inventory(
    package: &LoadedDevelopmentPackage,
    paths_file: &Path,
) -> Result<InventoryReport> {
    let text = fs::read_to_string(paths_file)
        .with_context(|| format!("read write-zone inventory {}", paths_file.display()))?;
    verify_inventory_with_zone(
        &package.manifest.write_zone,
        &text.lines().map(str::to_owned).collect::<Vec<_>>(),
    )
}

pub fn validate_all(package: &LoadedDevelopmentPackage, root: &Path) -> Result<ValidationReport> {
    ensure!(root.is_absolute(), "validation root must be absolute");
    if root.exists() {
        ensure!(
            fs::read_dir(root)?.next().is_none(),
            "validation root must be fresh and empty"
        );
    } else {
        fs::create_dir_all(root)?;
    }
    let roots = IsolationRoots {
        project: root.join("p"),
        home: root.join("h"),
        ymp_home: root.join("y"),
        tmp: root.join("t"),
        build: root.join("b"),
        export: root.join("e"),
    };
    for path in roots.paths() {
        fs::create_dir_all(path)?;
    }
    let mut prepared = BTreeMap::new();
    let mut seeded_source_failures = BTreeMap::new();
    let mut references = BTreeMap::new();
    for task in &package.manifest.tasks {
        let public_output = roots.project.join("public").join(&task.id);
        let preparation = prepare_task(package, &task.id, &public_output)?;
        prepared.insert(task.id.clone(), preparation.source_sha256);
        let source_report_path = roots.export.join(format!("source-{}.json", task.id));
        let source_report = verify_candidate(
            package,
            &task.id,
            &public_output,
            &roots,
            Some(&source_report_path),
        )?;
        ensure!(
            !source_report.passed,
            "seeded source unexpectedly passed: {}",
            task.id
        );
        seeded_source_failures.insert(
            task.id.clone(),
            source_report
                .failure_reason
                .context("seeded source failed without a reason")?,
        );
        let reference_output = roots.project.join("reference").join(&task.id);
        copy_tree(
            &package
                .root
                .join(&protected_task(package, &task.id)?.reference.path),
            &reference_output,
        )?;
        let report_path = roots.export.join(format!("reference-{}.json", task.id));
        let report = verify_candidate(
            package,
            &task.id,
            &reference_output,
            &roots,
            Some(&report_path),
        )?;
        ensure!(report.passed, "reference candidate failed: {}", task.id);
        ensure!(
            report.candidate_sha256 == task.reference_candidate_sha256,
            "reference digest was not reproduced: {}",
            task.id
        );
        references.insert(task.id.clone(), report.passed);
    }
    let mut observations = Vec::new();
    for mutation in &package.protected.mutations {
        let output = roots.project.join("mutations").join(&mutation.id);
        let materialized = materialize_mutation(package, &mutation.id, &output)?;
        let expected = expected_failure(package, mutation)?;
        let observed_reason = if mutation.kind == "candidate_file_replacement" {
            let report = verify_candidate(package, &mutation.task_id, &output, &roots, None)?;
            ensure!(
                !report.passed,
                "mutation unexpectedly passed: {}",
                mutation.id
            );
            report
                .failure_reason
                .context("failed mutation has no reason")?
        } else {
            match load_development_package(
                &output.join("manifest.json"),
                &output.join("manifest.sha256"),
            ) {
                Ok(_) => bail!("stratum relabel unexpectedly passed compliance"),
                Err(error) => {
                    let message = format!("{error:#}");
                    ensure!(
                        message.contains(&expected.reason),
                        "stratum relabel failed for the wrong reason: {message}"
                    );
                    expected.reason.clone()
                }
            }
        };
        ensure!(
            observed_reason == expected.reason,
            "mutation failed for the wrong reason: {}",
            mutation.id
        );
        observations.push(MutationObservation {
            mutation_id: mutation.id.clone(),
            mutation_sha256: materialized.mutation_sha256,
            expected_result_sha256: materialized.expected_result_sha256,
            observed_nonzero: true,
            observed_reason,
        });
    }
    let report = ValidationReport {
        schema_version: 1,
        corpus_id: CORPUS_ID.to_owned(),
        manifest_sha256: package.digest.clone(),
        model_calls: 0,
        roots: BTreeMap::from([
            ("p".to_owned(), roots.project.clone()),
            ("h".to_owned(), roots.home.clone()),
            ("y".to_owned(), roots.ymp_home.clone()),
            ("t".to_owned(), roots.tmp.clone()),
            ("b".to_owned(), roots.build.clone()),
            ("e".to_owned(), roots.export.clone()),
        ]),
        prepared_source_digests: prepared,
        seeded_source_failures,
        reference_results: references,
        mutations: observations,
        blind_spots: blind_spots(package),
    };
    write_json(&roots.export.join("validation.json"), &report)?;
    Ok(report)
}

fn validate_candidate_paths(
    package: &LoadedDevelopmentPackage,
    task_id: &str,
    candidate: &Path,
) -> Result<()> {
    ensure!(candidate.is_dir(), "candidate is not a directory");
    let base = package.root.join("public").join(task_id).join("project");
    let base_files = relative_files(&base)?;
    let candidate_files = relative_files(candidate)?;
    let all = base_files
        .union(&candidate_files)
        .cloned()
        .collect::<BTreeSet<_>>();
    for relative in all {
        if relative.starts_with("src/") {
            continue;
        }
        ensure!(
            base_files.contains(&relative) && candidate_files.contains(&relative),
            "candidate changed a protected path: {relative}"
        );
        ensure!(
            fs::read(base.join(&relative))? == fs::read(candidate.join(&relative))?,
            "candidate changed a protected path: {relative}"
        );
    }
    Ok(())
}

fn validate_isolation(
    roots: &IsolationRoots,
    candidate: &Path,
    report_path: Option<&Path>,
) -> Result<()> {
    let paths = roots.paths();
    for path in &paths {
        validate_absolute_normal(path, "isolation root")?;
        fs::create_dir_all(path)?;
        ensure!(
            !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "isolation root must not be a symbolic link: {}",
            path.display()
        );
    }
    let canonical = paths
        .iter()
        .map(fs::canonicalize)
        .collect::<std::io::Result<Vec<_>>>()?;
    for (index, left) in canonical.iter().enumerate() {
        for right in canonical.iter().skip(index + 1) {
            ensure!(
                left != right && !left.starts_with(right) && !right.starts_with(left),
                "p,h,y,t,b,e must be separate roots"
            );
        }
    }
    validate_absolute_normal(candidate, "candidate")?;
    ensure!(
        !fs::symlink_metadata(candidate)?.file_type().is_symlink(),
        "candidate root must not be a symbolic link"
    );
    ensure!(
        fs::canonicalize(candidate)?.starts_with(&canonical[0]),
        "candidate must be inside isolated project root"
    );
    if let Some(report) = report_path {
        validate_absolute_normal(report, "report")?;
        ensure!(
            report.starts_with(&roots.export),
            "report must be inside isolated export root"
        );
    }
    Ok(())
}

fn validate_absolute_normal(path: &Path, label: &str) -> Result<()> {
    ensure!(path.is_absolute(), "{label} path must be absolute");
    for component in path.components() {
        ensure!(
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            ),
            "{label} path must not contain traversal"
        );
    }
    Ok(())
}

impl IsolationRoots {
    fn paths(&self) -> [&Path; 6] {
        [
            &self.project,
            &self.home,
            &self.ymp_home,
            &self.tmp,
            &self.build,
            &self.export,
        ]
    }
}

fn run_cargo(
    current_dir: &Path,
    roots: &IsolationRoots,
    target: &Path,
    label: &str,
    args: &[&str],
) -> Result<CheckResult> {
    let cargo_home = roots.home.join("cargo");
    fs::create_dir_all(&cargo_home)?;
    let rustup_home = rustup_home()?;
    let mut command = Command::new("cargo");
    command
        .args(args)
        .current_dir(current_dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &roots.home)
        .env("YMP_HOME", &roots.ymp_home)
        .env("TMPDIR", &roots.tmp)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_NET_OFFLINE", "true")
        .env("RUST_BACKTRACE", "0");
    if let Some(path) = rustup_home {
        command.env("RUSTUP_HOME", path);
    }
    run_bounded(&mut command, label, &roots.tmp)
}

fn rustup_home() -> Result<Option<PathBuf>> {
    if let Some(path) = std::env::var_os("RUSTUP_HOME") {
        return Ok(Some(path.into()));
    }
    let output = Command::new("rustup")
        .args(["show", "home"])
        .output()
        .context("resolve pinned Rust toolchain home")?;
    if !output.status.success() {
        return Ok(None);
    }
    let path = String::from_utf8(output.stdout)
        .context("rustup home is not UTF-8")?
        .trim()
        .to_owned();
    Ok((!path.is_empty()).then(|| PathBuf::from(path)))
}

fn run_bounded(command: &mut Command, label: &str, tmp: &Path) -> Result<CheckResult> {
    let stdout_file = NamedTempFile::new_in(tmp)?;
    let stderr_file = NamedTempFile::new_in(tmp)?;
    command.stdout(Stdio::from(stdout_file.reopen()?));
    command.stderr(Stdio::from(stderr_file.reopen()?));
    let mut child = command.spawn().with_context(|| format!("run {label}"))?;
    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if started.elapsed() >= COMMAND_TIMEOUT {
            child
                .kill()
                .with_context(|| format!("kill timed-out {label}"))?;
            break (child.wait()?, true);
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ok(CheckResult {
        command: label.to_owned(),
        success: status.success() && !timed_out,
        status: exit_code(status),
        timed_out,
        stdout: read_tail(stdout_file.path())?,
        stderr: read_tail(stderr_file.path())?,
    })
}

fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code()
}

fn read_tail(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(OUTPUT_LIMIT);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

fn task<'a>(package: &'a LoadedDevelopmentPackage, id: &str) -> Result<&'a DevelopmentTask> {
    package
        .manifest
        .tasks
        .iter()
        .find(|task| task.id == id)
        .with_context(|| format!("unknown development task {id}"))
}

fn protected_task<'a>(
    package: &'a LoadedDevelopmentPackage,
    id: &str,
) -> Result<&'a ProtectedTask> {
    package
        .protected
        .task_bindings
        .iter()
        .find(|task| task.task_id == id)
        .with_context(|| format!("missing protected task {id}"))
}

fn mutation<'a>(package: &'a LoadedDevelopmentPackage, id: &str) -> Result<&'a Mutation> {
    package
        .protected
        .mutations
        .iter()
        .find(|mutation| mutation.id == id)
        .with_context(|| format!("unknown development mutation {id}"))
}

fn expected_failure(
    package: &LoadedDevelopmentPackage,
    mutation: &Mutation,
) -> Result<ExpectedFailure> {
    serde_json::from_slice(&fs::read(package.root.join(&mutation.expected.path))?)
        .with_context(|| format!("parse expected result for {}", mutation.id))
}

fn blind_spots(package: &LoadedDevelopmentPackage) -> Vec<BlindSpot> {
    package
        .protected
        .task_bindings
        .iter()
        .flat_map(|task| {
            task.oracles.iter().map(|oracle| BlindSpot {
                task_id: task.task_id.clone(),
                oracle_id: oracle.id.clone(),
                statement: oracle.blind_spot.clone(),
            })
        })
        .collect()
}

fn validate_write_zone_declaration(write_zone: &WriteZone) -> Result<()> {
    ensure!(
        write_zone.new_prefix == "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/",
        "development write-zone prefix changed"
    );
    ensure!(
        write_zone.new_files == ["ymp-rust/tools/ymp-corpus/src/development.rs"],
        "development write-zone new-file seam changed"
    );
    ensure!(
        write_zone.additive_seams
            == [
                "ymp-rust/tools/ymp-corpus/src/lib.rs",
                "ymp-rust/tools/ymp-corpus/src/main.rs",
                "ymp-rust/tools/ymp-corpus/Cargo.toml",
                "ymp-rust/Cargo.lock",
            ],
        "development additive seams changed"
    );
    Ok(())
}

fn verify_file_artifact(root: &Path, artifact: &ArtifactRef) -> Result<()> {
    validate_relative_path(&artifact.path)?;
    validate_sha256(&artifact.sha256, "artifact digest")?;
    ensure!(
        sha256_file(&root.join(&artifact.path))? == artifact.sha256,
        "artifact digest mismatch: {}",
        artifact.path.display()
    );
    Ok(())
}

fn verify_tree_artifact(root: &Path, artifact: &ArtifactRef) -> Result<()> {
    validate_relative_path(&artifact.path)?;
    validate_sha256(&artifact.sha256, "tree artifact digest")?;
    ensure!(
        tree_digest(&root.join(&artifact.path))? == artifact.sha256,
        "tree artifact digest mismatch: {}",
        artifact.path.display()
    );
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty() && !path.is_absolute(),
        "artifact path must be relative"
    );
    for component in path.components() {
        ensure!(
            matches!(component, Component::Normal(_)),
            "artifact path must not traverse"
        );
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
        "identifier is not stable lowercase ASCII: {value}"
    );
    Ok(())
}

fn validate_oracle_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| { byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' }),
        "oracle identifier is not stable lowercase ASCII: {value}"
    );
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not lowercase SHA-256"
    );
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("create directory {}", destination.display()))?;
    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            bail!("development corpus trees must not contain symbolic links");
        }
    }
    Ok(())
}

fn relative_files(root: &Path) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)?
                        .components()
                        .map(|component| component.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/"),
                );
            } else {
                bail!("candidate trees must not contain symbolic links");
            }
        }
    }
    Ok(files)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    fs::write(path, format!("{}\n", report_json(value)?))
        .with_context(|| format!("write report {}", path.display()))
}

fn verify_inventory_with_zone(zone: &WriteZone, lines: &[String]) -> Result<InventoryReport> {
    let mut paths = Vec::new();
    for line in lines.iter().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let (status, path) = match fields.as_slice() {
            [path] => ("", *path),
            [status, path] => (*status, *path),
            _ => bail!("invalid write-zone inventory line: {line}"),
        };
        let new_path =
            path.starts_with(&zone.new_prefix) || zone.new_files.iter().any(|item| item == path);
        let allowed = if new_path {
            status.is_empty() || status == "A" || status == "??"
        } else if zone.additive_seams.iter().any(|item| item == path) {
            status.is_empty() || status == "M"
        } else {
            false
        };
        ensure!(allowed, "WRITE_ZONE_VIOLATION: {status} {path}");
        paths.push(path.to_owned());
    }
    ensure!(!paths.is_empty(), "write-zone inventory is empty");
    Ok(InventoryReport {
        schema_version: 1,
        accepted: true,
        paths,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{InventoryReport, WriteZone, verify_inventory_with_zone};

    #[test]
    fn inventory_rejects_calibration_and_primary_paths() {
        let zone = WriteZone {
            new_prefix: "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/"
                .to_owned(),
            new_files: vec!["ymp-rust/tools/ymp-corpus/src/development.rs".to_owned()],
            additive_seams: vec![
                "ymp-rust/tools/ymp-corpus/src/lib.rs".to_owned(),
                "ymp-rust/tools/ymp-corpus/src/main.rs".to_owned(),
                "ymp-rust/tools/ymp-corpus/Cargo.toml".to_owned(),
                "ymp-rust/Cargo.lock".to_owned(),
            ],
        };
        for path in [
            "ymp-rust/tools/ymp-calibration/README.md",
            "ymp-rust/tools/ymp-corpus/corpus/study/manifest-v1.json",
            "ymp-rust/crates/ymp-runtime-api/src/lib.rs",
        ] {
            assert!(verify_inventory_with_zone(&zone, &[format!("M\t{path}")]).is_err());
        }
    }

    #[test]
    fn inventory_accepts_only_declared_additive_seam() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("paths");
        fs::write(
            &path,
            "A\tymp-rust/tools/ymp-corpus/src/development.rs\nM\tymp-rust/tools/ymp-corpus/src/lib.rs\n",
        )
        .unwrap();
        let report = verify_inventory_with_zone(
            &WriteZone {
                new_prefix: "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/"
                    .to_owned(),
                new_files: vec!["ymp-rust/tools/ymp-corpus/src/development.rs".to_owned()],
                additive_seams: vec!["ymp-rust/tools/ymp-corpus/src/lib.rs".to_owned()],
            },
            &fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(report.accepted);
    }

    fn _assert_report_is_public(_: InventoryReport) {}
}

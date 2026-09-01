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

use crate::development::{
    IsolationRoots as V1IsolationRoots, LoadedDevelopmentPackage, load_development_package,
    prepare_task as prepare_v1_task, verify_candidate as verify_v1_candidate,
};
use crate::{report_json, sha256_file, tree_digest};

const CORPUS_ID: &str = "weak-diagnostic-v2-l4plus-matrix";
const SEED_ROOT: &str = "ymp-weak-diagnostic-v2-l4plus-matrix";
const ACCEPTED_V1_MANIFEST_SHA256: &str =
    "085bdd3b915faf61545283ad1ec13e7e693398ebf28eded0460e51c122243696";
const ACCEPTED_V1_TREE_SHA256: &str =
    "0e5f0797557e431e3fb37f657d7608cbff0ac3a9b85fd056a9161f341ffca4ad";
const FROZEN_MANIFEST_SHA256: &str =
    "30e7d0d0f700b69cfabb34fae9250fa842e7692ee18288e97922bf4f263b8ab0";
const FROZEN_PROTECTED_MANIFEST_SHA256: &str =
    "9a703822d6cad48b2565909f88b17607f8c99a4273c2651f1baa8897121957c5";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const OUTPUT_LIMIT: usize = 16 * 1024;

#[derive(Debug, Subcommand)]
pub enum DevelopmentV2Command {
    Check {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        digest: PathBuf,
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

impl DevelopmentV2Command {
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Check { manifest, digest } => {
                let package = load_development_v2_package(manifest, digest)?;
                println!("{}", report_json(&compliance_report(&package))?);
            }
            Self::Validate {
                manifest,
                digest,
                root,
            } => {
                let package = load_development_v2_package(manifest, digest)?;
                println!("{}", report_json(&validate_all(&package, root)?)?);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentV2Manifest {
    pub schema_version: u32,
    pub corpus_id: String,
    pub artifact_role: String,
    pub freeze_state: String,
    pub model_calls_at_freeze: u64,
    pub primary_tasks_or_seeds_consumed: bool,
    pub seed_namespace_root: String,
    pub accepted_v1_manifest_sha256: String,
    pub accepted_v1_tree_sha256: String,
    pub protected_manifest_sha256: String,
    pub coverage: Vec<CoverageCell>,
    tasks: Vec<DevelopmentV2Task>,
    write_zone: WriteZone,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageCell {
    pub split: String,
    pub stratum: String,
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DevelopmentV2Task {
    id: String,
    origin: String,
    source_sha256: String,
    public_contract_sha256: String,
    protected_oracle_sha256: String,
    reference_candidate_sha256: String,
    stratum: String,
    split: String,
    seed_namespace: String,
    dependency_order: Vec<String>,
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
struct ProtectedManifest {
    schema_version: u32,
    corpus_id: String,
    task_bindings: Vec<ProtectedTask>,
    mutations: Vec<Mutation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProtectedTask {
    task_id: String,
    frozen_stratum: String,
    seeded_invalid_reason: String,
    oracle: ArtifactRef,
    reference: ArtifactRef,
    oracles: Vec<OracleSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OracleSpec {
    id: String,
    test: String,
    failure_reason: String,
    blind_spot: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRef {
    path: PathBuf,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    id: String,
    artifact: ArtifactRef,
    expected: ArtifactRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationInstruction {
    schema_version: u32,
    operation: String,
    task_id: String,
    split: Option<String>,
    stratum: Option<String>,
    from_task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFailure {
    schema_version: u32,
    mutation_id: String,
    command: String,
    expected_exit: String,
    reason: String,
    blind_spot: String,
}

#[derive(Debug)]
pub struct LoadedDevelopmentV2Package {
    root: PathBuf,
    v1_root: PathBuf,
    digest: String,
    manifest: DevelopmentV2Manifest,
    protected: ProtectedManifest,
    v1: LoadedDevelopmentPackage,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlindSpot {
    pub subject: String,
    pub statement: String,
}

#[derive(Debug, Serialize)]
pub struct DevelopmentV2ComplianceReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub manifest_sha256: String,
    pub accepted_v1_manifest_sha256: String,
    pub accepted_v1_tree_sha256: String,
    pub retained_v1_byte_identical: bool,
    pub model_calls: u64,
    pub coverage: Vec<CoverageCell>,
    pub fresh_cross_split_identities: bool,
    pub sibling_only_protected_oracles: bool,
    pub blind_spots: Vec<BlindSpot>,
}

#[derive(Debug, Serialize)]
struct CommandCheck {
    command: String,
    success: bool,
    status: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
struct VerificationSummary {
    task_id: String,
    candidate_sha256: String,
    passed: bool,
    failure_reason: Option<String>,
    model_calls: u64,
    protected_files_in_project: bool,
    sibling_verifier: bool,
    checks: Vec<CommandCheck>,
}

#[derive(Debug, Serialize)]
pub struct MutationObservation {
    pub mutation_id: String,
    pub mutation_sha256: String,
    pub expected_result_sha256: String,
    pub observed_nonzero: bool,
    pub observed_reason: String,
    pub model_calls: u64,
    pub blind_spot: String,
    pub manifest: PathBuf,
    pub digest: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct DevelopmentV2ValidationReport {
    pub schema_version: u32,
    pub corpus_id: String,
    pub manifest_sha256: String,
    pub accepted_v1_manifest_sha256: String,
    pub accepted_v1_tree_sha256: String,
    pub retained_v1_byte_identical: bool,
    pub model_calls: u64,
    pub roots: BTreeMap<String, PathBuf>,
    pub coverage: Vec<CoverageCell>,
    pub seeded_invalid_failures: BTreeMap<String, String>,
    pub seeded_invalid_blind_spots: BTreeMap<String, String>,
    pub reference_results: BTreeMap<String, bool>,
    pub mutations: Vec<MutationObservation>,
    pub protected_files_in_project: bool,
    pub sibling_only_protected_oracles: bool,
}

#[derive(Clone, Debug)]
struct IsolationRoots {
    project: PathBuf,
    home: PathBuf,
    ymp_home: PathBuf,
    tmp: PathBuf,
    build: PathBuf,
    export: PathBuf,
}

pub fn load_development_v2_package(
    manifest_path: &Path,
    digest_path: &Path,
) -> Result<LoadedDevelopmentV2Package> {
    let bytes = fs::read(manifest_path)
        .with_context(|| format!("read development-v2 manifest {}", manifest_path.display()))?;
    let supplied = fs::read_to_string(digest_path)
        .with_context(|| format!("read development-v2 digest {}", digest_path.display()))?;
    let digest = sha256_bytes(&bytes);
    validate_sha256(supplied.trim(), "development-v2 manifest sidecar")?;
    ensure!(
        supplied.trim() == digest,
        "development-v2 manifest digest mismatch"
    );
    let manifest: DevelopmentV2Manifest =
        serde_json::from_slice(&bytes).context("parse development-v2 manifest")?;
    let root = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let v1_root = root
        .parent()
        .context("development-v2 package has no development parent")?
        .join("weak-diagnostic-v1");
    let v1 = load_development_package(
        &v1_root.join("manifest.json"),
        &v1_root.join("manifest.sha256"),
    )
    .context("PARENT_V1_MUTATED: accepted v1 package failed its frozen validator")?;
    let protected_path = root.join("protected/manifest.json");
    let protected_bytes = fs::read(&protected_path)
        .with_context(|| format!("read protected-v2 manifest {}", protected_path.display()))?;
    let protected: ProtectedManifest =
        serde_json::from_slice(&protected_bytes).context("parse protected-v2 manifest")?;
    validate_semantics(&manifest, &protected, &root, &v1_root)?;
    ensure!(
        digest == FROZEN_MANIFEST_SHA256,
        "development-v2 manifest changed after freeze"
    );
    ensure!(
        manifest.protected_manifest_sha256 == FROZEN_PROTECTED_MANIFEST_SHA256
            && sha256_bytes(&protected_bytes) == FROZEN_PROTECTED_MANIFEST_SHA256,
        "protected-v2 manifest changed after freeze"
    );
    Ok(LoadedDevelopmentV2Package {
        root,
        v1_root,
        digest,
        manifest,
        protected,
        v1,
    })
}

pub fn compliance_report(package: &LoadedDevelopmentV2Package) -> DevelopmentV2ComplianceReport {
    let mut coverage = package.manifest.coverage.clone();
    coverage.sort();
    DevelopmentV2ComplianceReport {
        schema_version: 2,
        corpus_id: CORPUS_ID.to_owned(),
        manifest_sha256: package.digest.clone(),
        accepted_v1_manifest_sha256: ACCEPTED_V1_MANIFEST_SHA256.to_owned(),
        accepted_v1_tree_sha256: ACCEPTED_V1_TREE_SHA256.to_owned(),
        retained_v1_byte_identical: true,
        model_calls: 0,
        coverage,
        fresh_cross_split_identities: true,
        sibling_only_protected_oracles: true,
        blind_spots: blind_spots(package),
    }
}

fn validate_semantics(
    manifest: &DevelopmentV2Manifest,
    protected: &ProtectedManifest,
    root: &Path,
    v1_root: &Path,
) -> Result<()> {
    ensure!(
        manifest.schema_version == 2,
        "unsupported development-v2 schema"
    );
    ensure!(
        manifest.corpus_id == CORPUS_ID,
        "unexpected development-v2 corpus id"
    );
    ensure!(
        manifest.artifact_role == "development_only_held_out_corpus",
        "development-v2 corpus claims an unsupported role"
    );
    ensure!(
        manifest.freeze_state == "frozen_before_model_call" && manifest.model_calls_at_freeze == 0,
        "development-v2 corpus was not frozen before model use"
    );
    ensure!(
        !manifest.primary_tasks_or_seeds_consumed,
        "development-v2 corpus consumes a primary task or seed"
    );
    ensure!(
        manifest.seed_namespace_root == SEED_ROOT,
        "unexpected v2 seed namespace"
    );
    ensure!(
        manifest.accepted_v1_manifest_sha256 == ACCEPTED_V1_MANIFEST_SHA256,
        "PARENT_V1_DIGEST_CHANGED: accepted v1 manifest binding changed"
    );
    ensure!(
        manifest.accepted_v1_tree_sha256 == ACCEPTED_V1_TREE_SHA256,
        "PARENT_V1_MUTATED: accepted v1 tree binding changed"
    );
    ensure!(
        sha256_file(&v1_root.join("manifest.json"))? == ACCEPTED_V1_MANIFEST_SHA256,
        "PARENT_V1_DIGEST_CHANGED: accepted v1 manifest bytes changed"
    );
    ensure!(
        tree_digest(v1_root)? == ACCEPTED_V1_TREE_SHA256,
        "PARENT_V1_MUTATED: accepted v1 tree bytes changed"
    );
    validate_write_zone_declaration(&manifest.write_zone)?;
    validate_tasks(manifest, root, v1_root)?;
    validate_matrix(manifest)?;
    validate_freshness(manifest)?;
    validate_protected(manifest, protected, root)?;
    validate_mutations(protected, root)?;
    Ok(())
}

fn validate_tasks(manifest: &DevelopmentV2Manifest, root: &Path, v1_root: &Path) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut seeds = BTreeSet::new();
    for task in &manifest.tasks {
        validate_id(&task.id)?;
        ensure!(
            ids.insert(task.id.clone()),
            "duplicate development-v2 task id"
        );
        ensure!(
            matches!(task.origin.as_str(), "accepted_v1" | "v2"),
            "unknown task origin"
        );
        for (digest, label) in [
            (&task.source_sha256, "task source digest"),
            (&task.public_contract_sha256, "public contract digest"),
            (&task.protected_oracle_sha256, "protected oracle digest"),
            (
                &task.reference_candidate_sha256,
                "reference candidate digest",
            ),
        ] {
            validate_sha256(digest, label)?;
        }
        ensure!(
            seeds.insert(task.seed_namespace.clone()),
            "seed namespace collision"
        );
        ensure!(
            !task.seed_namespace.contains("calibration")
                && !task.seed_namespace.contains("primary")
                && !task.seed_namespace.contains("ymp-study-seed-v1"),
            "development-v2 seed namespace collides with calibration or primary use"
        );
    }
    ensure!(
        ids == BTreeSet::from([
            "l4-config-fusion".to_owned(),
            "l4-policy-union".to_owned(),
            "l4-sequential-checkpoint".to_owned(),
            "l4-sequential-replay".to_owned(),
        ]),
        "ACCEPTED_V1_CELL_REPLACEMENT: exact v2 task set changed"
    );
    let parent: Value = serde_json::from_slice(&fs::read(v1_root.join("manifest.json"))?)?;
    for id in ["l4-config-fusion", "l4-sequential-replay"] {
        let task = task(manifest, id)?;
        ensure!(
            task.origin == "accepted_v1",
            "ACCEPTED_V1_CELL_REPLACEMENT: {id}"
        );
        let parent_task = parent
            .pointer("/tasks")
            .and_then(Value::as_array)
            .and_then(|tasks| {
                tasks
                    .iter()
                    .find(|item| item.pointer("/id").and_then(Value::as_str) == Some(id))
            })
            .with_context(|| format!("accepted v1 task is missing: {id}"))?;
        for (field, actual) in task_identity_fields(task) {
            ensure!(
                parent_task
                    .pointer(&format!("/{field}"))
                    .and_then(Value::as_str)
                    == Some(actual),
                "ACCEPTED_V1_CELL_REPLACEMENT: {id}.{field}"
            );
        }
        ensure!(
            parent_task.pointer("/dependency_order")
                == Some(&serde_json::to_value(&task.dependency_order)?),
            "ACCEPTED_V1_CELL_REPLACEMENT: {id}.dependency_order"
        );
    }
    for id in ["l4-sequential-checkpoint", "l4-policy-union"] {
        let task = task(manifest, id)?;
        ensure!(
            task.origin == "v2",
            "new v2 task has the wrong origin: {id}"
        );
        ensure!(
            task.seed_namespace == format!("{SEED_ROOT}/{}/{}", task.split, task.id),
            "new task seed namespace is not derived from the v2 namespace"
        );
        let public = root.join("public").join(id);
        ensure!(
            tree_digest(&public)? == task.source_sha256,
            "task source digest mismatch: {id}"
        );
        ensure!(
            sha256_file(&public.join("TASK.md"))? == task.public_contract_sha256,
            "public contract digest mismatch: {id}"
        );
        ensure!(
            fs::read(public.join("TASK.md"))? == fs::read(public.join("project/TASK.md"))?,
            "prepared project contract differs from package contract: {id}"
        );
    }
    Ok(())
}

fn task_identity_fields(task: &DevelopmentV2Task) -> [(&str, &str); 8] {
    [
        ("id", &task.id),
        ("source_sha256", &task.source_sha256),
        ("public_contract_sha256", &task.public_contract_sha256),
        ("protected_oracle_sha256", &task.protected_oracle_sha256),
        (
            "reference_candidate_sha256",
            &task.reference_candidate_sha256,
        ),
        ("stratum", &task.stratum),
        ("split", &task.split),
        ("seed_namespace", &task.seed_namespace),
    ]
}

fn validate_matrix(manifest: &DevelopmentV2Manifest) -> Result<()> {
    let mut task_splits: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for cell in &manifest.coverage {
        task_splits
            .entry(&cell.task_id)
            .or_default()
            .insert(&cell.split);
    }
    ensure!(
        task_splits.values().all(|splits| splits.len() == 1),
        "SPLIT_ALIAS: one task identifier occurs in both splits"
    );
    for cell in &manifest.coverage {
        ensure!(
            matches!(cell.split.as_str(), "development" | "transfer")
                && matches!(
                    cell.stratum.as_str(),
                    "decomposable" | "strongly_sequential_expected_null"
                ),
            "UNKNOWN_MATRIX_AXIS: unknown stratum or split"
        );
    }
    let expected = BTreeSet::from([
        ("development", "decomposable"),
        ("development", "strongly_sequential_expected_null"),
        ("transfer", "decomposable"),
        ("transfer", "strongly_sequential_expected_null"),
    ]);
    let actual = manifest
        .coverage
        .iter()
        .map(|cell| (cell.split.as_str(), cell.stratum.as_str()))
        .collect::<BTreeSet<_>>();
    ensure!(
        manifest.coverage.len() == 4 && actual == expected,
        "MISSING_MATRIX_CELL: expected exactly one task in every stratum×split cell"
    );
    let mut covered = BTreeSet::new();
    for cell in &manifest.coverage {
        let task = task(manifest, &cell.task_id)?;
        ensure!(
            task.split == cell.split && task.stratum == cell.stratum,
            "STRATUM_RELABEL: task and coverage labels disagree"
        );
        ensure!(
            covered.insert(cell.task_id.as_str()),
            "duplicate coverage task"
        );
    }
    ensure!(
        covered.len() == manifest.tasks.len(),
        "matrix does not cover every task"
    );
    Ok(())
}

fn validate_freshness(manifest: &DevelopmentV2Manifest) -> Result<()> {
    for stratum in ["decomposable", "strongly_sequential_expected_null"] {
        let development = manifest
            .tasks
            .iter()
            .find(|task| task.stratum == stratum && task.split == "development")
            .context("MISSING_MATRIX_CELL: missing development task")?;
        let transfer = manifest
            .tasks
            .iter()
            .find(|task| task.stratum == stratum && task.split == "transfer")
            .context("MISSING_MATRIX_CELL: missing transfer task")?;
        for (label, left, right) in [
            ("task_id", development.id.as_str(), transfer.id.as_str()),
            (
                "source",
                development.source_sha256.as_str(),
                transfer.source_sha256.as_str(),
            ),
            (
                "public_contract",
                development.public_contract_sha256.as_str(),
                transfer.public_contract_sha256.as_str(),
            ),
            (
                "protected_oracle",
                development.protected_oracle_sha256.as_str(),
                transfer.protected_oracle_sha256.as_str(),
            ),
            (
                "reference_candidate",
                development.reference_candidate_sha256.as_str(),
                transfer.reference_candidate_sha256.as_str(),
            ),
            (
                "seed_namespace",
                development.seed_namespace.as_str(),
                transfer.seed_namespace.as_str(),
            ),
        ] {
            ensure!(left != right, "CROSS_SPLIT_ALIAS: {stratum}.{label}");
        }
    }
    Ok(())
}

fn validate_protected(
    manifest: &DevelopmentV2Manifest,
    protected: &ProtectedManifest,
    root: &Path,
) -> Result<()> {
    ensure!(
        protected.schema_version == 2 && protected.corpus_id == CORPUS_ID,
        "protected-v2 manifest identity mismatch"
    );
    let mut ids = BTreeSet::new();
    for binding in &protected.task_bindings {
        ensure!(
            ids.insert(binding.task_id.as_str()),
            "duplicate protected-v2 binding"
        );
        let task = task(manifest, &binding.task_id)?;
        ensure!(
            task.origin == "v2",
            "accepted v1 oracle must remain in the v1 package"
        );
        ensure!(
            binding.frozen_stratum == task.stratum,
            "STRATUM_RELABEL: task {} is frozen as {}",
            task.id,
            binding.frozen_stratum
        );
        let expected_oracle = format!("protected/oracles/{}.rs", task.id);
        let expected_reference = format!("protected/reference/{}", task.id);
        ensure!(
            binding.oracle.path == Path::new(&expected_oracle)
                && binding.oracle.sha256 == task.protected_oracle_sha256
                && binding.reference.path == Path::new(&expected_reference)
                && binding.reference.sha256 == task.reference_candidate_sha256,
            "PROTECTED_ORACLE_SUBSTITUTION: task {} binding changed",
            task.id
        );
        verify_file_artifact(root, &binding.oracle)?;
        verify_tree_artifact(root, &binding.reference)?;
        ensure!(
            !binding.seeded_invalid_reason.is_empty()
                && binding
                    .oracles
                    .iter()
                    .any(|oracle| oracle.failure_reason == binding.seeded_invalid_reason),
            "seeded invalid reason has no protected oracle"
        );
        let oracle_ids = binding
            .oracles
            .iter()
            .map(|oracle| oracle.id.as_str())
            .collect::<BTreeSet<_>>();
        let expected_ids = if task.stratum == "decomposable" {
            BTreeSet::from(["branch_a", "branch_b", "integration"])
        } else {
            BTreeSet::from([
                "command_conflict",
                "ordered_transition",
                "rejected_identity",
            ])
        };
        ensure!(
            oracle_ids == expected_ids,
            "protected oracle set changed: {}",
            task.id
        );
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
        ids == BTreeSet::from(["l4-policy-union", "l4-sequential-checkpoint"]),
        "protected-v2 bindings do not match new tasks"
    );
    Ok(())
}

fn validate_mutations(protected: &ProtectedManifest, root: &Path) -> Result<()> {
    let expected = BTreeMap::from([
        ("missing-cell", "MISSING_MATRIX_CELL"),
        ("split-alias", "SPLIT_ALIAS"),
        ("stratum-relabel", "STRATUM_RELABEL"),
        (
            "protected-oracle-substitution",
            "PROTECTED_ORACLE_SUBSTITUTION",
        ),
    ]);
    let actual = protected
        .mutations
        .iter()
        .map(|mutation| mutation.id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        actual == expected.keys().copied().collect(),
        "protected mutation set changed"
    );
    let mut mutation_digests = BTreeSet::new();
    let mut expected_digests = BTreeSet::new();
    for mutation in &protected.mutations {
        verify_file_artifact(root, &mutation.artifact)?;
        verify_file_artifact(root, &mutation.expected)?;
        ensure!(
            mutation_digests.insert(mutation.artifact.sha256.clone())
                && expected_digests.insert(mutation.expected.sha256.clone()),
            "mutation artifacts must have distinct immutable digests"
        );
        let instruction: MutationInstruction =
            serde_json::from_slice(&fs::read(root.join(&mutation.artifact.path))?)?;
        let result = expected_failure(root, mutation)?;
        ensure!(
            instruction.schema_version == 1
                && result.schema_version == 1
                && result.mutation_id == mutation.id
                && result.command == "development-v2 check"
                && result.expected_exit == "nonzero"
                && result.reason == expected[mutation.id.as_str()]
                && !result.blind_spot.is_empty(),
            "mutation contract changed: {}",
            mutation.id
        );
    }
    Ok(())
}

pub fn validate_all(
    package: &LoadedDevelopmentV2Package,
    root: &Path,
) -> Result<DevelopmentV2ValidationReport> {
    validate_absolute_normal(root, "validation root")?;
    if root.exists() {
        ensure!(
            fs::read_dir(root)?.next().is_none(),
            "validation root must be fresh and empty"
        );
    } else {
        fs::create_dir_all(root)?;
    }
    let roots = IsolationRoots {
        project: root.join("project"),
        home: root.join("home"),
        ymp_home: root.join("ymp-home"),
        tmp: root.join("tmp"),
        build: root.join("build"),
        export: root.join("export"),
    };
    validate_isolation_roots(&roots)?;
    let v1_roots = V1IsolationRoots {
        project: roots.project.clone(),
        home: roots.home.clone(),
        ymp_home: roots.ymp_home.clone(),
        tmp: roots.tmp.clone(),
        build: roots.build.clone(),
        export: roots.export.clone(),
    };
    let mut seeded_failures = BTreeMap::new();
    let mut seeded_blind_spots = BTreeMap::new();
    let mut references = BTreeMap::new();
    for task in &package.manifest.tasks {
        let source = roots.project.join("seeded-invalid").join(&task.id);
        let reference = roots.project.join("reference").join(&task.id);
        if task.origin == "accepted_v1" {
            prepare_v1_task(&package.v1, &task.id, &source)?;
            copy_tree(
                &package.v1_root.join("protected/reference").join(&task.id),
                &reference,
            )?;
            let source_report =
                verify_v1_candidate(&package.v1, &task.id, &source, &v1_roots, None)?;
            ensure!(
                !source_report.passed,
                "seeded v1 source unexpectedly passed: {}",
                task.id
            );
            seeded_failures.insert(
                task.id.clone(),
                source_report
                    .failure_reason
                    .context("seeded v1 source failed without a reason")?,
            );
            let reference_report =
                verify_v1_candidate(&package.v1, &task.id, &reference, &v1_roots, None)?;
            ensure!(
                reference_report.passed,
                "accepted v1 reference failed: {}",
                task.id
            );
            ensure!(
                reference_report.candidate_sha256 == task.reference_candidate_sha256,
                "accepted v1 reference digest changed: {}",
                task.id
            );
            references.insert(task.id.clone(), true);
        } else {
            copy_tree(
                &package.root.join("public").join(&task.id).join("project"),
                &source,
            )?;
            copy_tree(
                &package.root.join("protected/reference").join(&task.id),
                &reference,
            )?;
            let binding = protected_task(package, &task.id)?;
            let source_report = verify_new_candidate(package, task, binding, &source, &roots)?;
            ensure!(
                !source_report.passed,
                "seeded v2 source unexpectedly passed: {}",
                task.id
            );
            let reason = source_report
                .failure_reason
                .context("seeded v2 source failed without a reason")?;
            ensure!(
                reason == binding.seeded_invalid_reason,
                "seeded invalid failed for the wrong reason: {}",
                task.id
            );
            let blind_spot = binding
                .oracles
                .iter()
                .find(|oracle| oracle.failure_reason == reason)
                .map(|oracle| oracle.blind_spot.clone())
                .context("seeded invalid failure has no recorded blind spot")?;
            seeded_failures.insert(task.id.clone(), reason);
            seeded_blind_spots.insert(task.id.clone(), blind_spot);
            let reference_report =
                verify_new_candidate(package, task, binding, &reference, &roots)?;
            ensure!(
                reference_report.passed,
                "new v2 reference failed: {}",
                task.id
            );
            ensure!(
                reference_report.candidate_sha256 == task.reference_candidate_sha256,
                "new v2 reference digest changed: {}",
                task.id
            );
            references.insert(task.id.clone(), true);
        }
    }
    ensure!(
        seeded_blind_spots.len() == 2
            && seeded_failures["l4-sequential-checkpoint"] != seeded_failures["l4-policy-union"],
        "new seeded invalid candidates need distinct reasons and blind spots"
    );
    let mutations = package
        .protected
        .mutations
        .iter()
        .map(|mutation| materialize_and_observe_mutation(package, mutation, &roots.export))
        .collect::<Result<Vec<_>>>()?;
    let protected_files_in_project = project_contains_protected_bytes(package, &roots.project)?;
    ensure!(
        !protected_files_in_project,
        "protected oracle bytes entered a candidate project"
    );
    let mut coverage = package.manifest.coverage.clone();
    coverage.sort();
    let report = DevelopmentV2ValidationReport {
        schema_version: 2,
        corpus_id: CORPUS_ID.to_owned(),
        manifest_sha256: package.digest.clone(),
        accepted_v1_manifest_sha256: ACCEPTED_V1_MANIFEST_SHA256.to_owned(),
        accepted_v1_tree_sha256: ACCEPTED_V1_TREE_SHA256.to_owned(),
        retained_v1_byte_identical: true,
        model_calls: 0,
        roots: BTreeMap::from([
            ("project".to_owned(), roots.project.clone()),
            ("home".to_owned(), roots.home.clone()),
            ("ymp_home".to_owned(), roots.ymp_home.clone()),
            ("tmp".to_owned(), roots.tmp.clone()),
            ("build".to_owned(), roots.build.clone()),
            ("export".to_owned(), roots.export.clone()),
        ]),
        coverage,
        seeded_invalid_failures: seeded_failures,
        seeded_invalid_blind_spots: seeded_blind_spots,
        reference_results: references,
        mutations,
        protected_files_in_project,
        sibling_only_protected_oracles: true,
    };
    write_json(
        &roots.export.join("development-v2-validation.json"),
        &report,
    )?;
    Ok(report)
}

fn verify_new_candidate(
    package: &LoadedDevelopmentV2Package,
    task: &DevelopmentV2Task,
    binding: &ProtectedTask,
    candidate: &Path,
    roots: &IsolationRoots,
) -> Result<VerificationSummary> {
    validate_candidate_paths(package, task, candidate)?;
    let digest = tree_digest(candidate)?;
    let private = Builder::new()
        .prefix("ymp-v2-protected-")
        .tempdir_in(&roots.tmp)
        .context("create sibling protected verifier root")?;
    ensure!(
        !private.path().starts_with(&roots.project),
        "protected verifier must be a sibling of candidate projects"
    );
    copy_tree(candidate, private.path())?;
    let oracle_target = private.path().join("tests/ymp_protected.rs");
    fs::create_dir_all(oracle_target.parent().expect("oracle target has parent"))?;
    fs::copy(package.root.join(&binding.oracle.path), &oracle_target)?;
    let target = roots.build.join(&task.id).join(&digest);
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
    Ok(VerificationSummary {
        task_id: task.id.clone(),
        candidate_sha256: digest,
        passed: failure_reason.is_none(),
        failure_reason,
        model_calls: 0,
        protected_files_in_project: false,
        sibling_verifier: true,
        checks,
    })
}

fn materialize_and_observe_mutation(
    package: &LoadedDevelopmentV2Package,
    mutation: &Mutation,
    project_root: &Path,
) -> Result<MutationObservation> {
    let destination = project_root.join("mutations").join(&mutation.id);
    let v1_destination = destination.join("weak-diagnostic-v1");
    let v2_destination = destination.join("weak-diagnostic-v2");
    ensure!(!destination.exists(), "mutation output already exists");
    copy_tree(&package.v1_root, &v1_destination)?;
    copy_tree(&package.root, &v2_destination)?;
    let instruction: MutationInstruction =
        serde_json::from_slice(&fs::read(package.root.join(&mutation.artifact.path))?)?;
    let expected = expected_failure(&package.root, mutation)?;
    let manifest_path = v2_destination.join("manifest.json");
    let protected_path = v2_destination.join("protected/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    match instruction.operation.as_str() {
        "remove_cell" => {
            let cells = manifest
                .pointer_mut("/coverage")
                .and_then(Value::as_array_mut)
                .context("coverage is missing")?;
            cells.retain(|cell| {
                cell.pointer("/task_id").and_then(Value::as_str)
                    != Some(instruction.task_id.as_str())
            });
        }
        "alias_split" => {
            manifest
                .pointer_mut("/coverage")
                .and_then(Value::as_array_mut)
                .context("coverage is missing")?
                .push(serde_json::json!({
                    "split": instruction.split.context("split alias has no split")?,
                    "stratum": "decomposable",
                    "task_id": instruction.task_id,
                }));
        }
        "relabel_stratum" => {
            manifest_task_mut(&mut manifest, &instruction.task_id)?
                .as_object_mut()
                .context("task is not an object")?
                .insert(
                    "stratum".to_owned(),
                    Value::String(instruction.stratum.context("relabel has no stratum")?),
                );
        }
        "substitute_oracle" => {
            let from = instruction
                .from_task_id
                .as_deref()
                .context("oracle substitution has no source task")?;
            let mut protected: Value = serde_json::from_slice(&fs::read(&protected_path)?)?;
            let replacement = protected_binding(&protected, from)?
                .pointer("/oracle")
                .cloned()
                .context("replacement oracle is missing")?;
            protected_binding_mut(&mut protected, &instruction.task_id)?
                .as_object_mut()
                .context("protected binding is not an object")?
                .insert("oracle".to_owned(), replacement);
            let protected_bytes = pretty_json(&protected)?;
            fs::write(&protected_path, &protected_bytes)?;
            manifest
                .as_object_mut()
                .context("manifest is not an object")?
                .insert(
                    "protected_manifest_sha256".to_owned(),
                    Value::String(sha256_bytes(&protected_bytes)),
                );
        }
        other => bail!("unknown mutation operation: {other}"),
    }
    let manifest_bytes = pretty_json(&manifest)?;
    fs::write(&manifest_path, &manifest_bytes)?;
    fs::write(
        v2_destination.join("manifest.sha256"),
        format!("{}\n", sha256_bytes(&manifest_bytes)),
    )?;
    let error =
        load_development_v2_package(&manifest_path, &v2_destination.join("manifest.sha256"))
            .expect_err("mutation unexpectedly passed development-v2 check");
    let message = format!("{error:#}");
    ensure!(
        message.contains(&expected.reason),
        "mutation {} failed for the wrong reason: {message}",
        mutation.id
    );
    Ok(MutationObservation {
        mutation_id: mutation.id.clone(),
        mutation_sha256: mutation.artifact.sha256.clone(),
        expected_result_sha256: mutation.expected.sha256.clone(),
        observed_nonzero: true,
        observed_reason: expected.reason,
        model_calls: 0,
        blind_spot: expected.blind_spot,
        manifest: manifest_path,
        digest: v2_destination.join("manifest.sha256"),
    })
}

fn manifest_task_mut<'a>(manifest: &'a mut Value, id: &str) -> Result<&'a mut Value> {
    manifest
        .pointer_mut("/tasks")
        .and_then(Value::as_array_mut)
        .and_then(|tasks| {
            tasks
                .iter_mut()
                .find(|task| task.pointer("/id").and_then(Value::as_str) == Some(id))
        })
        .with_context(|| format!("mutation task is missing: {id}"))
}

fn protected_binding<'a>(protected: &'a Value, id: &str) -> Result<&'a Value> {
    protected
        .pointer("/task_bindings")
        .and_then(Value::as_array)
        .and_then(|bindings| {
            bindings
                .iter()
                .find(|item| item.pointer("/task_id").and_then(Value::as_str) == Some(id))
        })
        .with_context(|| format!("protected mutation task is missing: {id}"))
}

fn protected_binding_mut<'a>(protected: &'a mut Value, id: &str) -> Result<&'a mut Value> {
    protected
        .pointer_mut("/task_bindings")
        .and_then(Value::as_array_mut)
        .and_then(|bindings| {
            bindings
                .iter_mut()
                .find(|item| item.pointer("/task_id").and_then(Value::as_str) == Some(id))
        })
        .with_context(|| format!("protected mutation task is missing: {id}"))
}

fn validate_candidate_paths(
    package: &LoadedDevelopmentV2Package,
    task: &DevelopmentV2Task,
    candidate: &Path,
) -> Result<()> {
    ensure!(candidate.is_dir(), "candidate is not a directory");
    ensure!(
        fs::canonicalize(candidate)?.starts_with(fs::canonicalize(candidate_root(candidate)?)?),
        "candidate must be inside isolated project root"
    );
    let base = package.root.join("public").join(&task.id).join("project");
    let base_files = relative_files(&base)?;
    let candidate_files = relative_files(candidate)?;
    for relative in base_files.union(&candidate_files) {
        if relative.starts_with("src/") {
            continue;
        }
        ensure!(
            base_files.contains(relative)
                && candidate_files.contains(relative)
                && fs::read(base.join(relative))? == fs::read(candidate.join(relative))?,
            "candidate changed a protected path: {relative}"
        );
    }
    Ok(())
}

fn candidate_root(candidate: &Path) -> Result<&Path> {
    candidate
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "project"))
        .context("candidate is not below isolated project root")
}

fn validate_isolation_roots(roots: &IsolationRoots) -> Result<()> {
    for path in roots.paths() {
        validate_absolute_normal(path, "isolation root")?;
        fs::create_dir_all(path)?;
        ensure!(
            !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "isolation root must not be a symbolic link: {}",
            path.display()
        );
    }
    let canonical = roots
        .paths()
        .iter()
        .map(fs::canonicalize)
        .collect::<std::io::Result<Vec<_>>>()?;
    for (index, left) in canonical.iter().enumerate() {
        for right in canonical.iter().skip(index + 1) {
            ensure!(
                left != right && !left.starts_with(right) && !right.starts_with(left),
                "project, HOME, YMP_HOME, TMPDIR, build and export must be separate roots"
            );
        }
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
) -> Result<CommandCheck> {
    let cargo_home = roots.home.join("cargo");
    fs::create_dir_all(&cargo_home)?;
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
    if let Some(path) = rustup_home()? {
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

fn run_bounded(command: &mut Command, label: &str, tmp: &Path) -> Result<CommandCheck> {
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
    Ok(CommandCheck {
        command: label.to_owned(),
        success: status.success() && !timed_out,
        status: exit_code(status),
        timed_out,
        stdout: read_tail(stdout_file.path())?,
        stderr: read_tail(stderr_file.path())?,
    })
}

fn project_contains_protected_bytes(
    package: &LoadedDevelopmentV2Package,
    project: &Path,
) -> Result<bool> {
    let protected_digests = package
        .manifest
        .tasks
        .iter()
        .map(|task| task.protected_oracle_sha256.as_str())
        .collect::<BTreeSet<_>>();
    for relative in relative_files(project)? {
        if relative.ends_with("ymp_protected.rs")
            || protected_digests.contains(sha256_file(&project.join(relative))?.as_str())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn task<'a>(manifest: &'a DevelopmentV2Manifest, id: &str) -> Result<&'a DevelopmentV2Task> {
    manifest
        .tasks
        .iter()
        .find(|task| task.id == id)
        .with_context(|| format!("unknown development-v2 task {id}"))
}

fn protected_task<'a>(
    package: &'a LoadedDevelopmentV2Package,
    id: &str,
) -> Result<&'a ProtectedTask> {
    package
        .protected
        .task_bindings
        .iter()
        .find(|task| task.task_id == id)
        .with_context(|| format!("missing protected-v2 task {id}"))
}

fn expected_failure(root: &Path, mutation: &Mutation) -> Result<ExpectedFailure> {
    serde_json::from_slice(&fs::read(root.join(&mutation.expected.path))?)
        .with_context(|| format!("parse expected result for {}", mutation.id))
}

fn blind_spots(package: &LoadedDevelopmentV2Package) -> Vec<BlindSpot> {
    let mut spots = package
        .protected
        .task_bindings
        .iter()
        .flat_map(|task| {
            task.oracles.iter().map(|oracle| BlindSpot {
                subject: format!("{}.{}", task.task_id, oracle.id),
                statement: oracle.blind_spot.clone(),
            })
        })
        .collect::<Vec<_>>();
    spots.extend(package.protected.mutations.iter().filter_map(|mutation| {
        expected_failure(&package.root, mutation)
            .ok()
            .map(|expected| BlindSpot {
                subject: mutation.id.clone(),
                statement: expected.blind_spot,
            })
    }));
    spots
}

fn validate_write_zone_declaration(zone: &WriteZone) -> Result<()> {
    ensure!(
        zone.new_prefix == "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v2/",
        "development-v2 write-zone prefix changed"
    );
    ensure!(
        zone.new_files == ["ymp-rust/tools/ymp-corpus/src/development_v2.rs"],
        "development-v2 new-file seam changed"
    );
    ensure!(
        zone.additive_seams
            == [
                "ymp-rust/tools/ymp-corpus/src/lib.rs",
                "ymp-rust/tools/ymp-corpus/src/main.rs",
                "ymp-rust/tools/ymp-corpus/Cargo.toml",
                "ymp-rust/Cargo.lock",
            ],
        "development-v2 additive seams changed"
    );
    Ok(())
}

pub fn validate_write_zone_inventory(lines: &[String]) -> Result<()> {
    let prefix = "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v2/";
    let new_files = ["ymp-rust/tools/ymp-corpus/src/development_v2.rs"];
    let seams = [
        "ymp-rust/tools/ymp-corpus/src/lib.rs",
        "ymp-rust/tools/ymp-corpus/src/main.rs",
        "ymp-rust/tools/ymp-corpus/Cargo.toml",
        "ymp-rust/Cargo.lock",
    ];
    for line in lines.iter().filter(|line| !line.trim().is_empty()) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let (status, path) = match fields.as_slice() {
            [path] => ("", *path),
            [status, path] => (*status, *path),
            _ => bail!("invalid write-zone inventory line: {line}"),
        };
        let allowed = if path.starts_with(prefix) || new_files.contains(&path) {
            matches!(status, "" | "A" | "??")
        } else if seams.contains(&path) {
            matches!(status, "" | "M")
        } else {
            false
        };
        ensure!(allowed, "WRITE_ZONE_VIOLATION: {status} {path}");
    }
    ensure!(
        lines.iter().any(|line| !line.trim().is_empty()),
        "write-zone inventory is empty"
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

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            bail!("development-v2 trees must not contain symbolic links");
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
                bail!("development-v2 trees must not contain symbolic links");
            }
        }
    }
    Ok(files)
}

fn pretty_json(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
    fs::write(path, format!("{}\n", report_json(value)?))?;
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code()
}

fn read_tail(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let start = bytes.len().saturating_sub(OUTPUT_LIMIT);
    Ok(String::from_utf8_lossy(&bytes[start..]).into_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        DevelopmentV2Manifest, copy_tree, load_development_v2_package, pretty_json, sha256_bytes,
        validate_freshness, validate_matrix, validate_write_zone_inventory,
    };

    fn development_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/development")
    }

    fn manifest() -> DevelopmentV2Manifest {
        serde_json::from_slice(
            &fs::read(development_root().join("weak-diagnostic-v2/manifest.json")).unwrap(),
        )
        .unwrap()
    }

    fn copied_packages() -> (tempfile::TempDir, PathBuf) {
        let directory = tempdir().unwrap();
        for name in ["weak-diagnostic-v1", "weak-diagnostic-v2"] {
            copy_tree(&development_root().join(name), &directory.path().join(name)).unwrap();
        }
        let v2 = directory.path().join("weak-diagnostic-v2");
        (directory, v2)
    }

    fn rewrite_manifest(v2: &Path, change: impl FnOnce(&mut Value)) {
        let path = v2.join("manifest.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        change(&mut value);
        let bytes = pretty_json(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::write(
            v2.join("manifest.sha256"),
            format!("{}\n", sha256_bytes(&bytes)),
        )
        .unwrap();
    }

    #[test]
    fn frozen_package_and_parent_load_together() {
        let v2 = development_root().join("weak-diagnostic-v2");
        load_development_v2_package(&v2.join("manifest.json"), &v2.join("manifest.sha256"))
            .unwrap();
    }

    #[test]
    fn any_parent_v1_write_is_rejected() {
        let (_directory, v2) = copied_packages();
        let parent_contract = v2
            .parent()
            .unwrap()
            .join("weak-diagnostic-v1/public/l4-config-fusion/TASK.md");
        let mut bytes = fs::read(&parent_contract).unwrap();
        bytes.extend_from_slice(b"\nmutation\n");
        fs::write(parent_contract, bytes).unwrap();
        let error =
            load_development_v2_package(&v2.join("manifest.json"), &v2.join("manifest.sha256"))
                .unwrap_err();
        assert!(format!("{error:#}").contains("PARENT_V1_MUTATED"));
    }

    #[test]
    fn changed_parent_digest_and_accepted_cell_are_rejected() {
        let (_directory, v2) = copied_packages();
        rewrite_manifest(&v2, |value| {
            value["accepted_v1_manifest_sha256"] = Value::String("0".repeat(64));
        });
        let error =
            load_development_v2_package(&v2.join("manifest.json"), &v2.join("manifest.sha256"))
                .unwrap_err();
        assert!(format!("{error:#}").contains("PARENT_V1_DIGEST_CHANGED"));

        let (_directory, v2) = copied_packages();
        rewrite_manifest(&v2, |value| {
            let task = value["tasks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|task| task["id"] == "l4-config-fusion")
                .unwrap();
            task["source_sha256"] = Value::String("0".repeat(64));
        });
        let error =
            load_development_v2_package(&v2.join("manifest.json"), &v2.join("manifest.sha256"))
                .unwrap_err();
        assert!(format!("{error:#}").contains("ACCEPTED_V1_CELL_REPLACEMENT"));
    }

    #[test]
    fn duplicate_and_unknown_matrix_cells_are_rejected() {
        let mut duplicate = manifest();
        duplicate.coverage.push(duplicate.coverage[0].clone());
        assert!(validate_matrix(&duplicate).is_err());

        let mut unknown = manifest();
        unknown.coverage[0].split = "holdout".to_owned();
        let error = validate_matrix(&unknown).unwrap_err();
        assert!(format!("{error:#}").contains("UNKNOWN_MATRIX_AXIS"));
    }

    #[test]
    fn every_required_cross_split_identity_must_be_fresh() {
        for field in [
            "id",
            "source",
            "public_contract",
            "protected_oracle",
            "reference_candidate",
            "seed_namespace",
        ] {
            let mut value = manifest();
            let development = value
                .tasks
                .iter()
                .find(|task| task.stratum == "decomposable" && task.split == "development")
                .unwrap()
                .clone();
            let transfer = value
                .tasks
                .iter_mut()
                .find(|task| task.stratum == "decomposable" && task.split == "transfer")
                .unwrap();
            match field {
                "id" => transfer.id = development.id,
                "source" => transfer.source_sha256 = development.source_sha256,
                "public_contract" => {
                    transfer.public_contract_sha256 = development.public_contract_sha256;
                }
                "protected_oracle" => {
                    transfer.protected_oracle_sha256 = development.protected_oracle_sha256;
                }
                "reference_candidate" => {
                    transfer.reference_candidate_sha256 = development.reference_candidate_sha256;
                }
                "seed_namespace" => transfer.seed_namespace = development.seed_namespace,
                _ => unreachable!(),
            }
            let error = validate_freshness(&value).unwrap_err();
            assert!(format!("{error:#}").contains("CROSS_SPLIT_ALIAS"));
        }
    }

    #[test]
    fn inventory_rejects_every_named_no_touch_surface() {
        for path in [
            "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v1/manifest.json",
            "ymp-rust/tools/ymp-corpus/src/development.rs",
            "ymp-rust/tools/ymp-calibration/src/main.rs",
            "ymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-admission-v1/manifest.json",
            "ymp-rust/tools/ymp-corpus/corpus/tasks/example/manifest.json",
            "ymp-rust/tools/ymp-corpus/corpus/study/manifest-v1.json",
            "ymp-rust/tools/ymp-corpus/corpus/policies/budget.json",
            "ymp-rust/tools/ymp-corpus/corpus/registry.json",
            "ymp-rust/crates/ymp-runtime/src/lib.rs",
            "ymp-rust/crates/ymp-application/src/lib.rs",
            "ymp-rust/crates/ymp-agent-api/src/lib.rs",
            "ymp-docs/research/example.md",
            "ymp-docs/work/STATUS.md",
        ] {
            assert!(validate_write_zone_inventory(&[format!("M\t{path}")]).is_err());
        }
    }

    #[test]
    fn inventory_accepts_only_the_additive_v2_surface() {
        assert!(
            validate_write_zone_inventory(&[
                "A\tymp-rust/tools/ymp-corpus/src/development_v2.rs".to_owned(),
                "M\tymp-rust/tools/ymp-corpus/src/lib.rs".to_owned(),
                "M\tymp-rust/tools/ymp-corpus/src/main.rs".to_owned(),
                "??\tymp-rust/tools/ymp-corpus/corpus/development/weak-diagnostic-v2/manifest.json"
                    .to_owned(),
            ])
            .is_ok()
        );
    }
}

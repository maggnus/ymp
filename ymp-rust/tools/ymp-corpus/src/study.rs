#![allow(clippy::too_many_lines)]

#[path = "study_freeze.rs"]
mod study_freeze;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Classification, LoadedCorpus, sha256_bytes};
use study_freeze::FROZEN_STUDY_MANIFEST_SHA256;

const APPROVED_INITIAL_CORPUS_ROOT: &str =
    "e4e886bf1dfd433342f3f10f34c086415462908d60f5a53d1448824c9fe3bfc7";
const Z_REPORTING_95: f64 = 1.959_963_984_540_054;
const DECISION_ALPHA: f64 = 0.025;
const CLUSTER_DIFFERENCE_LOWER_BOUND: f64 = -1.0;
const CLUSTER_DIFFERENCE_UPPER_BOUND: f64 = 1.0;

#[derive(Debug)]
pub struct FrozenStudyManifest {
    value: Value,
    digest: String,
    directory: PathBuf,
    power_inputs: PowerInputs,
}

#[derive(Debug, Serialize)]
pub struct ManifestVerificationReport<'a> {
    schema_version: u32,
    study_id: &'a str,
    manifest_sha256: &'a str,
    corpus_root_sha256: &'a str,
    authorization_scope: &'static str,
    owner_authorization_evaluated: bool,
    execution_ready: bool,
}

impl FrozenStudyManifest {
    pub fn verification_report(&self) -> ManifestVerificationReport<'_> {
        ManifestVerificationReport {
            schema_version: 1,
            study_id: self.string("/study_id").expect("validated study id"),
            manifest_sha256: &self.digest,
            corpus_root_sha256: APPROVED_INITIAL_CORPUS_ROOT,
            authorization_scope: "technical_verification_only",
            owner_authorization_evaluated: false,
            execution_ready: false,
        }
    }

    fn string(&self, pointer: &str) -> Result<&str> {
        self.value
            .pointer(pointer)
            .and_then(Value::as_str)
            .with_context(|| format!("manifest field {pointer} is not a string"))
    }

    fn u64(&self, pointer: &str) -> Result<u64> {
        self.value
            .pointer(pointer)
            .and_then(Value::as_u64)
            .with_context(|| format!("manifest field {pointer} is not an unsigned integer"))
    }

    fn bool(&self, pointer: &str) -> Result<bool> {
        self.value
            .pointer(pointer)
            .and_then(Value::as_bool)
            .with_context(|| format!("manifest field {pointer} is not a boolean"))
    }

    fn f64(&self, pointer: &str) -> Result<f64> {
        self.value
            .pointer(pointer)
            .and_then(Value::as_f64)
            .with_context(|| format!("manifest field {pointer} is not a number"))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PowerInputs {
    schema_version: u32,
    familywise_alpha: f64,
    claimable_strata: u32,
    target_joint_power: f64,
    reliability_required_contrasts_for_broad_claim: u32,
    communication_required_contrasts_for_broad_claim: u32,
    paired_cluster_difference_lower_bound: f64,
    paired_cluster_difference_upper_bound: f64,
    minimum_effect_numerator: u32,
    minimum_effect_denominator: u32,
    planning_alternative_numerator: u32,
    planning_alternative_denominator: u32,
    repetition_event_probability: f64,
    repetition_detection_probability: f64,
    independent_attempt_success_probability: f64,
    independent_pool_success_probability: f64,
    per_attempt_additional_call_probability: f64,
    per_attempt_call_coverage_probability: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PowerAnalysis {
    pub schema_version: u32,
    pub method: &'static str,
    pub familywise_alpha: f64,
    pub alpha_per_claimable_stratum: f64,
    pub target_joint_power: f64,
    pub minimum_practically_useful_effect: f64,
    pub planning_alternative_effect: f64,
    pub power_separation: f64,
    pub reliability_per_contrast_power: f64,
    pub communication_per_contrast_power: f64,
    pub reliability_unrounded_tasks_per_stratum: f64,
    pub communication_unrounded_tasks_per_stratum: f64,
    pub reliability_required_distinct_tasks_per_stratum: u64,
    pub communication_required_distinct_tasks_per_stratum: u64,
    pub corpus_required_distinct_tasks_per_stratum: u64,
    pub stochastic_repetitions_per_task: u64,
    pub independent_best_of_n: u64,
    pub selector_quanta: u64,
    pub arm_attempt_quanta: u64,
    pub calls_per_attempt_quantum: u64,
    pub repetitions_count_as_distinct_tasks: bool,
}

pub fn load_frozen_manifest(
    corpus: &LoadedCorpus,
    manifest_path: &Path,
    digest_path: &Path,
) -> Result<FrozenStudyManifest> {
    validate_corpus_root_sha256(&corpus.registry_sha256)?;
    let bytes = fs::read(manifest_path)
        .with_context(|| format!("read study manifest {}", manifest_path.display()))?;
    let supplied_digest = fs::read_to_string(digest_path)
        .with_context(|| format!("read study digest {}", digest_path.display()))?;
    verify_frozen_manifest_bytes(&bytes, supplied_digest.trim())?;
    let value: Value = serde_json::from_slice(&bytes).context("parse frozen study manifest")?;
    let directory = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    validate_manifest_semantics(&value, &directory)?;
    let inputs_path = directory.join(manifest_string(&value, "/artifacts/power_inputs/path")?);
    let inputs_bytes = fs::read(&inputs_path)
        .with_context(|| format!("read power inputs {}", inputs_path.display()))?;
    let inputs_digest = sha256_bytes(&inputs_bytes);
    ensure!(
        inputs_digest == manifest_string(&value, "/artifacts/power_inputs/sha256")?,
        "power-input digest does not match the frozen manifest"
    );
    let power_inputs: PowerInputs =
        serde_json::from_slice(&inputs_bytes).context("parse power inputs")?;
    let frozen = FrozenStudyManifest {
        value,
        digest: FROZEN_STUDY_MANIFEST_SHA256.to_owned(),
        directory,
        power_inputs,
    };
    let calculated = power_analysis(&frozen)?;
    validate_power_outputs(&frozen, &calculated)?;
    validate_bound_artifacts(&frozen, &calculated)?;
    Ok(frozen)
}

fn validate_corpus_root_sha256(registry_sha256: &str) -> Result<()> {
    ensure!(
        registry_sha256 == APPROVED_INITIAL_CORPUS_ROOT,
        "loaded corpus root is not the owner-approved initial root"
    );
    Ok(())
}

fn verify_frozen_manifest_bytes(bytes: &[u8], supplied_digest: &str) -> Result<()> {
    let actual = sha256_bytes(bytes);
    ensure!(
        supplied_digest == FROZEN_STUDY_MANIFEST_SHA256,
        "study digest sidecar differs from the compiled freeze"
    );
    ensure!(
        actual == FROZEN_STUDY_MANIFEST_SHA256,
        "study manifest changed after freeze"
    );
    Ok(())
}

fn validate_manifest_semantics(value: &Value, directory: &Path) -> Result<()> {
    ensure!(
        value.pointer("/schema_version").and_then(Value::as_u64) == Some(1),
        "unsupported study-manifest schema"
    );
    ensure!(
        manifest_string(value, "/study_id")? == "w1-exp-01b-matched-budget-v1",
        "unexpected study id"
    );
    ensure!(
        manifest_string(value, "/artifact_role")? == "technical_study_specification",
        "study manifest claims an unsupported authority"
    );
    ensure!(
        manifest_string(value, "/authorization_scope")? == "technical_verification_only",
        "study manifest must not claim owner authorization"
    );
    ensure!(
        value
            .pointer("/owner_authorization_evaluated")
            .and_then(Value::as_bool)
            == Some(false),
        "technical manifest cannot evaluate owner authorization"
    );
    ensure!(
        value
            .pointer("/primary_outcomes_observed")
            .and_then(Value::as_bool)
            == Some(false),
        "manifest cannot be frozen after primary outcomes"
    );
    ensure!(
        manifest_string(value, "/corpus/approved_initial_root_sha256")?
            == APPROVED_INITIAL_CORPUS_ROOT,
        "manifest is not bound to the approved corpus root"
    );
    ensure!(
        value
            .pointer("/corpus/initial_statistically_sufficient")
            .and_then(Value::as_bool)
            == Some(false),
        "the first four packages cannot claim statistical sufficiency"
    );
    ensure!(
        value
            .pointer("/corpus/external_package_ceiling")
            .is_some_and(Value::is_null),
        "external package ceiling must remain absent"
    );
    ensure!(
        value
            .pointer("/resources/project_currency_ceiling/minor_units")
            .is_some_and(Value::is_null)
            && value
                .pointer("/resources/project_currency_ceiling/currency")
                .is_some_and(Value::is_null),
        "the optional project currency ceiling must remain null"
    );
    ensure!(
        manifest_string(value, "/resources/project_currency_ceiling/scope")? == "project",
        "a currency ceiling may only have project scope"
    );
    ensure!(
        value
            .pointer("/resources/equal_arm_budget")
            .and_then(Value::as_object)
            .is_some(),
        "equal arm budget is missing"
    );
    ensure!(
        manifest_string(value, "/exclusions/post_randomization")? == "none",
        "post-randomization exclusions are forbidden"
    );
    ensure!(
        manifest_string(value, "/outcomes/primary")?
            == "indicator(terminal_outcome == accepted && exact protected verifier decision == passed)",
        "primary outcome changed"
    );
    ensure!(
        manifest_string(value, "/stopping/study_rule")?
            == "fixed sample; no efficacy, futility, cost, or conditional-power look",
        "stopping rule changed"
    );
    ensure!(
        directory.is_dir(),
        "study manifest has no readable artifact directory"
    );
    Ok(())
}

fn validate_bound_artifacts(
    manifest: &FrozenStudyManifest,
    calculated: &PowerAnalysis,
) -> Result<()> {
    for name in ["protocol", "power_inputs", "power_report"] {
        let path = manifest
            .directory
            .join(manifest.string(&format!("/artifacts/{name}/path"))?);
        let expected = manifest.string(&format!("/artifacts/{name}/sha256"))?;
        let bytes = fs::read(&path)
            .with_context(|| format!("read frozen study artifact {}", path.display()))?;
        ensure!(
            sha256_bytes(&bytes) == expected,
            "frozen study artifact digest mismatch: {}",
            path.display()
        );
    }
    let code_digest = sha256_bytes(include_bytes!("study.rs"));
    ensure!(
        code_digest == manifest.string("/artifacts/power_and_analysis_code_sha256")?,
        "power and analysis code digest mismatch"
    );
    let report_path = manifest
        .directory
        .join(manifest.string("/artifacts/power_report/path")?);
    let report: Value = serde_json::from_slice(
        &fs::read(&report_path)
            .with_context(|| format!("read frozen power report {}", report_path.display()))?,
    )
    .context("parse frozen power report")?;
    let calculated_value = serde_json::to_value(calculated)?;
    for (name, expected) in calculated_value
        .as_object()
        .context("calculated power report is not an object")?
    {
        let observed = report.get(name);
        let matches = match (observed.and_then(Value::as_f64), expected.as_f64()) {
            (Some(observed), Some(expected)) => (observed - expected).abs() <= 1.0e-9,
            _ => observed == Some(expected),
        };
        ensure!(
            matches,
            "frozen power-report field differs from the executable calculation: {name}; observed={observed:?}, expected={expected}"
        );
    }
    for (pointer, expected) in [
        (
            "/expansion_from_initial_edition/decomposable_additional_required",
            calculated.corpus_required_distinct_tasks_per_stratum - 2,
        ),
        (
            "/expansion_from_initial_edition/strongly_sequential_additional_required",
            calculated.corpus_required_distinct_tasks_per_stratum - 2,
        ),
        (
            "/expansion_from_initial_edition/total_additional_required",
            2 * (calculated.corpus_required_distinct_tasks_per_stratum - 2),
        ),
    ] {
        ensure!(
            report.pointer(pointer).and_then(Value::as_u64) == Some(expected),
            "frozen power-report expansion field differs from the executable calculation: {pointer}"
        );
    }
    Ok(())
}

fn manifest_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .with_context(|| format!("manifest field {pointer} is not a string"))
}

pub fn power_analysis(manifest: &FrozenStudyManifest) -> Result<PowerAnalysis> {
    let input = &manifest.power_inputs;
    ensure!(input.schema_version == 1, "unsupported power-input schema");
    ensure!(
        (0.0..1.0).contains(&input.familywise_alpha)
            && (0.0..1.0).contains(&input.target_joint_power),
        "invalid alpha or power"
    );
    ensure!(
        input.minimum_effect_numerator > 0
            && input.minimum_effect_denominator > input.minimum_effect_numerator,
        "invalid minimum effect"
    );
    let effect =
        f64::from(input.minimum_effect_numerator) / f64::from(input.minimum_effect_denominator);
    ensure!(
        input.planning_alternative_numerator > 0
            && input.planning_alternative_denominator > input.planning_alternative_numerator,
        "invalid planning alternative"
    );
    let planning_alternative = f64::from(input.planning_alternative_numerator)
        / f64::from(input.planning_alternative_denominator);
    let power_separation = planning_alternative - effect;
    ensure!(
        power_separation > 0.0,
        "planning alternative must exceed the useful-effect boundary"
    );
    let alpha_per_stratum = input.familywise_alpha / f64::from(input.claimable_strata);
    let reliability_power = 1.0
        - (1.0 - input.target_joint_power)
            / f64::from(input.reliability_required_contrasts_for_broad_claim);
    let communication_power = 1.0
        - (1.0 - input.target_joint_power)
            / f64::from(input.communication_required_contrasts_for_broad_claim);
    let difference_range =
        input.paired_cluster_difference_upper_bound - input.paired_cluster_difference_lower_bound;
    ensure!(
        input.paired_cluster_difference_lower_bound == CLUSTER_DIFFERENCE_LOWER_BOUND
            && input.paired_cluster_difference_upper_bound == CLUSTER_DIFFERENCE_UPPER_BOUND,
        "paired task-cluster difference bounds changed"
    );
    let reliability_unrounded = bounded_mean_sample_size(
        difference_range,
        power_separation,
        alpha_per_stratum,
        reliability_power,
    )?;
    let communication_unrounded = bounded_mean_sample_size(
        difference_range,
        power_separation,
        alpha_per_stratum,
        communication_power,
    )?;
    let reliability_tasks = reliability_unrounded.ceil() as u64;
    let communication_tasks = communication_unrounded.ceil() as u64;
    let repetitions = geometric_trials(
        input.repetition_event_probability,
        input.repetition_detection_probability,
    )?;
    let best_of_n = geometric_trials(
        input.independent_attempt_success_probability,
        input.independent_pool_success_probability,
    )?;
    let calls_per_attempt_quantum = geometric_trials(
        input.per_attempt_additional_call_probability,
        input.per_attempt_call_coverage_probability,
    )?;
    Ok(PowerAnalysis {
        schema_version: 1,
        method: "distribution-free Hoeffding fixed-sample bound for paired task-cluster differences in [-1,1], testing the 0.125 boundary against a 0.25 alternative; repetitions receive zero distinct-task credit",
        familywise_alpha: input.familywise_alpha,
        alpha_per_claimable_stratum: alpha_per_stratum,
        target_joint_power: input.target_joint_power,
        minimum_practically_useful_effect: effect,
        planning_alternative_effect: planning_alternative,
        power_separation,
        reliability_per_contrast_power: reliability_power,
        communication_per_contrast_power: communication_power,
        reliability_unrounded_tasks_per_stratum: reliability_unrounded,
        communication_unrounded_tasks_per_stratum: communication_unrounded,
        reliability_required_distinct_tasks_per_stratum: reliability_tasks,
        communication_required_distinct_tasks_per_stratum: communication_tasks,
        corpus_required_distinct_tasks_per_stratum: reliability_tasks.max(communication_tasks),
        stochastic_repetitions_per_task: repetitions,
        independent_best_of_n: best_of_n,
        selector_quanta: 1,
        arm_attempt_quanta: best_of_n + 1,
        calls_per_attempt_quantum,
        repetitions_count_as_distinct_tasks: false,
    })
}

fn validate_power_outputs(
    manifest: &FrozenStudyManifest,
    calculated: &PowerAnalysis,
) -> Result<()> {
    let expected = [
        (
            "/power/reliability_required_distinct_tasks_per_stratum",
            calculated.reliability_required_distinct_tasks_per_stratum,
        ),
        (
            "/power/communication_required_distinct_tasks_per_stratum",
            calculated.communication_required_distinct_tasks_per_stratum,
        ),
        (
            "/corpus/required_distinct_tasks_per_stratum/decomposable",
            calculated.corpus_required_distinct_tasks_per_stratum,
        ),
        (
            "/corpus/required_distinct_tasks_per_stratum/strongly_sequential",
            calculated.corpus_required_distinct_tasks_per_stratum,
        ),
        (
            "/randomization/repetitions_per_task",
            calculated.stochastic_repetitions_per_task,
        ),
        (
            "/resources/independent_best_of_n",
            calculated.independent_best_of_n,
        ),
        (
            "/resources/arm_attempt_quanta",
            calculated.arm_attempt_quanta,
        ),
        (
            "/resources/calls_per_attempt_quantum",
            calculated.calls_per_attempt_quantum,
        ),
    ];
    for (pointer, value) in expected {
        ensure!(
            manifest.u64(pointer)? == value,
            "manifest field {pointer} differs from the reproducible power calculation"
        );
    }
    ensure!(
        !manifest.bool("/power/repetitions_count_as_distinct_tasks")?,
        "repetitions cannot increase the distinct-task count"
    );
    for (pointer, expected) in [
        (
            "/power/minimum_effect",
            calculated.minimum_practically_useful_effect,
        ),
        (
            "/power/planning_alternative_effect",
            calculated.planning_alternative_effect,
        ),
        ("/power/power_separation", calculated.power_separation),
    ] {
        ensure!(
            (manifest.f64(pointer)? - expected).abs() <= f64::EPSILON,
            "manifest field {pointer} differs from the reproducible power calculation"
        );
    }
    let budget: BudgetVector = serde_json::from_value(
        manifest
            .value
            .pointer("/resources/equal_arm_budget")
            .context("missing equal arm budget")?
            .clone(),
    )
    .context("parse equal arm budget")?;
    ensure!(
        budget == BudgetVector::from_power(calculated),
        "equal arm budget is not the declared mechanical resource scale"
    );
    Ok(())
}

fn bounded_mean_sample_size(range: f64, effect: f64, alpha: f64, power: f64) -> Result<f64> {
    ensure!(
        range > 0.0 && effect > 0.0 && effect < range,
        "invalid bounded-mean range or effect"
    );
    ensure!(
        (0.0..1.0).contains(&alpha) && (0.0..1.0).contains(&power),
        "invalid bounded-mean alpha or power"
    );
    let beta = 1.0 - power;
    Ok(range.powi(2) / (2.0 * effect.powi(2))
        * ((1.0 / alpha).ln().sqrt() + (1.0 / beta).ln().sqrt()).powi(2))
}

fn geometric_trials(event_probability: f64, target_probability: f64) -> Result<u64> {
    ensure!(
        (0.0..1.0).contains(&event_probability) && (0.0..1.0).contains(&target_probability),
        "geometric-design probabilities must be in (0, 1)"
    );
    Ok(((1.0 - target_probability).ln() / (1.0 - event_probability).ln()).ceil() as u64)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetVector {
    attempt_quanta: u64,
    model_calls: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    wall_time_ms: u64,
    max_parallel_requests: u64,
    protected_queries: u64,
}

impl BudgetVector {
    fn from_power(power: &PowerAnalysis) -> Self {
        Self {
            attempt_quanta: power.arm_attempt_quanta,
            model_calls: power.arm_attempt_quanta * power.calls_per_attempt_quantum,
            input_tokens: power.arm_attempt_quanta * power.calls_per_attempt_quantum * 131_072,
            cached_input_tokens: power.arm_attempt_quanta
                * power.calls_per_attempt_quantum
                * 131_072,
            output_tokens: power.arm_attempt_quanta * power.calls_per_attempt_quantum * 16_384,
            reasoning_output_tokens: power.arm_attempt_quanta
                * power.calls_per_attempt_quantum
                * 16_384,
            wall_time_ms: power.arm_attempt_quanta * power.calls_per_attempt_quantum * 600_000,
            max_parallel_requests: power.independent_best_of_n,
            protected_queries: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudyRecords {
    schema_version: u32,
    mode: String,
    manifest_sha256: String,
    corpus_root_sha256: String,
    primary_blocks: Vec<PrimaryBlock>,
    communication_episodes: Vec<CommunicationEpisode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrimaryBlock {
    block_id: String,
    task_id: String,
    stratum: String,
    repetition: u64,
    seed: u64,
    assignment_order: Vec<String>,
    runtime_profile_sha256: String,
    model_route_sha256: String,
    arms: Vec<ArmRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArmRecord {
    condition: String,
    assignment_position: u64,
    runtime_profile_sha256: String,
    budget_opportunity: BudgetVector,
    participant_starts: u64,
    selection_commit_sequence: u64,
    protected_result_reveal_sequence: Option<u64>,
    terminal_outcome: String,
    candidate_sha256: Option<String>,
    verifier_decision: String,
    excluded: bool,
    exclusion_reason: Option<String>,
    blinded_selector: Option<BlindedSelector>,
    audit_false_acceptance: Option<bool>,
    accounting: RunAccounting,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BlindedSelector {
    assessment_commit_sequence: u64,
    producer_identity_reveal_sequence: u64,
    arm_identity_reveal_sequence: u64,
    producer_rationale_visible_before_commit: bool,
    collaboration_messages_visible_before_commit: bool,
    other_assessments_visible_before_commit: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RunAccounting {
    wall_time_ms: u64,
    max_parallelism_observed: u64,
    protected_queries: u64,
    routes: Vec<RouteAccounting>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RouteAccounting {
    route_sha256: String,
    model_calls: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    errors: u64,
    retries: u64,
    provider_cost_minor_units: u64,
    provider_cost_currency: String,
    in_flight_overshoot: OvershootAccounting,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OvershootAccounting {
    model_calls: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    provider_cost_minor_units: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommunicationEpisode {
    episode_id: String,
    task_id: String,
    stratum: String,
    selection_sha256: String,
    selected_without_message_content_or_outcome: bool,
    receiver_endpoint_committed_before_intervention: bool,
    variants: Vec<CommunicationVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommunicationVariant {
    intervention: String,
    budget_opportunity: BudgetVector,
    receiver_target_action: bool,
    task_value: bool,
    terminal_outcome: String,
    accounting: RunAccounting,
}

pub fn load_study_records(path: &Path) -> Result<StudyRecords> {
    let bytes = fs::read(path).with_context(|| format!("read study records {}", path.display()))?;
    serde_json::from_slice(&bytes).context("parse study records")
}

pub fn analyze_study_records(
    corpus: &LoadedCorpus,
    manifest: &FrozenStudyManifest,
    records: &StudyRecords,
) -> Result<Value> {
    validate_records(corpus, manifest, records)?;
    let power = power_analysis(manifest)?;
    let primary = primary_analysis(records, &power);
    let communication = communication_analysis(records, &power);
    let reliability = reliability_analysis(records, manifest);
    Ok(json!({
        "schema_version": 1,
        "study_id": manifest.string("/study_id")?,
        "manifest_sha256": manifest.digest,
        "corpus_root_sha256": APPROVED_INITIAL_CORPUS_ROOT,
        "authorization_scope": "technical_verification_only",
        "owner_authorization_evaluated": false,
        "data_mode": records.mode,
        "primary": primary,
        "instrument_reliability": reliability,
        "communication_interventions": communication,
        "decision": {
            "verdict": "synthetic_only_no_product_decision",
            "reason": "synthetic records exercise the frozen analysis but cannot satisfy owner authorization, corpus breadth, runtime-profile admission, or primary evidence",
            "criterion_changed_after_results": false
        }
    }))
}

fn validate_records(
    corpus: &LoadedCorpus,
    manifest: &FrozenStudyManifest,
    records: &StudyRecords,
) -> Result<()> {
    ensure!(
        records.schema_version == 1,
        "unsupported study-record schema"
    );
    ensure!(
        records.mode == "synthetic",
        "only synthetic dry-run records are admitted here"
    );
    ensure!(
        records.manifest_sha256 == manifest.digest,
        "record manifest digest mismatch"
    );
    ensure!(
        records.corpus_root_sha256 == APPROVED_INITIAL_CORPUS_ROOT,
        "record corpus root mismatch"
    );
    let expected_budget: BudgetVector = serde_json::from_value(
        manifest
            .value
            .pointer("/resources/equal_arm_budget")
            .context("missing frozen budget")?
            .clone(),
    )?;
    let required_conditions = ["single_strong", "independent_best_of_n", "coordinated_ymp"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut block_ids = BTreeSet::new();
    let mut task_repetitions = BTreeSet::new();
    for block in &records.primary_blocks {
        ensure!(block_ids.insert(&block.block_id), "duplicate primary block");
        ensure!(
            task_repetitions.insert((&block.task_id, block.repetition)),
            "duplicate task repetition"
        );
        ensure!(
            (1..=manifest.u64("/randomization/repetitions_per_task")?).contains(&block.repetition),
            "repetition is outside the frozen range"
        );
        ensure_lower_sha256(&block.runtime_profile_sha256, "runtime profile")?;
        ensure_lower_sha256(&block.model_route_sha256, "model route")?;
        ensure!(
            block.seed == expected_block_seed(manifest, &block.task_id, block.repetition)?,
            "block seed differs from the frozen deterministic assignment"
        );
        ensure!(
            block.assignment_order
                == expected_assignment_order(manifest, &block.task_id, block.repetition),
            "assignment order differs from the frozen deterministic assignment"
        );
        validate_task_stratum(corpus, &block.task_id, &block.stratum)?;
        let conditions = block
            .arms
            .iter()
            .map(|arm| arm.condition.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            conditions == required_conditions && block.arms.len() == required_conditions.len(),
            "primary block must contain each condition exactly once"
        );
        for arm in &block.arms {
            let assignment_position = usize::try_from(arm.assignment_position).ok();
            ensure!(
                assignment_position.and_then(|position| block.assignment_order.get(position))
                    == Some(&arm.condition),
                "arm record does not correspond to its assigned launch position"
            );
            ensure!(
                arm.runtime_profile_sha256 == block.runtime_profile_sha256,
                "arm runtime profile differs from its frozen block profile"
            );
            ensure!(
                arm.accounting.routes.len() == 1
                    && arm.accounting.routes[0].route_sha256 == block.model_route_sha256,
                "arm model route differs from its frozen block route"
            );
            ensure!(
                arm.budget_opportunity == expected_budget,
                "unequal arm budget opportunity"
            );
            ensure!(
                !arm.excluded && arm.exclusion_reason.is_none(),
                "post-randomization exclusion is forbidden"
            );
            validate_protected_result_order(arm)?;
            validate_terminal_and_verifier(arm)?;
            validate_selector(arm, manifest)?;
            validate_accounting(&arm.accounting, &expected_budget)?;
            let max_participants = match arm.condition.as_str() {
                "single_strong" => 1,
                "independent_best_of_n" => manifest.u64("/resources/independent_best_of_n")?,
                "coordinated_ymp" => manifest.u64("/resources/independent_best_of_n")?,
                _ => bail!("unknown condition {}", arm.condition),
            };
            ensure!(
                (1..=max_participants).contains(&arm.participant_starts),
                "participant topology violates the frozen condition"
            );
        }
    }
    ensure!(
        !records.primary_blocks.is_empty(),
        "dry run has no primary blocks"
    );
    validate_communication(corpus, manifest, records, &expected_budget)?;
    Ok(())
}

fn expected_block_seed(
    manifest: &FrozenStudyManifest,
    task_id: &str,
    repetition: u64,
) -> Result<u64> {
    let digest = schedule_digest(
        "ymp-study-seed-v1",
        &manifest.digest,
        task_id,
        repetition,
        None,
    );
    u64::from_str_radix(&digest[..16], 16).context("derive frozen block seed")
}

fn expected_assignment_order(
    manifest: &FrozenStudyManifest,
    task_id: &str,
    repetition: u64,
) -> Vec<String> {
    let mut conditions = ["single_strong", "independent_best_of_n", "coordinated_ymp"]
        .into_iter()
        .map(|condition| {
            (
                schedule_digest(
                    "ymp-study-order-v1",
                    &manifest.digest,
                    task_id,
                    repetition,
                    Some(condition),
                ),
                condition.to_owned(),
            )
        })
        .collect::<Vec<_>>();
    conditions.sort();
    conditions
        .into_iter()
        .map(|(_, condition)| condition)
        .collect()
}

fn schedule_digest(
    domain: &str,
    manifest_sha256: &str,
    task_id: &str,
    repetition: u64,
    condition: Option<&str>,
) -> String {
    let mut input = format!("{domain}\0{manifest_sha256}\0{task_id}\0{repetition}");
    if let Some(condition) = condition {
        input.push('\0');
        input.push_str(condition);
    }
    sha256_bytes(input.as_bytes())
}

fn validate_task_stratum(corpus: &LoadedCorpus, task_id: &str, stratum: &str) -> Result<()> {
    let task = corpus
        .tasks
        .iter()
        .find(|task| task.id == task_id)
        .with_context(|| format!("record names task outside approved root: {task_id}"))?;
    let expected = match task.classification {
        Classification::Decomposable => "decomposable",
        Classification::StronglySequential => "strongly_sequential",
    };
    ensure!(stratum == expected, "record task stratum changed");
    Ok(())
}

fn validate_terminal_and_verifier(arm: &ArmRecord) -> Result<()> {
    let terminals = [
        "accepted",
        "exhausted",
        "abstained",
        "cancelled",
        "infrastructure_error",
    ];
    ensure!(
        terminals.contains(&arm.terminal_outcome.as_str()),
        "undeclared terminal outcome"
    );
    if arm.terminal_outcome == "accepted" {
        ensure!(
            arm.verifier_decision == "passed",
            "accepted outcome lacks exact verifier pass"
        );
        ensure!(
            arm.candidate_sha256.as_deref().is_some_and(is_lower_sha256),
            "accepted outcome lacks an exact candidate digest"
        );
        ensure!(
            arm.audit_false_acceptance.is_some(),
            "accepted outcome is missing the blinded false-acceptance audit"
        );
    } else {
        ensure!(
            arm.verifier_decision != "passed",
            "non-accepted terminal substituted for a verifier pass"
        );
        ensure!(
            arm.audit_false_acceptance.is_none(),
            "non-accepted outcome cannot enter the false-acceptance denominator"
        );
    }
    Ok(())
}

fn validate_protected_result_order(arm: &ArmRecord) -> Result<()> {
    ensure!(
        arm.selection_commit_sequence > 0,
        "candidate selection lacks a committed event sequence"
    );
    if arm.accounting.protected_queries == 0 {
        ensure!(
            arm.protected_result_reveal_sequence.is_none(),
            "protected result was revealed without a protected query"
        );
    } else {
        let reveal = arm
            .protected_result_reveal_sequence
            .context("protected query lacks a result-reveal event sequence")?;
        ensure!(
            arm.selection_commit_sequence < reveal,
            "protected result was revealed before candidate selection was committed"
        );
    }
    Ok(())
}

fn validate_selector(arm: &ArmRecord, manifest: &FrozenStudyManifest) -> Result<()> {
    if arm.condition == "independent_best_of_n" {
        let blind = arm
            .blinded_selector
            .as_ref()
            .context("independent selection lacks a blinded commitment")?;
        ensure!(
            blind.assessment_commit_sequence == arm.selection_commit_sequence,
            "blinded selector assessment does not bind the selected candidate"
        );
        ensure!(
            blind.assessment_commit_sequence < blind.producer_identity_reveal_sequence
                && blind.assessment_commit_sequence < blind.arm_identity_reveal_sequence,
            "selector received identity before its commitment"
        );
        ensure!(
            !blind.producer_rationale_visible_before_commit
                && !blind.collaboration_messages_visible_before_commit
                && !blind.other_assessments_visible_before_commit,
            "selector received prohibited context before its commitment"
        );
        ensure!(
            arm.participant_starts == manifest.u64("/resources/independent_best_of_n")?,
            "independent arm did not start the frozen number of producers"
        );
    } else {
        ensure!(
            arm.blinded_selector.is_none(),
            "selector metadata appeared in a non-selector condition"
        );
    }
    Ok(())
}

fn validate_accounting(accounting: &RunAccounting, budget: &BudgetVector) -> Result<()> {
    ensure!(
        accounting.routes.len() == 1,
        "each admitted runtime profile must bind exactly one model route"
    );
    ensure!(
        accounting.wall_time_ms <= budget.wall_time_ms,
        "wall-time opportunity was exceeded"
    );
    ensure!(
        accounting.max_parallelism_observed <= budget.max_parallel_requests,
        "parallelism opportunity was exceeded"
    );
    ensure!(
        accounting.protected_queries <= budget.protected_queries,
        "protected-query budget was exceeded"
    );
    let mut totals = [0_u64; 5];
    let mut overshoot = [0_u64; 5];
    for route in &accounting.routes {
        ensure_lower_sha256(&route.route_sha256, "model route")?;
        ensure!(
            !route.provider_cost_currency.trim().is_empty(),
            "provider cost currency is missing"
        );
        totals[0] = totals[0]
            .checked_add(route.model_calls)
            .context("call total overflow")?;
        totals[1] = totals[1]
            .checked_add(route.input_tokens)
            .context("input total overflow")?;
        totals[2] = totals[2]
            .checked_add(route.cached_input_tokens)
            .context("cached-input total overflow")?;
        totals[3] = totals[3]
            .checked_add(route.output_tokens)
            .context("output total overflow")?;
        totals[4] = totals[4]
            .checked_add(route.reasoning_output_tokens)
            .context("reasoning total overflow")?;
        overshoot[0] += route.in_flight_overshoot.model_calls;
        overshoot[1] += route.in_flight_overshoot.input_tokens;
        overshoot[2] += route.in_flight_overshoot.cached_input_tokens;
        overshoot[3] += route.in_flight_overshoot.output_tokens;
        overshoot[4] += route.in_flight_overshoot.reasoning_output_tokens;
    }
    let limits = [
        budget.model_calls,
        budget.input_tokens,
        budget.cached_input_tokens,
        budget.output_tokens,
        budget.reasoning_output_tokens,
    ];
    for ((total, limit), excess) in totals.into_iter().zip(limits).zip(overshoot) {
        ensure!(
            total <= limit.saturating_add(excess),
            "route usage exceeds budget without matching in-flight overshoot"
        );
    }
    Ok(())
}

fn validate_communication(
    corpus: &LoadedCorpus,
    manifest: &FrozenStudyManifest,
    records: &StudyRecords,
    expected_budget: &BudgetVector,
) -> Result<()> {
    let required = [
        "original_message",
        "no_message",
        "neutral_same_bytes",
        "cross_task_or_sender_shuffle",
        "direct_evidence",
        "delayed_message",
        "false_finding",
        "participant_removed",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for episode in &records.communication_episodes {
        ensure!(
            ids.insert(&episode.episode_id),
            "duplicate communication episode"
        );
        validate_task_stratum(corpus, &episode.task_id, &episode.stratum)?;
        ensure_lower_sha256(&episode.selection_sha256, "episode selection")?;
        ensure!(
            episode.selected_without_message_content_or_outcome
                && episode.receiver_endpoint_committed_before_intervention,
            "communication episode was selected or configured after observing content or outcome"
        );
        let variants = episode
            .variants
            .iter()
            .map(|variant| variant.intervention.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            variants == required && episode.variants.len() == required.len(),
            "communication episode lacks a frozen intervention"
        );
        for variant in &episode.variants {
            ensure!(
                variant.budget_opportunity == *expected_budget,
                "communication intervention has unequal budget"
            );
            ensure!(
                [
                    "accepted",
                    "exhausted",
                    "abstained",
                    "cancelled",
                    "infrastructure_error"
                ]
                .contains(&variant.terminal_outcome.as_str()),
                "communication intervention has an invalid terminal outcome"
            );
            validate_accounting(&variant.accounting, expected_budget)?;
        }
    }
    ensure!(
        !records.communication_episodes.is_empty(),
        "dry run has no communication episodes"
    );
    ensure!(
        manifest.string("/communication/private_chain_of_thought")? == "not_collected",
        "communication study cannot collect private chain-of-thought"
    );
    Ok(())
}

fn ensure_lower_sha256(value: &str, label: &str) -> Result<()> {
    ensure!(is_lower_sha256(value), "{label} is not a lowercase SHA-256");
    Ok(())
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn primary_analysis(records: &StudyRecords, power: &PowerAnalysis) -> Value {
    let mut strata = BTreeMap::new();
    for stratum in ["decomposable", "strongly_sequential"] {
        let rates = ["single_strong", "independent_best_of_n", "coordinated_ymp"]
            .into_iter()
            .map(|condition| {
                let values = acceptance_values(records, stratum, condition);
                (condition, binary_summary(&values))
            })
            .collect::<BTreeMap<_, _>>();
        let against_single =
            clustered_difference(records, stratum, "coordinated_ymp", "single_strong");
        let against_independent =
            clustered_difference(records, stratum, "coordinated_ymp", "independent_best_of_n");
        let distinct_tasks = records
            .primary_blocks
            .iter()
            .filter(|block| block.stratum == stratum)
            .map(|block| block.task_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        strata.insert(
            stratum,
            json!({
                "acceptance": rates,
                "coordinated_minus_single": against_single,
                "coordinated_minus_independent": against_independent,
                "distinct_tasks": distinct_tasks,
                "required_distinct_tasks": power.corpus_required_distinct_tasks_per_stratum,
                "sample_complete": distinct_tasks as u64 >= power.corpus_required_distinct_tasks_per_stratum
            }),
        );
    }
    json!({
        "primary_outcome": "independently verified accepted-result indicator",
        "minimum_practically_useful_effect": power.minimum_practically_useful_effect,
        "planning_alternative_effect": power.planning_alternative_effect,
        "cluster_unit": "distinct task",
        "strata": strata
    })
}

fn acceptance_values(records: &StudyRecords, stratum: &str, condition: &str) -> Vec<f64> {
    records
        .primary_blocks
        .iter()
        .filter(|block| block.stratum == stratum)
        .filter_map(|block| {
            block
                .arms
                .iter()
                .find(|arm| arm.condition == condition)
                .map(|arm| f64::from(arm.terminal_outcome == "accepted"))
        })
        .collect()
}

fn binary_summary(values: &[f64]) -> Value {
    let successes = values.iter().sum::<f64>();
    let n = values.len() as f64;
    let estimate = if n == 0.0 { 0.0 } else { successes / n };
    let (lower, upper) = wilson_interval(successes, n, Z_REPORTING_95);
    json!({
        "successes": successes as u64,
        "observations": values.len(),
        "estimate": estimate,
        "ci_95": [lower, upper]
    })
}

fn clustered_difference(
    records: &StudyRecords,
    stratum: &str,
    treatment: &str,
    baseline: &str,
) -> Value {
    let mut task_values: BTreeMap<&str, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for block in records
        .primary_blocks
        .iter()
        .filter(|block| block.stratum == stratum)
    {
        let treatment_value = block
            .arms
            .iter()
            .find(|arm| arm.condition == treatment)
            .is_some_and(|arm| arm.terminal_outcome == "accepted");
        let baseline_value = block
            .arms
            .iter()
            .find(|arm| arm.condition == baseline)
            .is_some_and(|arm| arm.terminal_outcome == "accepted");
        let entry = task_values.entry(&block.task_id).or_default();
        entry.0.push(f64::from(treatment_value));
        entry.1.push(f64::from(baseline_value));
    }
    let differences = task_values
        .into_values()
        .map(|(treatment_values, baseline_values)| mean(&treatment_values) - mean(&baseline_values))
        .collect::<Vec<_>>();
    mean_interval(&differences)
}

fn mean_interval(values: &[f64]) -> Value {
    if values.is_empty() {
        return json!({
            "tasks": 0,
            "estimate": null,
            "empirical_standard_error": null,
            "ci_95": [null, null],
            "decision_lower_alpha_0_025": null,
            "decision_upper_alpha_0_025": null
        });
    }
    let estimate = mean(values);
    let variance = if values.len() > 1 {
        values
            .iter()
            .map(|value| (value - estimate).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64
    } else {
        0.0
    };
    let standard_error = (variance / values.len() as f64).sqrt();
    let radius = hoeffding_radius(
        CLUSTER_DIFFERENCE_UPPER_BOUND - CLUSTER_DIFFERENCE_LOWER_BOUND,
        DECISION_ALPHA,
        values.len(),
    );
    json!({
        "tasks": values.len(),
        "estimate": estimate,
        "empirical_standard_error": standard_error,
        "ci_95": [
            (estimate - radius).max(CLUSTER_DIFFERENCE_LOWER_BOUND),
            (estimate + radius).min(CLUSTER_DIFFERENCE_UPPER_BOUND)
        ],
        "decision_lower_alpha_0_025":
            (estimate - radius).max(CLUSTER_DIFFERENCE_LOWER_BOUND),
        "decision_upper_alpha_0_025":
            (estimate + radius).min(CLUSTER_DIFFERENCE_UPPER_BOUND)
    })
}

fn hoeffding_radius(range: f64, alpha: f64, observations: usize) -> f64 {
    range * ((1.0 / alpha).ln() / (2.0 * observations as f64)).sqrt()
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn wilson_interval(successes: f64, n: f64, z: f64) -> (f64, f64) {
    if n == 0.0 {
        return (0.0, 1.0);
    }
    let proportion = successes / n;
    let denominator = 1.0 + z * z / n;
    let center = (proportion + z * z / (2.0 * n)) / denominator;
    let radius =
        z * ((proportion * (1.0 - proportion) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((center - radius).max(0.0), (center + radius).min(1.0))
}

fn reliability_analysis(records: &StudyRecords, manifest: &FrozenStudyManifest) -> Value {
    let mut by_condition = BTreeMap::new();
    for condition in ["single_strong", "independent_best_of_n", "coordinated_ymp"] {
        let arms = records
            .primary_blocks
            .iter()
            .flat_map(|block| &block.arms)
            .filter(|arm| arm.condition == condition)
            .collect::<Vec<_>>();
        let mut strata = BTreeMap::new();
        for stratum in ["decomposable", "strongly_sequential"] {
            let stratum_arms = records
                .primary_blocks
                .iter()
                .filter(|block| block.stratum == stratum)
                .flat_map(|block| &block.arms)
                .filter(|arm| arm.condition == condition)
                .collect::<Vec<_>>();
            strata.insert(stratum, reliability_arm_summary(&stratum_arms));
        }
        let mut summary = reliability_arm_summary(&arms);
        summary
            .as_object_mut()
            .expect("summary is an object")
            .insert("strata".to_owned(), json!(strata));
        by_condition.insert(condition, summary);
    }
    json!({
        "accounting_completeness_required": 1.0,
        "false_acceptance_upper_bound_limit": manifest
            .value
            .pointer("/instrument_reliability/false_acceptance_upper_bound")
            .and_then(Value::as_f64),
        "infrastructure_upper_bound_limit": manifest
            .value
            .pointer("/instrument_reliability/infrastructure_error_upper_bound")
            .and_then(Value::as_f64),
        "conditions": by_condition
    })
}

fn reliability_arm_summary(arms: &[&ArmRecord]) -> Value {
    let infrastructure = arms
        .iter()
        .filter(|arm| arm.terminal_outcome == "infrastructure_error")
        .count() as f64;
    let cancelled = arms
        .iter()
        .filter(|arm| arm.terminal_outcome == "cancelled")
        .count() as f64;
    let audited = arms
        .iter()
        .filter(|arm| arm.terminal_outcome == "accepted")
        .count() as f64;
    let false_accepts = arms
        .iter()
        .filter(|arm| arm.audit_false_acceptance == Some(true))
        .count() as f64;
    let (infra_lower, infra_upper) = wilson_interval(
        infrastructure + cancelled,
        arms.len() as f64,
        Z_REPORTING_95,
    );
    let (audit_lower, audit_upper) = wilson_interval(false_accepts, audited, Z_REPORTING_95);
    let accounting = arms.iter().map(|arm| &arm.accounting).collect::<Vec<_>>();
    json!({
        "observations": arms.len(),
        "infrastructure_or_cancelled": infrastructure + cancelled,
        "infrastructure_or_cancelled_ci_95": [infra_lower, infra_upper],
        "audited_acceptances": audited,
        "false_acceptances": false_accepts,
        "false_acceptance_ci_95": [audit_lower, audit_upper],
        "accounting": accounting_summary(&accounting)
    })
}

fn accounting_summary(accounting: &[&RunAccounting]) -> Value {
    let mut routes: BTreeMap<(&str, &str), [u64; 17]> = BTreeMap::new();
    for record in accounting {
        for route in &record.routes {
            let values = routes
                .entry((&route.route_sha256, &route.provider_cost_currency))
                .or_default();
            values[0] += route.model_calls;
            values[1] += route.input_tokens;
            values[2] += route.cached_input_tokens;
            values[3] += route.output_tokens;
            values[4] += route.reasoning_output_tokens;
            values[5] += route.errors;
            values[6] += route.retries;
            values[7] += route.provider_cost_minor_units;
            values[8] += route.in_flight_overshoot.model_calls;
            values[9] += route.in_flight_overshoot.input_tokens;
            values[10] += route.in_flight_overshoot.cached_input_tokens;
            values[11] += route.in_flight_overshoot.output_tokens;
            values[12] += route.in_flight_overshoot.reasoning_output_tokens;
            values[13] += route.in_flight_overshoot.provider_cost_minor_units;
            values[14] += record.wall_time_ms;
            values[15] += record.protected_queries;
            values[16] = values[16].max(record.max_parallelism_observed);
        }
    }
    let routes = routes
        .into_iter()
        .map(|((route, currency), values)| {
            json!({
                "route_sha256": route,
                "model_calls": values[0],
                "input_tokens": values[1],
                "cached_input_tokens": values[2],
                "output_tokens": values[3],
                "reasoning_output_tokens": values[4],
                "errors": values[5],
                "retries": values[6],
                "provider_cost_minor_units": values[7],
                "provider_cost_currency": currency,
                "in_flight_overshoot_model_calls": values[8],
                "in_flight_overshoot_input_tokens": values[9],
                "in_flight_overshoot_cached_input_tokens": values[10],
                "in_flight_overshoot_output_tokens": values[11],
                "in_flight_overshoot_reasoning_output_tokens": values[12],
                "in_flight_overshoot_provider_cost_minor_units": values[13],
                "wall_time_ms": values[14],
                "protected_queries": values[15],
                "max_parallelism_observed": values[16]
            })
        })
        .collect::<Vec<_>>();
    json!({
        "wall_time_ms": accounting.iter().map(|record| record.wall_time_ms).sum::<u64>(),
        "max_parallelism_observed": accounting
            .iter()
            .map(|record| record.max_parallelism_observed)
            .max()
            .unwrap_or(0),
        "protected_queries": accounting
            .iter()
            .map(|record| record.protected_queries)
            .sum::<u64>(),
        "routes": routes
    })
}

fn communication_analysis(records: &StudyRecords, power: &PowerAnalysis) -> Value {
    let mut strata = BTreeMap::new();
    for stratum in ["decomposable", "strongly_sequential"] {
        let episodes = records
            .communication_episodes
            .iter()
            .filter(|episode| episode.stratum == stratum)
            .collect::<Vec<_>>();
        let listening = communication_difference(&episodes, "original_message", "no_message", true);
        let value_no_message =
            communication_difference(&episodes, "original_message", "no_message", false);
        let value_direct =
            communication_difference(&episodes, "original_message", "direct_evidence", false);
        let distinct_tasks = episodes
            .iter()
            .map(|episode| episode.task_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        strata.insert(
            stratum,
            json!({
                "positive_listening_original_minus_no_message": listening,
                "task_value_original_minus_no_message": value_no_message,
                "task_value_original_minus_direct_evidence": value_direct,
                "distinct_task_linked_episodes": distinct_tasks,
                "required_distinct_task_linked_episodes": power.communication_required_distinct_tasks_per_stratum,
                "sample_complete": distinct_tasks as u64 >= power.communication_required_distinct_tasks_per_stratum
            }),
        );
    }
    let accounting = [
        "original_message",
        "no_message",
        "neutral_same_bytes",
        "cross_task_or_sender_shuffle",
        "direct_evidence",
        "delayed_message",
        "false_finding",
        "participant_removed",
    ]
    .into_iter()
    .map(|intervention| {
        let values = records
            .communication_episodes
            .iter()
            .flat_map(|episode| &episode.variants)
            .filter(|variant| variant.intervention == intervention)
            .map(|variant| &variant.accounting)
            .collect::<Vec<_>>();
        (intervention, accounting_summary(&values))
    })
    .collect::<BTreeMap<_, _>>();
    json!({
        "confirmatory_interventions": [
            "original_message",
            "no_message",
            "direct_evidence"
        ],
        "secondary_interventions": [
            "neutral_same_bytes",
            "cross_task_or_sender_shuffle",
            "delayed_message",
            "false_finding",
            "participant_removed"
        ],
        "strata": strata,
        "accounting_by_intervention": accounting
    })
}

fn communication_difference(
    episodes: &[&CommunicationEpisode],
    treatment: &str,
    baseline: &str,
    receiver_action: bool,
) -> Value {
    let values = episodes
        .iter()
        .filter_map(|episode| {
            let treatment = episode
                .variants
                .iter()
                .find(|variant| variant.intervention == treatment)?;
            let baseline = episode
                .variants
                .iter()
                .find(|variant| variant.intervention == baseline)?;
            let pair = if receiver_action {
                (
                    treatment.receiver_target_action,
                    baseline.receiver_target_action,
                )
            } else {
                (treatment.task_value, baseline.task_value)
            };
            Some(f64::from(pair.0) - f64::from(pair.1))
        })
        .collect::<Vec<_>>();
    mean_interval(&values)
}

#[derive(Debug, Serialize)]
pub struct NegativeControlReport {
    schema_version: u32,
    controls: Vec<NegativeControl>,
    all_invalid_variants_rejected: bool,
}

#[derive(Debug, Serialize)]
struct NegativeControl {
    id: &'static str,
    rejected: bool,
    observed_error: String,
}

pub fn negative_controls(
    corpus: &LoadedCorpus,
    manifest: &FrozenStudyManifest,
    records: &StudyRecords,
    _digest_path: &Path,
) -> Result<NegativeControlReport> {
    const IDS: [&str; 16] = [
        "unequal_arm_budget",
        "early_selector_disclosure",
        "inconsistent_post_randomization_exclusion",
        "outcome_substitution",
        "arm_runtime_profile_mismatch",
        "arm_model_route_mismatch",
        "early_protected_result_disclosure",
        "arbitrary_block_seed",
        "reversed_assignment_order",
        "arm_assignment_mismatch",
        "substituted_corpus_root",
        "frozen_budget_mutation",
        "frozen_exclusion_rule_mutation",
        "frozen_primary_outcome_mutation",
        "frozen_stopping_rule_mutation",
        "frozen_manifest_identity_mutation",
    ];
    let controls = IDS
        .into_iter()
        .map(|id| record_control(id, run_negative_control(corpus, manifest, records, id)))
        .collect::<Vec<_>>();
    let all_invalid_variants_rejected = controls.iter().all(|control| control.rejected);
    ensure!(
        all_invalid_variants_rejected,
        "one or more study negative controls produced a false successful result"
    );
    Ok(NegativeControlReport {
        schema_version: 1,
        controls,
        all_invalid_variants_rejected,
    })
}

pub fn run_negative_control(
    corpus: &LoadedCorpus,
    manifest: &FrozenStudyManifest,
    records: &StudyRecords,
    id: &str,
) -> Result<()> {
    match id {
        "unequal_arm_budget" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0]
                .budget_opportunity
                .model_calls += 1;
            validate_records(corpus, manifest, &changed)
        }
        "early_selector_disclosure" => {
            let mut changed = records.clone();
            let selector = changed.primary_blocks[0]
                .arms
                .iter_mut()
                .find(|arm| arm.condition == "independent_best_of_n")
                .and_then(|arm| arm.blinded_selector.as_mut())
                .context("synthetic fixture has no blinded selector")?;
            selector.producer_identity_reveal_sequence = selector.assessment_commit_sequence;
            validate_records(corpus, manifest, &changed)
        }
        "inconsistent_post_randomization_exclusion" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0].excluded = true;
            changed.primary_blocks[0].arms[0].exclusion_reason = Some("post_outcome".to_owned());
            validate_records(corpus, manifest, &changed)
        }
        "outcome_substitution" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0].terminal_outcome = "accepted".to_owned();
            changed.primary_blocks[0].arms[0].verifier_decision = "failed".to_owned();
            changed.primary_blocks[0].arms[0].candidate_sha256 = Some("a".repeat(64));
            changed.primary_blocks[0].arms[0].audit_false_acceptance = Some(false);
            validate_records(corpus, manifest, &changed)
        }
        "arm_runtime_profile_mismatch" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0].runtime_profile_sha256 = "f".repeat(64);
            validate_records(corpus, manifest, &changed)
        }
        "arm_model_route_mismatch" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0].accounting.routes[0].route_sha256 = "f".repeat(64);
            validate_records(corpus, manifest, &changed)
        }
        "early_protected_result_disclosure" => {
            let mut changed = records.clone();
            let arm = &mut changed.primary_blocks[0].arms[0];
            arm.protected_result_reveal_sequence = Some(arm.selection_commit_sequence);
            validate_records(corpus, manifest, &changed)
        }
        "arbitrary_block_seed" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].seed = changed.primary_blocks[0].seed.wrapping_add(1);
            validate_records(corpus, manifest, &changed)
        }
        "reversed_assignment_order" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].assignment_order.reverse();
            validate_records(corpus, manifest, &changed)
        }
        "arm_assignment_mismatch" => {
            let mut changed = records.clone();
            changed.primary_blocks[0].arms[0].assignment_position =
                (changed.primary_blocks[0].arms[0].assignment_position + 1) % 3;
            validate_records(corpus, manifest, &changed)
        }
        "substituted_corpus_root" => validate_corpus_root_sha256(&"f".repeat(64)),
        "frozen_budget_mutation" => frozen_manifest_mutation(
            manifest,
            "/resources/equal_arm_budget/model_calls",
            json!(61),
        ),
        "frozen_exclusion_rule_mutation" => frozen_manifest_mutation(
            manifest,
            "/exclusions/post_randomization",
            json!("arm_specific_if_infrastructure"),
        ),
        "frozen_primary_outcome_mutation" => {
            frozen_manifest_mutation(manifest, "/outcomes/primary", json!("selector_score"))
        }
        "frozen_stopping_rule_mutation" => frozen_manifest_mutation(
            manifest,
            "/stopping/study_rule",
            json!("stop_when_positive"),
        ),
        "frozen_manifest_identity_mutation" => {
            frozen_manifest_mutation(manifest, "/study_id", json!("post-outcome-v2"))
        }
        _ => bail!("unknown study negative control: {id}"),
    }
}

fn frozen_manifest_mutation(
    manifest: &FrozenStudyManifest,
    pointer: &str,
    replacement: Value,
) -> Result<()> {
    let mut value = manifest.value.clone();
    *value
        .pointer_mut(pointer)
        .with_context(|| format!("missing negative-control pointer {pointer}"))? = replacement;
    let bytes = serde_json::to_vec_pretty(&value)?;
    let rewritten_sidecar = sha256_bytes(&bytes);
    verify_frozen_manifest_bytes(&bytes, &rewritten_sidecar)
}

fn record_control(id: &'static str, result: Result<()>) -> NegativeControl {
    match result {
        Ok(()) => NegativeControl {
            id,
            rejected: false,
            observed_error: "invalid variant was accepted".to_owned(),
        },
        Err(error) => NegativeControl {
            id,
            rejected: true,
            observed_error: format!("{error:#}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        APPROVED_INITIAL_CORPUS_ROOT, BudgetVector, FROZEN_STUDY_MANIFEST_SHA256,
        load_frozen_manifest, load_study_records, negative_controls, power_analysis,
    };

    fn corpus_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus")
    }

    fn study_path(name: &str) -> PathBuf {
        corpus_root().join("study").join(name)
    }

    #[test]
    fn frozen_manifest_and_power_are_reproducible() {
        let corpus = super::super::load_corpus(&corpus_root()).unwrap();
        let manifest = load_frozen_manifest(
            &corpus,
            &study_path("manifest-v1.json"),
            &study_path("manifest-v1.sha256"),
        )
        .unwrap();
        let power = power_analysis(&manifest).unwrap();
        assert_eq!(power.reliability_required_distinct_tasks_per_stratum, 1889);
        assert_eq!(
            power.communication_required_distinct_tasks_per_stratum,
            1992
        );
        assert_eq!(power.corpus_required_distinct_tasks_per_stratum, 1992);
        assert_eq!(power.stochastic_repetitions_per_task, 5);
        assert_eq!(power.independent_best_of_n, 11);
        assert_eq!(power.arm_attempt_quanta, 12);
        assert_eq!(power.calls_per_attempt_quantum, 5);
        assert_eq!(manifest.digest, FROZEN_STUDY_MANIFEST_SHA256);
        assert_eq!(
            manifest
                .string("/corpus/approved_initial_root_sha256")
                .unwrap(),
            APPROVED_INITIAL_CORPUS_ROOT
        );
        assert_eq!(BudgetVector::from_power(&power).model_calls, 60);
    }

    #[test]
    fn dry_run_and_all_negative_controls_are_discriminating() {
        let corpus = super::super::load_corpus(&corpus_root()).unwrap();
        let digest = study_path("manifest-v1.sha256");
        let manifest =
            load_frozen_manifest(&corpus, &study_path("manifest-v1.json"), &digest).unwrap();
        let records = load_study_records(&study_path("synthetic-records-v1.json")).unwrap();
        super::analyze_study_records(&corpus, &manifest, &records).unwrap();
        let controls = negative_controls(&corpus, &manifest, &records, &digest).unwrap();
        assert!(controls.all_invalid_variants_rejected);
        assert_eq!(controls.controls.len(), 16);
    }
}

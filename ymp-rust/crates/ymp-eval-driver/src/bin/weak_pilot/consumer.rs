//! Frozen-manifest consumer shared by native and explicitly synthetic runs.
use super::{consumer_engine, raw::Group, write_json};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{process::Command, sync::mpsc, time::Instant};
use ymp_core::*;
use ymp_providers::{
    ExecutionBackend, ExecutionFuture, NativeExecutionBackend, ProviderEvent, TurnRequest,
};

const CONDITIONS: [&str; 6] = [
    "strong-solo",
    "weak-solo",
    "independent-2",
    "independent-3",
    "cooperation-2",
    "cooperation-3",
];

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub id: String,
    pub condition: String,
    pub task: String,
    pub variant: String,
    pub blind_id: String,
    #[serde(default)]
    pub fixture_fault: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub execution_kind: String,
    pub phase: String,
    pub source_revision: String,
    pub repository: PathBuf,
    pub output: PathBuf,
    pub workspace_root: PathBuf,
    pub runner_sha256: String,
    pub python: PathBuf,
    pub codex: PathBuf,
    pub codex_sha256: String,
    pub native_home: PathBuf,
    pub protected_roots: Vec<PathBuf>,
    pub controls: BTreeMap<String, Value>,
    pub control_evidence: PathBuf,
    pub control_evidence_sha256: String,
    pub config: Config,
    pub catalog: ProviderCapabilities,
    pub weak_model: String,
    pub strong_model: String,
    pub max_invocations: u64,
    pub group_seconds: u64,
    pub seed: u64,
    pub task_prompt: String,
    pub frozen_files: BTreeMap<String, String>,
    pub prerequisite_manifest_sha256: Option<String>,
    pub attempts: Vec<Attempt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    schema_version: u32,
    authority: String,
    reference: String,
    approved_at: String,
    manifest_sha256: Vec<String>,
}

fn file_hash(path: &Path) -> Result<String> {
    Ok(bytes_digest(&std::fs::read(path)?))
}

impl Manifest {
    fn validate(&self, scripted: bool) -> Result<()> {
        ensure!(
            self.schema_version == 2,
            "Unsupported consumer manifest version"
        );
        ensure!(
            self.execution_kind
                == if scripted {
                    "protocol-fixture"
                } else {
                    "native"
                },
            "Execution kind cannot cross the fixture/native boundary"
        );
        ensure!(
            self.repository.is_absolute()
                && self.output.is_absolute()
                && self.workspace_root.is_absolute()
                && self.python.is_absolute()
                && self.codex.is_absolute(),
            "Manifest paths must be absolute"
        );
        ensure!(
            !self.protected_roots.is_empty()
                && self.protected_roots.iter().all(|p| p.is_absolute()),
            "Freeze the private user-data and prior-study roots"
        );
        ensure!(
            !self.workspace_root.exists()
                && self.workspace_root.parent().is_some_and(|p| p.is_dir())
                && !self.workspace_root.starts_with(&self.output)
                && !self.output.starts_with(&self.workspace_root),
            "Private controller and fresh solving roots must be disjoint"
        );
        ensure!(
            self.runner_sha256 == file_hash(&std::env::current_exe()?)?,
            "Frozen runner digest changed"
        );
        ensure!(
            self.codex_sha256 == file_hash(&self.codex)?,
            "Frozen native/fixture executable changed"
        );
        if scripted {
            ensure!(
                self.codex.canonicalize()?
                    == self
                        .repository
                        .join("ymp-evals/weak-pilot/codex_protocol_v2.py")
                        .canonicalize()?,
                "Scripted mode can execute only the reviewed local protocol fixture"
            );
        } else {
            let accepted = std::process::Command::new("git")
                .args([
                    "merge-base",
                    "--is-ancestor",
                    "1c17f4e",
                    &self.source_revision,
                ])
                .current_dir(&self.repository)
                .status()?;
            ensure!(
                accepted.success(),
                "The accepted P0 restoration base 1c17f4e must precede native measurement"
            );
        }
        ensure!(
            !self.output.exists(),
            "Output must be fresh; no automatic resume or overwrite"
        );
        ensure!(
            self.output.parent().is_some_and(|p| p.is_dir()),
            "Output parent must exist"
        );
        ensure!(
            !self
                .output
                .parent()
                .unwrap()
                .canonicalize()?
                .ancestors()
                .any(|p| p.join(".git").exists()),
            "Run output must be outside Git"
        );
        ensure!(
            !self
                .output
                .components()
                .any(|p| matches!(p.as_os_str().to_str(), Some(".ymp2" | "_ymp3" | "3223c6a9"))),
            "Live application data is forbidden"
        );
        let current = std::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", &self.source_revision, "HEAD"])
            .current_dir(&self.repository)
            .status()?;
        ensure!(
            current.success(),
            "Manifest source revision is not part of the checkout history"
        );
        let product = std::process::Command::new("git")
            .args([
                "diff",
                "--quiet",
                "1c17f4e",
                "--",
                "Cargo.toml",
                "Cargo.lock",
                "ymp-bridges",
                "ymp-rust/crates/ymp-core",
                "ymp-rust/crates/ymp-storage",
                "ymp-rust/crates/ymp-providers",
                "ymp-rust/crates/ymp-runtime",
                "ymp-rust/crates/ymp-cli",
                "ymp-rust/crates/ymp-tui",
                "ymp-rust/crates/ymp-workspace",
            ])
            .current_dir(&self.repository)
            .status()?;
        ensure!(
            product.success(),
            "Product bytes differ from accepted P0 base 1c17f4e"
        );
        for (relative, expected) in &self.frozen_files {
            let path = Path::new(relative);
            ensure!(
                !path.is_absolute()
                    && !path
                        .components()
                        .any(|p| matches!(p, std::path::Component::ParentDir)),
                "Invalid frozen source path"
            );
            ensure!(
                file_hash(&self.repository.join(path))? == *expected,
                "Frozen source changed: {relative}"
            );
        }
        for area in [
            "ymp-evals/weak-pilot",
            "ymp-evals/validators",
            "ymp-rust/crates/ymp-eval-driver/src/bin",
        ] {
            for entry in walkdir::WalkDir::new(self.repository.join(area))
                .into_iter()
                .filter_entry(|e| e.file_name() != "__pycache__")
            {
                let entry = entry?;
                if entry.file_type().is_file() {
                    ensure!(
                        self.frozen_files.contains_key(
                            &entry
                                .path()
                                .strip_prefix(&self.repository)?
                                .to_string_lossy()
                                .to_string()
                        ),
                        "Manifest omits a consumer or fixture file"
                    );
                }
            }
        }
        ensure!(
            self.max_invocations > 0
                && self.max_invocations <= 32
                && (1..=1200).contains(&self.group_seconds),
            "Missing or excessive explicit group limits"
        );
        self.config.validate()?;
        self.catalog.validate()?;
        ensure!(
            self.config.limits.attempts == 1 && self.config.limits.parallel == 2,
            "Use the frozen one-attempt, two-slot scheduling policy"
        );
        ensure!(
            self.config.limits.turns as u64 == self.max_invocations,
            "Config and external call ceilings differ"
        );
        let limits = self
            .config
            .limits
            .resources
            .as_ref()
            .context("Missing resources")?;
        ensure!(
            limits.unknown_usage == UnknownUsagePolicy::Stop
                && limits.observed_tokens.is_some()
                && limits.invocation_tokens.is_some(),
            "Complete observed-resource admission is required"
        );
        ensure!(
            self.config
                .acceptance_contracts
                .as_ref()
                .is_none_or(Vec::is_empty),
            "Do not install hidden scoring as in-run acceptance"
        );
        ensure!(
            self.config.providers.len() == 1
                && self.config.providers[0].kind == ProviderKind::Codex
                && self.config.providers[0].command == self.codex.to_string_lossy()
                && self.config.providers[0].args.is_empty()
                && self.config.providers[0].env_refs.is_empty(),
            "Unexpected provider configuration"
        );
        ensure!(
            self.attempts.len() <= 12 && !self.attempts.is_empty(),
            "The small consumer supports at most twelve outcomes"
        );
        let mut ids = std::collections::BTreeSet::new();
        let mut blind = std::collections::BTreeSet::new();
        let mut cells = std::collections::BTreeSet::new();
        for attempt in &self.attempts {
            ensure!(
                scripted || attempt.fixture_fault.is_none(),
                "Native manifests cannot contain fixture behavior"
            );
            ensure!(
                CONDITIONS.contains(&attempt.condition.as_str())
                    && ["reconcile", "repair"].contains(&attempt.task.as_str())
                    && ["preparation", "measured"].contains(&attempt.variant.as_str()),
                "Unknown task, variant or condition"
            );
            ensure!(
                attempt
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
                    && !attempt.id.is_empty()
                    && ids.insert(&attempt.id),
                "Invalid or duplicate attempt id"
            );
            ensure!(
                attempt.blind_id.len() == 32
                    && attempt
                        .blind_id
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                    && blind.insert(&attempt.blind_id),
                "Invalid or repeated blind identifier"
            );
            ensure!(
                self.phase != "preparation-calibration" || attempt.variant == "preparation",
                "Calibration cannot consume measured variants"
            );
            ensure!(
                cells.insert((&attempt.task, &attempt.condition)),
                "Repeated task/condition is not part of this frozen pilot"
            );
        }
        ensure!(
            self.phase != "measured-pilot"
                || self.attempts.len() == 12 && self.prerequisite_manifest_sha256.is_some(),
            "Measured pilot requires all twelve cells and its calibration prerequisite"
        );
        for model in [&self.weak_model, &self.strong_model] {
            ensure!(
                self.catalog.models.iter().any(|m| &m.id == model
                    && m.controls
                        .as_ref()
                        .is_some_and(|controls| controls.iter().any(|c| c.id == "effort"
                            && c.values.contains(&NativeControlValue::Choice("low".into()))))),
                "Frozen catalog does not advertise model {model} with low"
            );
            ensure!(
                scripted == model.starts_with("fixture-"),
                "Fixture models cannot masquerade as native measurements"
            );
        }
        for (id, expected_model) in [
            ("strong-1", &self.strong_model),
            ("weak-1", &self.weak_model),
            ("weak-2", &self.weak_model),
            ("weak-3", &self.weak_model),
        ] {
            let agent = self.config.agent(id)?;
            let execution = self
                .config
                .execution
                .get(id)
                .context("Missing execution pin")?;
            ensure!(
                agent.model.as_ref() == Some(expected_model)
                    && agent.name == *expected_model
                    && execution.fixed.model.as_ref() == Some(expected_model)
                    && execution.fixed.effort.as_deref() == Some("low"),
                "Frozen profile or low pin is incorrect"
            );
        }
        for key in [
            "features.multi_agent",
            "features.multi_agent_v2",
            "agents.enabled",
            "features.memories",
            "memories.generate_memories",
            "memories.use_memories",
        ] {
            ensure!(
                self.controls.get(key) == Some(&json!(false)),
                "Missing enforced control {key}"
            );
        }
        ensure!(
            self.controls.get("project_doc_max_bytes") == Some(&json!(0)),
            "Disable inherited project instructions"
        );
        if !scripted {
            ensure!(
                self.control_evidence.is_absolute()
                    && file_hash(&self.control_evidence)? == self.control_evidence_sha256,
                "Native control evidence is missing or changed"
            );
            let evidence: Value = serde_json::from_slice(&std::fs::read(&self.control_evidence)?)?;
            ensure!(
                evidence["inference_calls"] == 0
                    && evidence["pass"] == true
                    && evidence["binary_sha256"] == self.codex_sha256,
                "Native configuration/enforcement probe did not pass for this executable"
            );
        }
        Ok(())
    }
}

/// Model participants never receive this operator-supplied authorization record.
fn authorize(
    manifest: &Manifest,
    hash: &str,
    approval: Option<&Path>,
    prerequisite: Option<&Path>,
) -> Result<()> {
    let path = approval.context("owner_approval_required: no measured launch is authorized")?;
    ensure!(
        path.is_absolute()
            && !path.starts_with(&manifest.output)
            && !path.starts_with(&manifest.workspace_root),
        "Approval must be supplied outside model workspaces"
    );
    let record: Approval = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        record.schema_version == 1
            && record.authority == "owner"
            && !record.reference.trim().is_empty()
            && !record.approved_at.is_empty()
            && record.manifest_sha256.iter().any(|h| h == hash),
        "Owner approval does not cover the exact frozen manifest"
    );
    if let Some(expected) = &manifest.prerequisite_manifest_sha256 {
        let report: Value = serde_json::from_slice(&std::fs::read(
            prerequisite.context("Calibration report is required before pilot")?,
        )?)?;
        ensure!(
            report["manifest_sha256"] == *expected
                && report["execution_kind"] == "native"
                && report["complete"] == true
                && report["calibration_allows_pilot"] == true,
            "Calibration did not establish the predeclared continuation conditions"
        );
    }
    Ok(())
}

pub async fn run(
    manifest_path: &Path,
    scripted: bool,
    approval: Option<&Path>,
    prerequisite: Option<&Path>,
) -> Result<Value> {
    let bytes = std::fs::read(manifest_path)?;
    let hash = bytes_digest(&bytes);
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    // Authorization is external, never a model-authored field or a fixture flag.
    if !scripted {
        authorize(&manifest, &hash, approval, prerequisite)?;
    }
    manifest.validate(scripted)?;
    std::fs::create_dir(&manifest.output)?;
    std::fs::create_dir(&manifest.workspace_root)?;
    write_json(&manifest.output.join("frozen-manifest.json"), &manifest)?;
    let mut outcomes = Vec::new();
    let mut interrupted = false;
    for attempt in &manifest.attempts {
        let started = Instant::now();
        let result = if interrupted {
            Ok(
                json!({"attempt_id":attempt.id,"condition":attempt.condition,"status":"not_started","reason":"prior_condition_interruption","interpretable":false,"objective_success":null,"runtime":null,"usage":null,"elapsed_seconds":null}),
            )
        } else {
            execute(&manifest, attempt, scripted).await
        };
        let outcome = match result {
            Ok(value) => value,
            Err(error) => {
                let relative = if attempt.condition.starts_with("cooperation") {
                    "summary.json"
                } else {
                    "group/summary.json"
                };
                let runtime = std::fs::read(manifest.output.join(&attempt.id).join(relative))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
                json!({"attempt_id":attempt.id,"condition":attempt.condition,"status":"failed","error":format!("{error:#}"),"objective_success":false,"interpretable":false,"runtime":runtime,"elapsed_seconds":started.elapsed().as_secs_f64()})
            }
        };
        write_json(
            &manifest.output.join(format!("{}.result.json", attempt.id)),
            &outcome,
        )?;
        interrupted |= outcome["interpretable"] != true;
        outcomes.push(outcome);
        let report = json!({"schema_version":2,"execution_kind":manifest.execution_kind,"manifest_sha256":hash,"source_revision":manifest.source_revision,"phase":manifest.phase,"native_measurements":!scripted,"complete":outcomes.len()==manifest.attempts.len() && !interrupted,"calibration_allows_pilot":outcomes.len()==manifest.attempts.len() && outcomes.iter().all(|row| row["interpretable"]==true && row["objective_success"]==true),"outcomes":outcomes});
        write_json(&manifest.output.join("run.json"), &report)?;
    }
    Ok(serde_json::from_slice(&std::fs::read(
        manifest.output.join("run.json"),
    )?)?)
}

async fn python(
    manifest: &Manifest,
    script: &str,
    args: &[String],
    deadline: Instant,
) -> Result<Value> {
    let mut command = Command::new(&manifest.python);
    command
        .arg("-B")
        .arg(
            manifest
                .repository
                .join("ymp-evals/weak-pilot")
                .join(script),
        )
        .args(args)
        .kill_on_drop(true);
    let result = tokio::time::timeout_at(deadline, command.output())
        .await
        .context("condition_deadline: external operation stopped")??;
    let value: Value = serde_json::from_slice(&result.stdout)
        .with_context(|| format!("{script}: {}", String::from_utf8_lossy(&result.stderr)))?;
    ensure!(
        result.status.success()
            || script == "observer.py"
                && args.first().is_some_and(|a| a == "score")
                && value.get("objective_success").is_some(),
        "{script} failed: {value}"
    );
    Ok(value)
}

async fn stage(
    manifest: &Manifest,
    attempt: &Attempt,
    work: &Path,
    deadline: Instant,
) -> Result<()> {
    python(
        manifest,
        "observer.py",
        &[
            "stage".into(),
            "--task".into(),
            attempt.task.clone(),
            "--variant".into(),
            attempt.variant.clone(),
            "--destination".into(),
            work.to_string_lossy().into_owned(),
        ],
        deadline,
    )
    .await?;
    Ok(())
}

fn config(
    manifest: &Manifest,
    attempt: &Attempt,
    directory: &Path,
    scripted: bool,
) -> Result<Config> {
    let count = if attempt.condition.ends_with("-3") {
        3
    } else if attempt.condition.ends_with("-2") {
        2
    } else {
        1
    };
    let strong = attempt.condition == "strong-solo";
    let ids = (1..=count)
        .map(|n| format!("{}-{n}", if strong { "strong" } else { "weak" }))
        .collect::<Vec<_>>();
    let mut config = manifest.config.clone();
    config.agents.retain(|a| ids.contains(&a.id));
    config.execution.retain(|id, _| ids.contains(id));
    config.team = ids.clone();
    config.team_constraints = TeamConstraints {
        fixed_size: Some(count),
        fixed_roster: Some(ids.clone()),
        eligible_agents: Some(ids),
        max_members: count,
    };
    config.limits.turns = manifest.max_invocations as usize
        + if attempt.condition.starts_with("cooperation") {
            0
        } else {
            1
        };
    if !attempt.condition.starts_with("cooperation") {
        // Keep the mandatory storage reserve without inventing a solo review.
        config
            .limits
            .resources
            .as_mut()
            .unwrap()
            .review_reserve_tokens = Some(1);
    }
    let mut denied = vec![
        manifest.repository.clone(),
        manifest.output.clone(),
        manifest.native_home.clone(),
    ];
    denied.extend(manifest.protected_roots.iter().cloned());
    for row in &manifest.attempts {
        let parent = manifest.workspace_root.join(&row.id);
        if row.condition.starts_with("cooperation") {
            denied.push(parent);
        } else {
            let members = if row.condition.ends_with("-3") {
                3
            } else if row.condition.ends_with("-2") {
                2
            } else {
                1
            };
            for ordinal in 1..=members {
                denied.push(parent.join(format!("candidate-{ordinal}")));
            }
        }
    }
    let policy = json!({"schema_version":2,"controls":manifest.controls,"deny_paths":denied,"native_home":manifest.native_home});
    write_json(&directory.join("native-policy.json"), &policy)?;
    let provider = &mut config.providers[0];
    provider.command = manifest.python.to_string_lossy().into_owned();
    provider.args = vec![
        manifest
            .repository
            .join("ymp-evals/weak-pilot/codex_envelope.py")
            .to_string_lossy()
            .into_owned(),
        "--policy".into(),
        directory
            .join("native-policy.json")
            .to_string_lossy()
            .into_owned(),
        "--codex".into(),
        manifest.codex.to_string_lossy().into_owned(),
        "--journal".into(),
        directory
            .join("envelope.jsonl")
            .to_string_lossy()
            .into_owned(),
    ];
    if scripted {
        let scenario = json!({"execution_kind":"protocol-fixture","task":attempt.task,"variant":attempt.variant,"cooperation_tasks":if count==3 {2}else{1},"fixture_root":manifest.repository.join("ymp-evals/weak-pilot/fixtures"),"request_log":directory.join("protocol-requests.jsonl"),"fault":attempt.fixture_fault});
        write_json(&directory.join("protocol-scenario.json"), &scenario)?;
        provider.args.extend([
            "--codex-arg=--scenario".into(),
            format!(
                "--codex-arg={}",
                directory.join("protocol-scenario.json").display()
            ),
        ]);
    }
    config
        .capabilities
        .insert(provider.id.clone(), manifest.catalog.clone());
    let observed_at = match &manifest.catalog.source {
        CapabilitySource::NativeMetadata { observed_at, .. } => observed_at.clone(),
        _ => bail_catalog()?,
    };
    config.native_catalog.providers.insert(
        provider.id.clone(),
        NativeProviderSnapshot {
            provider_fingerprint: provider_fingerprint(provider),
            last_attempt: observed_at,
            failure: None,
            catalog: Some(manifest.catalog.clone()),
        },
    );
    config.validate()?;
    write_json(&directory.join("captured-config.json"), &config)?;
    config.native_catalog.save(directory)?;
    Ok(config)
}

async fn execute(manifest: &Manifest, attempt: &Attempt, scripted: bool) -> Result<Value> {
    let directory = manifest.output.join(&attempt.id);
    std::fs::create_dir(&directory)?;
    let config = config(manifest, attempt, &directory, scripted)?;
    let journal = Arc::new(Journal::new(
        directory.join("provider-events.jsonl"),
        scripted,
    ));
    let started = Instant::now();
    let deadline = started + Duration::from_secs(manifest.group_seconds);
    let mut candidates = Vec::new();
    let runtime;
    let mut errors = Vec::new();
    if attempt.condition.starts_with("cooperation") {
        let work = manifest.workspace_root.join(&attempt.id);
        stage(manifest, attempt, &work, deadline).await?;
        runtime = consumer_engine::run_with_work(
            &directory,
            &work,
            config,
            &manifest.task_prompt,
            journal.clone(),
            deadline,
        )
        .await?;
        candidates.push(work);
    } else {
        let group = Group::configured_with_work(
            &directory.join("group"),
            &manifest.workspace_root.join(&attempt.id),
            config.clone(),
            manifest.max_invocations,
            deadline,
            &manifest.task_prompt,
            &manifest.execution_kind,
        )?;
        for index in 0..config.agents.len() {
            let work = group.workdir().join(format!("candidate-{}", index + 1));
            stage(manifest, attempt, &work, deadline).await?;
            candidates.push(work);
        }
        for (profile, work) in config.agents.iter().zip(&candidates) {
            let resources = config.limits.resources.as_ref().unwrap();
            let request = TurnRequest {
                profile: profile.clone(),
                provider: config.providers[0].clone(),
                settings: ExecutionSettings {
                    permission_mode: Some("write".into()),
                    ..config.execution_settings(profile, &ModelEffort::default())?
                },
                cwd: work.clone(),
                prompt: manifest.task_prompt.clone(),
                purpose: "pilot_candidate".into(),
                read_only: false,
                resume: None,
                usage_baseline: None,
                mcp: None,
                resource_controls: NativeResourceControls {
                    max_turns: Some(resources.native_max_turns),
                    max_output_chars: Some(resources.max_output_chars),
                },
                timeout_secs: config.limits.turn_timeout_secs,
                bridge: PathBuf::new(),
            };
            if let Err(error) = group.invoke(journal.as_ref(), request).await {
                errors.push(format!("{error:#}"));
                break;
            }
            if group
                .store
                .session_usage(&group.session.id)?
                .total
                .is_partial()
            {
                errors.push("partial_or_unknown_usage".into());
                break;
            }
        }
        runtime = group.save_trace()?;
    }
    let selection = if candidates.len() > 1 {
        let mut args = vec![
            "--task".into(),
            attempt.task.clone(),
            "--public-check".into(),
            manifest
                .repository
                .join("ymp-evals/weak-pilot/fixtures")
                .join(&attempt.variant)
                .join(&attempt.task)
                .join("visible/public_test.py")
                .to_string_lossy()
                .into_owned(),
        ];
        args.extend(candidates.iter().map(|p| p.to_string_lossy().into_owned()));
        python(manifest, "public_select.py", &args, deadline).await?
    } else {
        json!({"selected_participant_ordinal":1,"selection_rule":"only-artifact"})
    };
    let selected = selection["selected_participant_ordinal"]
        .as_u64()
        .context("Missing selection")? as usize;
    let submission = manifest
        .output
        .join(format!("submission-{}", attempt.blind_id));
    let seal = python(
        manifest,
        "observer.py",
        &[
            "seal".into(),
            "--task".into(),
            attempt.task.clone(),
            "--variant".into(),
            attempt.variant.clone(),
            "--workdir".into(),
            candidates[selected - 1].to_string_lossy().into_owned(),
            "--destination".into(),
            submission.to_string_lossy().into_owned(),
            "--blind-id".into(),
            attempt.blind_id.clone(),
        ],
        deadline,
    )
    .await?;
    let elapsed = started.elapsed().as_secs_f64();
    let score = python(
        manifest,
        "observer.py",
        &[
            "score".into(),
            "--submission".into(),
            submission.to_string_lossy().into_owned(),
        ],
        Instant::now() + Duration::from_secs(10),
    )
    .await?;
    let cooperation = attempt.condition.starts_with("cooperation");
    let interpretable = errors.is_empty()
        && elapsed < manifest.group_seconds as f64
        && if cooperation {
            runtime["status"] == "completed"
                && runtime["protocol_deviations"]
                    .as_array()
                    .is_some_and(Vec::is_empty)
        } else {
            runtime["accounting_closed"] == true
                && runtime["usage"]["total"]["partial_calls"].as_u64() == Some(0)
                && runtime["usage"]["total"]["reported"] == runtime["usage"]["total"]["calls"]
                && runtime["usage"]["total"]["calls"].as_u64() == Some(candidates.len() as u64)
        };
    Ok(
        json!({"schema_version":2,"execution_kind":manifest.execution_kind,"attempt_id":attempt.id,"condition":attempt.condition,"task":attempt.task,"variant":attempt.variant,"status":if interpretable {"completed"}else{"failed"},"interpretable":interpretable,"objective_success":score["objective_success"],"runtime":runtime,"selection":selection,"submission":seal,"external_score":score,"elapsed_seconds":elapsed,"errors":errors}),
    )
}
fn bail_catalog() -> Result<String> {
    anyhow::bail!("A captured catalog is required")
}

struct Journal {
    path: PathBuf,
    scripted: bool,
    write_lock: Mutex<()>,
}
impl Journal {
    fn new(path: PathBuf, scripted: bool) -> Self {
        Self {
            path,
            scripted,
            write_lock: Mutex::new(()),
        }
    }
    fn append(&self, value: &Value) -> Result<()> {
        use std::io::Write;
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Journal lock failed"))?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(value)?)?;
        Ok(())
    }
}
impl ExecutionBackend for Journal {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: if self.scripted {
                "ymp201.protocol-consumer"
            } else {
                "ymp201.native-consumer"
            }
            .into(),
            version: "2".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.read_only {
            WorkspaceAccess::ReadAll
        } else {
            WorkspaceAccess::WriteAll
        }
    }
    fn execute(
        &self,
        mut request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let stream = new_id();
            if let Some(index) = request
                .provider
                .args
                .iter()
                .position(|arg| arg == "--journal")
            {
                let base = PathBuf::from(
                    request
                        .provider
                        .args
                        .get(index + 1)
                        .context("Missing envelope journal path")?,
                );
                request.provider.args[index + 1] = base
                    .with_file_name(format!("envelope-{stream}.jsonl"))
                    .to_string_lossy()
                    .into_owned();
            }
            if let Some(index) = request
                .provider
                .args
                .iter()
                .position(|arg| arg == "--policy")
            {
                let base = PathBuf::from(
                    request
                        .provider
                        .args
                        .get(index + 1)
                        .context("Missing envelope policy path")?,
                );
                let mut policy: Value = serde_json::from_slice(&std::fs::read(&base)?)?;
                let cwd = request.cwd.canonicalize()?;
                policy["deny_paths"]
                    .as_array_mut()
                    .context("Missing policy deny paths")?
                    .retain(|p| p.as_str() != cwd.to_str());
                let path = base.with_file_name(format!("native-policy-{stream}.json"));
                write_json(&path, &policy)?;
                request.provider.args[index + 1] = path.to_string_lossy().into_owned();
            }
            self.append(&json!({"stream_id":stream,"kind":"request","agent_id":request.profile.id,"purpose":request.purpose,"cwd":request.cwd,"resume":request.resume,"settings":request.settings,"prompt":request.prompt,"mcp_present":request.mcp.is_some()}))?;
            if self.scripted {
                request.provider.args.extend([
                    "--codex-arg=--purpose".into(),
                    format!("--codex-arg={}", request.purpose),
                ]);
            }
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut future = NativeExecutionBackend.execute(request, tx);
            let outcome = loop {
                tokio::select! {result=&mut future=>break result,Some(event)=rx.recv()=>{self.append(&json!({"stream_id":stream,"event":event_json(&event)}))?;let _=events.send(event);}}
            };
            while let Ok(event) = rx.try_recv() {
                self.append(&json!({"stream_id":stream,"event":event_json(&event)}))?;
                let _ = events.send(event);
            }
            self.append(&json!({"stream_id":stream,"kind":"terminal","error":outcome.as_ref().err().map(|e|format!("{e:#}"))}))?;
            outcome
        })
    }
}
fn event_json(event: &ProviderEvent) -> Value {
    match event {
        ProviderEvent::Capabilities(v) => json!({"kind":"capabilities","value":v}),
        ProviderEvent::Execution(v) => json!({"kind":"execution","value":v}),
        ProviderEvent::Usage(v) => json!({"kind":"usage","value":v}),
        ProviderEvent::Delta(v) => json!({"kind":"delta","value":v}),
        ProviderEvent::Session(v) => json!({"kind":"session","value":v}),
        ProviderEvent::Tool(v) => json!({"kind":"tool","value":v}),
        ProviderEvent::Retry {
            session_id,
            turn_id,
            error_code,
        } => {
            json!({"kind":"retry","session_id":session_id,"turn_id":turn_id,"error_code":error_code})
        }
    }
}

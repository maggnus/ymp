//! Public runtime consumers. Every provider turn is scripted/mock and low effort.
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_providers::{
    ExecutionBackend, ExecutionFuture, NativeExecutionBackend, ProviderEvent, TurnRequest,
};
use ymp_runtime::*;
use ymp_storage::Store;

#[path = "knowledge_correction/board_integration.rs"]
mod board_integration;

struct Scripted {
    value: AtomicUsize,
    fail_plan: AtomicBool,
    requests: Mutex<Vec<TurnRequest>>,
}
impl ExecutionBackend for Scripted {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "test.observation-correction".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            ensure!(
                request.provider.kind == ProviderKind::Mock,
                "No native inference"
            );
            ensure!(
                request.settings.effort.as_deref() == Some("low"),
                "Tests require low effort"
            );
            self.requests.lock().unwrap().push(request.clone());
            if request.purpose == "plan" && self.fail_plan.load(Ordering::SeqCst) {
                bail!("Scripted pause after contract capture");
            }
            let purpose = request.purpose.clone();
            let directory = request.cwd.clone();
            let mut output = NativeExecutionBackend.execute(request, events).await?;
            if purpose == "plan" {
                let mut plan: Value = serde_json::from_str(&output.text)?;
                plan["tasks"][0]["title"] = json!("Record observation");
                plan["tasks"][0]["description"] = json!(
                    "Record O04, Hill, 2026-W36 completion percentage from the declared input"
                );
                output.text = plan.to_string();
            }
            if purpose == "execute" {
                let value = self.value.load(Ordering::SeqCst);
                std::fs::write(
                    directory.join(format!("claim-{value}.json")),
                    serde_json::to_vec(
                        &json!({"row":"O04", "site":"Hill", "week":"2026-W36", "value":value}),
                    )?,
                )?;
                output.text = format!("Recorded observation O04 at Hill: {value} percent");
            }
            Ok(output)
        })
    }
}

fn scope() -> BTreeMap<String, String> {
    [
        ("dataset", "observations"),
        ("site", "Hill"),
        ("week", "2026-W36"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
fn reference(entry: &MemoryEntry) -> KnowledgeRef {
    KnowledgeRef {
        id: entry.id.clone(),
        version: content_digest(&serde_json::to_string(entry).unwrap()),
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    directory: PathBuf,
    store: Store,
    engine: Engine,
    backend: Arc<Scripted>,
    verifier: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("project");
        std::fs::create_dir_all(directory.join("inputs")).unwrap();
        let directory = directory.canonicalize().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .to_owned();
        for name in ["observations.csv", "observations-corrected.csv"] {
            std::fs::copy(
                root.join("ymp-evals/fixtures/universal/inputs").join(name),
                directory.join("inputs").join(name),
            )
            .unwrap();
        }
        let verifier = temp.path().join("check_observation.py");
        std::fs::write(&verifier, "import csv,json,sys\nfrom pathlib import Path\nroot=Path(sys.argv[1])\nrows=list(csv.DictReader((root/sys.argv[2]).open()))\nrow=next(r for r in rows if r['row_id']=='O04')\nassert row['site']=='Hill' and row['week']=='2026-W36'\nexpected={'row':'O04','site':'Hill','week':'2026-W36','value':100*int(row['completed'])/int(row['scheduled'])}\nassert json.loads((root/sys.argv[3]).read_text())==expected\n").unwrap();
        let store = Store::open(&temp.path().join("state")).unwrap();
        let config: Config = serde_json::from_value(json!({
            "version":1, "knowledge_scope":scope(),
            "providers":[{"id":"mock","kind":"mock","command":"internal"}],
            "agents":[{"id":"a","name":"A","provider":"mock","instructions":"[mock:no-checks]"}, {"id":"b","name":"B","provider":"mock","instructions":"[mock:no-checks]"}],
            "team":["a","b"], "limits":{"attempts":1,"turns":40},
            "execution":{"a":{"fixed":{"effort":"low"}},"b":{"fixed":{"effort":"low"}}}
        })).unwrap();
        let backend = Arc::new(Scripted {
            value: AtomicUsize::new(95),
            fail_plan: AtomicBool::new(false),
            requests: Mutex::new(vec![]),
        });
        let (events, _) = mpsc::unbounded_channel();
        let engine = Engine::new(store.clone(), config, events, CancellationToken::new())
            .unwrap()
            .with_execution_backend(backend.clone())
            .unwrap();
        Self {
            _temp: temp,
            directory,
            store,
            engine,
            backend,
            verifier,
        }
    }
    fn contract(&self, value: usize, target: Option<&MemoryEntry>) -> AcceptanceContract {
        let input = if value == 95 {
            "inputs/observations.csv"
        } else {
            "inputs/observations-corrected.csv"
        };
        let artifact = format!("claim-{value}.json");
        AcceptanceContract {
            knowledge_correction: target.map(|old| KnowledgeCorrectionBinding {
                projection: KnowledgeProjection::ProjectOutcome, target: reference(old), applicability: scope(), criterion_ids: vec!["observation-value".into()],
                source_replacement: Some(KnowledgeSourceReplacement { previous_input: "inputs/observations.csv".into(), replacement_input: input.into() }),
            }),
            task_title: "Record observation".into(),
            criteria: vec![AcceptanceCriterion { id:"observation-value".into(), description:"The O04 claim for Hill in 2026-W36 matches the declared source; a bound correction replaces the previous claim only for this scope".into() }],
            artifacts:vec![artifact.clone().into()], inputs:vec![input.into()],
            checks:vec![TrustedCheck { id:"actual-observation".into(), criterion_ids:vec!["observation-value".into()], assertion:CheckAssertion::Command {
                program:"/usr/bin/python3".into(), args:vec![self.verifier.to_string_lossy().into(), "{workdir}".into(), input.into(), artifact], verifier_files:vec![self.verifier.clone()],
            }}],
        }
    }
    async fn run(&mut self, contract: AcceptanceContract) -> RunOutcome {
        self.engine.config.acceptance_contracts = Some(vec![contract]);
        self.engine
            .run(&self.directory, "Record the observation", None)
            .await
            .unwrap()
    }
    fn outcome_entry(&self, session: &str) -> MemoryEntry {
        self.store
            .memory_inventory(Some(&self.store.project(&self.directory).unwrap().id))
            .unwrap()
            .into_iter()
            .find(|e| e.source_session == session && e.kind == "outcome")
            .unwrap()
    }
    async fn first(&mut self) -> MemoryEntry {
        let outcome = self.run(self.contract(95, None)).await;
        assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
        let entry = self.outcome_entry(&outcome.session.id);
        assert_eq!(entry.status, "active");
        assert!(entry.content.contains("95"));
        entry
    }
    fn resolve(
        &self,
        entry: &MemoryEntry,
        scope: &BTreeMap<String, String>,
    ) -> Option<MemoryEntry> {
        self.store
            .resolve_memory(
                entry.project_id.as_deref(),
                &entry.id,
                None,
                scope,
                KnowledgeRetrievalMode::Supported,
            )
            .unwrap()
    }
}

#[tokio::test]
async fn cross_file_correction_reopens_retrieves_and_replays_without_duplicate_credit() {
    let mut f = Fixture::new();
    let old = f.first().await;
    assert!(f.resolve(&old, &scope()).is_some());
    let mut harbor = scope();
    harbor.insert("site".into(), "Harbor".into());
    assert!(f.resolve(&old, &harbor).is_none());
    f.backend.value.store(60, Ordering::SeqCst);
    f.backend.fail_plan.store(true, Ordering::SeqCst);
    let paused = f.run(f.contract(60, Some(&old))).await;
    assert_eq!(paused.session.status, "blocked");
    assert!(f.directory.join("inputs/observations.csv").is_file());
    assert!(f
        .directory
        .join("inputs/observations-corrected.csv")
        .is_file());
    assert!(f.resolve(&old, &scope()).is_none(), "Capturing the corrected file must invalidate the old source even while the original remains unchanged");
    let inspection = f
        .store
        .inspect_knowledge(old.project_id.as_deref(), &scope())
        .unwrap();
    assert_eq!(
        inspection
            .iter()
            .find(|r| r.entry.id == old.id)
            .unwrap()
            .availability,
        KnowledgeAvailability::SourceVersionChanged
    );
    assert_eq!(f.store.observations().unwrap().len(), 1);
    f.backend.fail_plan.store(false, Ordering::SeqCst);
    f.engine.config.acceptance_contracts = None;
    let outcome = f
        .engine
        .run(
            &f.directory,
            "Record the observation",
            Some(&paused.session.id),
        )
        .await
        .unwrap();
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let new = f.outcome_entry(&outcome.session.id);
    assert_eq!(new.supersedes.as_deref(), Some(old.id.as_str()));
    assert!(new.content.contains("60"));
    assert!(f.resolve(&new, &scope()).is_some());
    assert!(f.resolve(&new, &harbor).is_none());
    let reopened = Store::open(&f.store.home).unwrap();
    let history = reopened
        .inspect_knowledge(old.project_id.as_deref(), &scope())
        .unwrap();
    let historical = history.iter().find(|e| e.entry.id == old.id).unwrap();
    assert_eq!(historical.availability, KnowledgeAvailability::Superseded);
    assert_eq!(historical.replaced_by.as_deref(), Some(new.id.as_str()));
    assert_eq!(historical.entry.content, old.content);
    assert_eq!(historical.entry.provenance, old.provenance);
    let accepted = reopened
        .decisions(&outcome.session.id)
        .unwrap()
        .into_iter()
        .find(|d| d.kind == "task_accepted")
        .unwrap();
    let proposal = KnowledgeCorrectionProposal {
        target: reference(&old),
        acceptance_id: accepted.id.clone(),
    };
    assert!(matches!(
        reopened
            .commit_knowledge_correction(&proposal, &BoundKnowledgeCorrections.identity())
            .unwrap(),
        KnowledgeCorrectionOutcome::AlreadyApplied { .. }
    ));
    let observations = reopened.observations().unwrap();
    assert_eq!(observations.len(), 2);
    let observed = observations
        .iter()
        .find(|o| o.evidence == accepted.id)
        .unwrap();
    assert!(!reopened.observe_confirmed(observed, &accepted.id).unwrap());
    assert_eq!(reopened.observations().unwrap().len(), 2);
    assert_eq!(
        reopened
            .decisions(&outcome.session.id)
            .unwrap()
            .iter()
            .filter(|d| d.kind == "knowledge_superseded")
            .count(),
        1
    );
    // Re-retaining original acceptance cannot reactivate its superseded record.
    let source = old.provenance.as_ref().unwrap().source.as_ref().unwrap();
    assert_eq!(
        reopened
            .retain_knowledge(
                &source.acceptance_id,
                &KnowledgeProposal::ProjectOutcome,
                &scope(),
                &EvidenceKnowledgeProposals.identity()
            )
            .unwrap()
            .status,
        "superseded"
    );
    f.backend.fail_plan.store(true, Ordering::SeqCst);
    let later = f
        .engine
        .run(&f.directory, "Find observation", None)
        .await
        .unwrap();
    let trace = reopened.trace(&later.session.id).unwrap();
    let retrieval = trace
        .history
        .iter()
        .find(|e| e.kind == "memory_retrieval")
        .unwrap();
    assert!(retrieval.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == new.id));
    assert!(!retrieval.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == old.id));
    let requests = f.backend.requests.lock().unwrap();
    let last = requests.last().unwrap();
    assert!(last
        .prompt
        .contains("Captured artifact excerpt: {\"row\":\"O04\",\"site\":\"Hill\",\"value\":60"));
    assert!(!last.prompt.contains("\"value\":95"));
    assert!(requests
        .iter()
        .filter(|r| r.purpose == "review")
        .any(|r| r.prompt.contains(&old.id)
            && r.prompt.contains("source_replacement")
            && r.prompt
                .contains("historical context, not current evidence")
            && r.prompt.contains("\"value\":95")));
}

#[tokio::test]
async fn unrelated_confirmation_agreement_and_changed_authority_cannot_supersede() {
    let mut f = Fixture::new();
    let old = f.first().await;
    f.backend.value.store(60, Ordering::SeqCst);
    let unrelated = f.run(f.contract(60, None)).await;
    assert_eq!(unrelated.session.status, "completed");
    let acceptance = f
        .outcome_entry(&unrelated.session.id)
        .provenance
        .unwrap()
        .source
        .unwrap()
        .acceptance_id;
    let proposal = KnowledgeCorrectionProposal {
        target: reference(&old),
        acceptance_id: acceptance,
    };
    assert!(f
        .store
        .commit_knowledge_correction(&proposal, &BoundKnowledgeCorrections.identity())
        .unwrap_err()
        .to_string()
        .contains("no authority"));
    assert!(f.resolve(&old, &scope()).is_some());
    for defect in ["target", "scope", "input", "criterion"] {
        let mut contract = f.contract(60, Some(&old));
        let binding = contract.knowledge_correction.as_mut().unwrap();
        match defect {
            "target" => binding.target.version = "invented".into(),
            "scope" => {
                binding.applicability.insert("site".into(), "Harbor".into());
            }
            "input" => {
                binding.source_replacement.as_mut().unwrap().previous_input = "unrelated.csv".into()
            }
            _ => binding.criterion_ids = vec!["unrelated".into()],
        }
        f.engine.config.acceptance_contracts = Some(vec![contract]);
        let sessions = f.store.sessions(None).unwrap().len();
        assert!(
            f.engine
                .run(&f.directory, "Record observation", None)
                .await
                .is_err(),
            "{defect}"
        );
        assert_eq!(
            f.store.sessions(None).unwrap().len(),
            sessions,
            "Failed authority must roll back session capture"
        );
    }
    let mut agreement = f.contract(60, Some(&old));
    agreement.checks.clear();
    let result = f.run(agreement).await;
    assert_eq!(result.session.status, "completed");
    let candidate = f.outcome_entry(&result.session.id);
    assert_eq!(candidate.status, "proposed");
    assert!(candidate.supersedes.is_none());
    assert_eq!(
        f.store.observations().unwrap().len(),
        2,
        "Agreement produces no correction credit"
    );
    assert!(f
        .store
        .trace(&result.session.id)
        .unwrap()
        .history
        .iter()
        .any(|e| e.kind == "knowledge_correction_denied"));
    let mut self_review = f
        .store
        .decisions(&unrelated.session.id)
        .unwrap()
        .into_iter()
        .find(|d| d.kind == "candidate_review")
        .unwrap();
    let trace = f.store.trace(&unrelated.session.id).unwrap();
    let producer = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute")
        .unwrap();
    self_review.id = new_id();
    self_review.actor = Some(producer.agent_id.clone());
    self_review.links.assignment_id = Some(producer.id.clone());
    self_review.links.invocation_id = Some(
        trace
            .invocations
            .iter()
            .find(|i| i.assignment_id == producer.id)
            .unwrap()
            .id
            .clone(),
    );
    assert!(f
        .store
        .record_decision(&self_review)
        .unwrap_err()
        .to_string()
        .contains("Independent review excludes"));
}

struct NoCorrections;
impl KnowledgeCorrectionPolicy for NoCorrections {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "test.defer-correction".into(),
            version: "3".into(),
        }
    }
    fn propose(&self, _: KnowledgeCorrectionInput<'_>) -> Result<Vec<KnowledgeCorrectionProposal>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn unrelated_review_context_cannot_approve_the_correction_relation() {
    let mut f = Fixture::new();
    let old = f.first().await;
    f.backend.value.store(60, Ordering::SeqCst);
    f.engine.knowledge_corrections = Arc::new(NoCorrections);
    let outcome = f.run(f.contract(60, Some(&old))).await;
    let trace = f.store.trace(&outcome.session.id).unwrap();
    let review = trace
        .decisions
        .iter()
        .find(|d| d.kind == "candidate_review")
        .unwrap();
    let invocation = trace
        .invocations
        .iter()
        .find(|i| Some(&i.id) == review.links.invocation_id.as_ref())
        .unwrap();
    let assignment = trace
        .assignments
        .iter()
        .find(|a| a.id == invocation.assignment_id)
        .unwrap();
    for include_relation in [false, true] {
        let mut fresh = assignment.clone();
        fresh.id = new_id();
        fresh.state = InvocationState::Running;
        fresh.started_at = now();
        fresh.ended_at = None;
        fresh.grant_ids.clear();
        if !include_relation {
            fresh
                .context
                .retain(|c| c.kind != ContextKind::KnowledgeCorrection);
        }
        let mut call = invocation.clone();
        call.id = new_id();
        call.assignment_id = fresh.id.clone();
        call.state = InvocationState::Running;
        call.started_at = now();
        call.ended_at = None;
        call.usage = None;
        call.terminal_reason = None;
        let call = f.store.admit_invocation(&fresh, call).unwrap();
        f.store
            .finish_invocation(
                &outcome.session.id,
                &call.id,
                InvocationState::Completed,
                Some("Scripted independent inspection"),
            )
            .unwrap();
        let mut decision = review.clone();
        decision.id = new_id();
        decision.links.assignment_id = Some(fresh.id);
        decision.links.invocation_id = Some(call.id);
        let recorded = f.store.record_decision(&decision);
        if include_relation {
            recorded.unwrap();
        } else {
            assert!(recorded
                .unwrap_err()
                .to_string()
                .contains("did not include this exact correction relation"));
        }
    }
    assert!(f.outcome_entry(&outcome.session.id).supersedes.is_none());
}

struct MisboundCorrections;
impl KnowledgeCorrectionPolicy for MisboundCorrections {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "test.misbound-correction".into(),
            version: "1".into(),
        }
    }
    fn propose(
        &self,
        input: KnowledgeCorrectionInput<'_>,
    ) -> Result<Vec<KnowledgeCorrectionProposal>> {
        Ok(vec![KnowledgeCorrectionProposal {
            target: KnowledgeRef {
                id: "invented-target".into(),
                version: "invented-version".into(),
            },
            acceptance_id: input.acceptance.id.clone(),
        }])
    }
}

#[tokio::test]
async fn malicious_correction_policy_cannot_activate_an_uncommitted_replacement() {
    let mut f = Fixture::new();
    let old = f.first().await;
    f.backend.value.store(60, Ordering::SeqCst);
    f.engine.knowledge_corrections = Arc::new(MisboundCorrections);
    let outcome = f.run(f.contract(60, Some(&old))).await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let new = f.outcome_entry(&outcome.session.id);
    assert_eq!(new.status, "proposed");
    assert!(
        f.resolve(&new, &scope()).is_none(),
        "An accepted source alone must not activate a rejected correction proposal"
    );
    assert!(new.supersedes.is_none());
    assert!(f
        .store
        .trace(&outcome.session.id)
        .unwrap()
        .history
        .iter()
        .any(|e| e.kind == "knowledge_correction_denied"
            && e.data["implementation"]["id"] == "test.misbound-correction"));
}

#[tokio::test]
async fn policy_substitution_competing_stale_and_cyclic_corrections_remain_runtime_owned() {
    let mut f = Fixture::new();
    let old = f.first().await;
    f.backend.value.store(60, Ordering::SeqCst);
    f.engine.knowledge_corrections = Arc::new(NoCorrections);
    let one = f.run(f.contract(60, Some(&old))).await;
    assert_eq!(one.session.status, "completed", "{}", one.summary);
    let new = f.outcome_entry(&one.session.id);
    assert!(
        new.supersedes.is_none(),
        "The replacement policy must actually change behavior"
    );
    assert!(f
        .store
        .trace(&one.session.id)
        .unwrap()
        .history
        .iter()
        .any(|e| e.kind == "knowledge_correction_policy"
            && e.data["implementation"]["id"] == "test.defer-correction"));
    let two = f.run(f.contract(60, Some(&old))).await;
    let competing = f.outcome_entry(&two.session.id);
    let proposal = |entry: &MemoryEntry| KnowledgeCorrectionProposal {
        target: reference(&old),
        acceptance_id: entry
            .provenance
            .as_ref()
            .unwrap()
            .source
            .as_ref()
            .unwrap()
            .acceptance_id
            .clone(),
    };
    std::fs::write(f.directory.join("claim-60.json"), "stale").unwrap();
    assert!(f
        .store
        .commit_knowledge_correction(&proposal(&new), &BoundKnowledgeCorrections.identity())
        .is_err());
    let bytes = new.provenance.as_ref().unwrap().source.as_ref().unwrap();
    let saved = f
        .store
        .decisions(&one.session.id)
        .unwrap()
        .into_iter()
        .find(|d| d.id == bytes.acceptance_id)
        .unwrap();
    std::fs::write(
        f.directory.join("claim-60.json"),
        saved.links.result.unwrap().artifacts[0]
            .bytes
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    let mut forged = proposal(&new);
    forged.target = reference(&new);
    assert!(
        f.store
            .commit_knowledge_correction(&forged, &BoundKnowledgeCorrections.identity())
            .is_err(),
        "Policy cannot manufacture a self/cyclic target"
    );
    assert!(matches!(
        f.store
            .commit_knowledge_correction(&proposal(&new), &BoundKnowledgeCorrections.identity())
            .unwrap(),
        KnowledgeCorrectionOutcome::Applied { .. }
    ));
    assert!(
        f.store
            .commit_knowledge_correction(
                &proposal(&competing),
                &BoundKnowledgeCorrections.identity()
            )
            .is_err(),
        "Competing successor cannot overwrite the winner"
    );
    // A fresh contract cannot target the historical predecessor to form a cycle.
    f.engine.config.acceptance_contracts = Some(vec![f.contract(60, Some(&old))]);
    assert!(f
        .engine
        .run(&f.directory, "Record observation", None)
        .await
        .is_err());
    assert_eq!(
        f.store
            .inspect_knowledge(old.project_id.as_deref(), &scope())
            .unwrap()
            .iter()
            .find(|r| r.entry.id == old.id)
            .unwrap()
            .replaced_by
            .as_deref(),
        Some(new.id.as_str())
    );
}

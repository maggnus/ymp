use super::{confirmation_tests::exact_contract, tests::RunFixture, *};
use crate::*;
use std::collections::BTreeMap;
use ymp_providers::{ExecutionBackendIdentity, ExecutionFuture};

/// Uses the production mock backend for every turn except a later sibling.
struct FailingSibling;
impl ExecutionBackend for FailingSibling {
    fn identity(&self) -> ExecutionBackendIdentity {
        NativeExecutionBackend.identity()
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            if request.purpose == "execute"
                && request
                    .prompt
                    .rsplit("Your current assignment")
                    .next()
                    .unwrap()
                    .contains("Create another greeting")
            {
                bail!("Scripted later sibling failure");
            }
            NativeExecutionBackend.execute(request, events).await
        })
    }
}

fn scoped() -> BTreeMap<String, String> {
    [
        ("dataset", "observations"),
        ("site", "Hill"),
        ("week", "2026-W36"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}

fn source_entry(store: &Store, project: &str) -> MemoryEntry {
    store
        .memory_inventory(Some(project))
        .unwrap()
        .into_iter()
        .find(|m| m.kind == "outcome" && m.status == "active")
        .expect("Intermediate result must have an active evidence-linked knowledge projection")
}

#[tokio::test]
async fn knowledge_intermediate_survives_sibling_failure_and_reaches_later_session() {
    let mut fixture = RunFixture::new("[mock:split-writers]", true);
    fixture.engine = fixture
        .engine
        .with_execution_backend(Arc::new(FailingSibling))
        .unwrap();
    fixture.engine.acceptance_contracts.push(exact_contract());
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "blocked", "{}", outcome.summary);
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert!(outcome.summary.contains("Scripted later sibling failure"));
    assert!(trace
        .invocations
        .iter()
        .any(|i| i.state == InvocationState::Failed));
    assert!(trace.tasks.iter().any(|t| t.state == TaskState::Accepted));
    assert!(trace.tasks.iter().any(|t| t.state != TaskState::Accepted));
    assert!(trace.assignments.iter().all(|a| a.purpose != "learn"));
    let reopened = Store::open(&fixture.store.home).unwrap();
    let memory = source_entry(&reopened, &outcome.session.project_id);
    let provenance = memory.provenance.as_ref().unwrap();
    assert_eq!(provenance.confirmation, ConfirmationStatus::Confirmed);
    assert!(!provenance
        .source
        .as_ref()
        .unwrap()
        .confirmation_ids
        .is_empty());
    assert!(memory.content.contains("Hello from ymp"));
    assert_eq!(reopened.observations().unwrap().len(), 1);
    let locations = reopened.outcomes(&outcome.session.id).unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(
        locations[0].directory,
        fixture.project.canonicalize().unwrap()
    );
    assert_eq!(
        locations[0].artifacts[0].path,
        fixture.project.canonicalize().unwrap().join("greeting.txt")
    );
    assert_eq!(
        locations[0].artifacts[0].sha256,
        Some(bytes_digest(b"Hello from ymp\n"))
    );
    assert!(locations[0].current);
    assert_eq!(
        reopened
            .trace(&outcome.session.id)
            .unwrap()
            .invocations
            .len(),
        trace.invocations.len(),
        "Location inspection starts no inference"
    );

    let mut later = fixture.engine.clone();
    for agent in &mut later.config.agents {
        agent.instructions = "[mock:fail:plan]".into();
    }
    let next = later
        .run(&fixture.project, "Find greeting content", None)
        .await
        .unwrap();
    assert_ne!(next.session.id, outcome.session.id);
    let next_trace = reopened.trace(&next.session.id).unwrap();
    let retrieved = next_trace
        .history
        .iter()
        .filter(|e| e.kind == "memory_retrieval")
        .find_map(|e| {
            e.data["entries"]
                .as_array()?
                .iter()
                .find(|v| v["id"] == memory.id)
                .cloned()
        })
        .expect("Later session must receive intermediate knowledge before provider failure");
    assert_eq!(retrieved["source_session"], outcome.session.id);
    assert_eq!(
        retrieved["version"],
        content_digest(&serde_json::to_string(&memory).unwrap())
    );
    assert_eq!(retrieved["provenance"]["source"]["result_version"], 1);
    assert!(retrieved["included_chars"].as_u64().unwrap() <= 8000);
}

#[tokio::test]
async fn knowledge_scope_source_drift_and_retirement_are_resolved_by_runtime() {
    let mut fixture = RunFixture::new("", true);
    fixture.engine.knowledge_scope = scoped();
    let mut contract = exact_contract();
    std::fs::write(fixture.project.join("observations.csv"), b"Hill,95\n").unwrap();
    contract.inputs.push("observations.csv".into());
    // A declared check with no observed process result is not reusable success.
    contract.checks.push(TrustedCheck {
        id: "unobserved-validator".into(),
        criterion_ids: vec!["greeting-content".into()],
        assertion: CheckAssertion::Command {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "kill -TERM $$".into()],
            verifier_files: vec![],
        },
    });
    fixture.engine.acceptance_contracts.push(contract);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let memory = source_entry(&fixture.store, &outcome.session.project_id);
    let lookup = |scope: &BTreeMap<String, String>, project: Option<&str>| {
        fixture
            .store
            .resolve_memory(
                project,
                &memory.id,
                None,
                scope,
                KnowledgeRetrievalMode::Supported,
            )
            .unwrap()
    };
    assert!(lookup(&scoped(), Some(&outcome.session.project_id)).is_some());
    assert!(lookup(&BTreeMap::new(), Some(&outcome.session.project_id)).is_none());
    let mut wrong = scoped();
    wrong.insert("site".into(), "Harbor".into());
    assert!(lookup(&wrong, Some(&outcome.session.project_id)).is_none());
    assert!(lookup(&scoped(), Some("foreign-project")).is_none());
    assert!(fixture
        .store
        .resolve_memory(
            Some(&outcome.session.project_id),
            &memory.id,
            Some("forged-version"),
            &scoped(),
            KnowledgeRetrievalMode::Supported
        )
        .unwrap()
        .is_none());
    let global = fixture
        .store
        .memory_inventory(None)
        .unwrap()
        .into_iter()
        .find(|m| m.status == "active")
        .unwrap();
    assert!(!global.content.contains("Hill"));
    assert!(!global.content.contains("greeting.txt"));
    assert!(
        !global.content.contains("validator"),
        "Inconclusive checks cannot become shared successful experience"
    );
    assert!(fixture
        .store
        .resolve_memory(
            Some("foreign-project"),
            &global.id,
            None,
            &scoped(),
            KnowledgeRetrievalMode::Supported
        )
        .unwrap()
        .is_some());
    let candidates = fixture
        .store
        .search_memory(
            Some(&outcome.session.project_id),
            "lesson",
            &scoped(),
            KnowledgeRetrievalMode::IncludeUnconfirmed,
        )
        .unwrap();
    assert!(candidates.iter().any(|m| m.status == "proposed"
        && m.provenance.as_ref().unwrap().confirmation == ConfirmationStatus::Unconfirmed));
    assert!(fixture
        .store
        .memory(Some(&outcome.session.project_id), "lesson")
        .unwrap()
        .is_empty());
    let mut forged = memory.clone();
    forged.content = "Hill rate is 1".into();
    assert!(fixture.store.save_memory(&forged).is_err());
    forged.provenance = None;
    assert!(fixture.store.save_memory(&forged).is_err());
    std::fs::write(fixture.project.join("observations.csv"), b"Hill,60\n").unwrap();
    assert!(lookup(&scoped(), Some(&outcome.session.project_id)).is_none());
    assert!(fixture
        .store
        .resolve_memory(
            None,
            &global.id,
            None,
            &scoped(),
            KnowledgeRetrievalMode::Supported
        )
        .unwrap()
        .is_none());
    assert!(!fixture.store.outcomes(&outcome.session.id).unwrap()[0].current);
    std::fs::write(fixture.project.join("observations.csv"), b"Hill,95\n").unwrap();
    assert!(lookup(&scoped(), Some(&outcome.session.project_id)).is_some());
    fixture.store.forget_memory(&memory.id).unwrap();
    assert!(lookup(&scoped(), Some(&outcome.session.project_id)).is_none());
    let source = memory.provenance.unwrap().source.unwrap();
    let replay = fixture
        .store
        .retain_knowledge(
            &source.acceptance_id,
            &KnowledgeProposal::ProjectOutcome,
            &scoped(),
            &EvidenceKnowledgeProposals.identity(),
        )
        .unwrap();
    assert_eq!(
        replay.status, "retired",
        "Replay must not reactivate a retired entry"
    );
}

struct SubstituteRetrieval {
    ids: Vec<MemorySelection>,
}
impl KnowledgeRetrievalPolicy for SubstituteRetrieval {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "test.reverse-inventory".into(),
            version: "7".into(),
        }
    }
    fn candidate_source(&self) -> KnowledgeCandidateSource {
        KnowledgeCandidateSource::Inventory
    }
    fn select(&self, _input: KnowledgeRetrievalInput<'_>) -> Result<Vec<MemorySelection>> {
        Ok(self.ids.clone())
    }
}
struct SubstituteProposals;
impl KnowledgeProposalPolicy for SubstituteProposals {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "test.free-text".into(),
            version: "2".into(),
        }
    }
    fn propose(&self, _input: KnowledgeProposalInput<'_>) -> Result<Vec<KnowledgeProposal>> {
        Ok(vec![KnowledgeProposal::Candidate {
            title: "Invented source claim".into(),
            content: "Every greeting cures bugs".into(),
        }])
    }
}

#[tokio::test]
async fn knowledge_policy_substitution_cannot_forge_sources_activation_or_context_limits() {
    let mut fixture = RunFixture::new("", true);
    fixture.engine.knowledge_proposals = Arc::new(SubstituteProposals);
    let mut contract = exact_contract();
    contract.criteria[0].description = "greeting ".repeat(1600);
    fixture.engine.acceptance_contracts.push(contract);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let entry = source_entry(&fixture.store, &outcome.session.project_id);
    let inventory = fixture
        .store
        .memory_inventory(Some(&outcome.session.project_id))
        .unwrap();
    let candidate = inventory
        .iter()
        .find(|m| m.title == "Invented source claim")
        .unwrap();
    assert_eq!(candidate.status, "proposed");
    assert_eq!(
        candidate.provenance.as_ref().unwrap().policy.id,
        "test.free-text"
    );
    assert_eq!(
        candidate.provenance.as_ref().unwrap().confirmation,
        ConfirmationStatus::Unconfirmed
    );
    let legacy = MemoryEntry {
        provenance: None,
        id: "legacy".into(),
        project_id: None,
        kind: "procedure".into(),
        title: "Greeting legacy".into(),
        content: "Unknown old claim".into(),
        source_session: outcome.session.id.clone(),
        author: "one".into(),
        reviewer: Some("two".into()),
        status: "proposed".into(),
        created_at: now(),
        supersedes: None,
    };
    fixture.store.save_memory(&legacy).unwrap();
    assert!(fixture.store.memory(None, "legacy").unwrap().is_empty());
    assert_eq!(
        fixture
            .store
            .search_memory(
                None,
                "legacy",
                &Default::default(),
                KnowledgeRetrievalMode::IncludeUnconfirmed
            )
            .unwrap()
            .len(),
        1
    );
    fixture.engine.knowledge_retrieval = Arc::new(SubstituteRetrieval {
        ids: vec![
            MemorySelection {
                id: "missing".into(),
                version: None,
            },
            MemorySelection {
                id: candidate.id.clone(),
                version: None,
            },
            MemorySelection {
                id: "legacy".into(),
                version: None,
            },
            MemorySelection {
                id: entry.id.clone(),
                version: Some("wrong-version".into()),
            },
            MemorySelection {
                id: entry.id.clone(),
                version: None,
            },
            MemorySelection {
                id: entry.id.clone(),
                version: None,
            },
        ],
    });
    for agent in &mut fixture.engine.config.agents {
        agent.instructions = "[mock:fail:plan]".into();
    }
    let next = fixture
        .engine
        .run(&fixture.project, "A query without lexical overlap", None)
        .await
        .unwrap();
    let trace = fixture.store.trace(&next.session.id).unwrap();
    let event = trace
        .history
        .iter()
        .find(|e| e.kind == "memory_retrieval")
        .unwrap();
    assert!(
        event.data["query"].is_null(),
        "Inventory policy must not claim an effective FTS query"
    );
    assert_eq!(event.data["implementation"]["id"], "test.reverse-inventory");
    assert_eq!(event.data["entries"].as_array().unwrap().len(), 1);
    assert_eq!(event.data["entries"][0]["id"], entry.id);
    assert_eq!(event.data["included_chars"], 8000);
    let text: String = format!(
        "[confirmation: Confirmed] {}: {}",
        entry.title, entry.content
    )
    .chars()
    .take(8000)
    .collect();
    assert_eq!(
        event.data["entries"][0]["context_digest"],
        content_digest(&text)
    );
    assert_eq!(
        event.data["entries"][0]["version"],
        content_digest(&serde_json::to_string(&entry).unwrap())
    );
}

#[tokio::test]
async fn knowledge_partial_confirmation_preserves_candidates_without_activation() {
    let mut fixture = RunFixture::new("", true);
    let mut contract = exact_contract();
    contract.criteria.push(AcceptanceCriterion {
        id: "quality".into(),
        description: "The greeting is qualitatively appropriate".into(),
    });
    fixture.engine.acceptance_contracts.push(contract);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let inventory = fixture
        .store
        .memory_inventory(Some(&outcome.session.project_id))
        .unwrap();
    assert!(inventory.iter().all(|m| m.status == "proposed"));
    assert!(inventory.iter().any(|m| m.kind == "outcome"
        && !m
            .provenance
            .as_ref()
            .unwrap()
            .source
            .as_ref()
            .unwrap()
            .confirmation_ids
            .is_empty()));
    assert!(fixture
        .store
        .memory(Some(&outcome.session.project_id), "greeting")
        .unwrap()
        .is_empty());
    assert!(fixture.store.memory(None, "file").unwrap().is_empty());
    assert!(fixture.store.observations().unwrap().is_empty());
}

struct FailingProposals;
impl KnowledgeProposalPolicy for FailingProposals {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "test.failed-proposals".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, _input: KnowledgeProposalInput<'_>) -> Result<Vec<KnowledgeProposal>> {
        bail!("Scripted proposal failure")
    }
}

#[tokio::test]
async fn knowledge_policy_failure_preserves_accepted_project_fact() {
    let mut fixture = RunFixture::new("", true);
    fixture.engine.acceptance_contracts.push(exact_contract());
    fixture.engine.knowledge_proposals = Arc::new(FailingProposals);
    let outcome = fixture.run().await;
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    let entry = source_entry(&fixture.store, &outcome.session.project_id);
    assert!(fixture
        .store
        .memory(Some(&outcome.session.project_id), "greeting")
        .unwrap()
        .iter()
        .any(|m| m.id == entry.id));
    let trace = fixture.store.trace(&outcome.session.id).unwrap();
    assert!(trace
        .history
        .iter()
        .any(|e| e.kind == "knowledge_proposal_policy"
            && e.data["implementation"]["id"] == "test.failed-proposals"));
    assert!(fixture
        .store
        .messages(&outcome.session.id, 0, 10000)
        .unwrap()
        .iter()
        .any(|m| m.text.contains("Scripted proposal failure")));
}

#[tokio::test]
async fn knowledge_outcomes_retain_captured_location_after_project_relocation() {
    let mut fixture = RunFixture::new("[mock:split-writers]", true);
    fixture.engine = fixture
        .engine
        .with_execution_backend(Arc::new(FailingSibling))
        .unwrap();
    let mut contract = exact_contract();
    contract.inputs.push("input.csv".into());
    std::fs::write(fixture.project.join("input.csv"), b"Hill,95\n").unwrap();
    fixture.engine.acceptance_contracts.push(contract);
    let source = fixture.run().await;
    assert!(source.summary.contains("Scripted later sibling failure"));
    let before = fixture.store.outcomes(&source.session.id).unwrap();
    assert_eq!(before.len(), 1);
    assert!(before[0].current);
    assert_eq!(before[0].confirmation, ConfirmationStatus::Confirmed);
    assert_eq!(before[0].directory, fixture.project.canonicalize().unwrap());
    assert!(before[0].artifacts[0].path.exists());
    let trace = fixture.store.trace(&source.session.id).unwrap();
    let relocated = fixture.project.parent().unwrap().join("relocated-empty");
    std::fs::create_dir(&relocated).unwrap();
    fixture
        .store
        .relocate_project(&source.session.project_id, &relocated)
        .unwrap();
    let reopened = Store::open(&fixture.store.home).unwrap();
    assert_eq!(
        reopened
            .get_project(&source.session.project_id)
            .unwrap()
            .path,
        relocated.canonicalize().unwrap()
    );
    assert_eq!(
        reopened.outcomes(&source.session.id).unwrap(),
        before,
        "Project registration must not replace captured outcome location or grade"
    );
    assert!(!relocated.join("greeting.txt").exists());
    assert!(!reopened
        .memory(Some(&source.session.project_id), "greeting")
        .unwrap()
        .is_empty());

    // Matching files under the new association cannot conceal changes to the
    // original checked artifacts or supplied inputs.
    std::fs::write(relocated.join("greeting.txt"), b"Hello from ymp\n").unwrap();
    std::fs::write(relocated.join("input.csv"), b"Hill,95\n").unwrap();
    for path in ["greeting.txt", "input.csv"] {
        let original = fixture.project.join(path);
        let bytes = std::fs::read(&original).unwrap();
        std::fs::write(&original, b"changed original source\n").unwrap();
        let changed = reopened.outcomes(&source.session.id).unwrap();
        assert_eq!(changed[0].directory, before[0].directory);
        assert_eq!(changed[0].artifacts, before[0].artifacts);
        assert_eq!(changed[0].result_id, before[0].result_id);
        assert_eq!(changed[0].result_version, before[0].result_version);
        assert!(!changed[0].current);
        assert_eq!(changed[0].confirmation, ConfirmationStatus::Unconfirmed);
        assert!(reopened
            .memory(Some(&source.session.project_id), "greeting")
            .unwrap()
            .is_empty());
        std::fs::write(&original, bytes).unwrap();
        assert_eq!(reopened.outcomes(&source.session.id).unwrap(), before);
    }
    assert_eq!(
        reopened
            .trace(&source.session.id)
            .unwrap()
            .invocations
            .len(),
        trace.invocations.len()
    );
}

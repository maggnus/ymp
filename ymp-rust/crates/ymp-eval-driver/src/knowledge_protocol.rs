use crate::{
    export, protocols,
    script::{NativeJournal, ScriptedBackend},
    workflows, write_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_runtime::{
    BuiltinConfirmationChecker, ConfirmationChecker, Engine, KnowledgeProposalInput,
    KnowledgeProposalPolicy,
};
use ymp_storage::Store;
struct OnlyOutcome;
impl KnowledgeProposalPolicy for OnlyOutcome {
    fn identity(&self) -> KnowledgePolicyIdentity {
        KnowledgePolicyIdentity {
            id: "ymp.evals.outcome-only".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, _: KnowledgeProposalInput<'_>) -> Result<Vec<KnowledgeProposal>> {
        Ok(vec![])
    }
}
fn reference(entry: &MemoryEntry) -> Result<KnowledgeRef> {
    Ok(KnowledgeRef {
        id: entry.id.clone(),
        version: content_digest(&serde_json::to_string(entry)?),
    })
}
fn entry_value(store: &Store, entry: &MemoryEntry) -> Result<Value> {
    let provenance = entry
        .provenance
        .as_ref()
        .context("Missing source")?
        .source
        .as_ref()
        .context("Missing acceptance")?;
    let acceptance = store
        .decisions(&entry.source_session)?
        .into_iter()
        .find(|d| d.id == provenance.acceptance_id)
        .context("Missing accepted source")?;
    let result = acceptance.links.result.unwrap();
    let data: Value = serde_json::from_slice(result.artifacts[0].bytes.as_ref().unwrap())?;
    Ok(data["value"].clone())
}
fn contract(
    root: &Path,
    corrected: bool,
    target: Option<&MemoryEntry>,
    scope: &BTreeMap<String, String>,
) -> Result<AcceptanceContract> {
    let source = if corrected {
        "inputs/observations-corrected.csv"
    } else {
        "inputs/observations.csv"
    };
    let output = if corrected {
        "outputs/observation-corrected.json"
    } else {
        "outputs/observation-original.json"
    };
    let checker = root.join("ymp-evals/driver/protocol_checks.py");
    Ok(AcceptanceContract{knowledge_correction:target.map(|entry|anyhow::Ok(KnowledgeCorrectionBinding{projection:KnowledgeProjection::ProjectOutcome,target:reference(entry)?,applicability:scope.clone(),criterion_ids:vec!["observation-value".into()],source_replacement:Some(KnowledgeSourceReplacement{previous_input:"inputs/observations.csv".into(),replacement_input:source.into()})})).transpose()?,task_title:"Record observation".into(),criteria:vec![AcceptanceCriterion{id:"observation-value".into(),description:"O04 Hill 2026-W36 percentage equals the actual declared source, with an explicit correction relation when supplied".into()}],artifacts:vec![output.into()],inputs:vec![source.into()],checks:vec![TrustedCheck{id:"observation-check".into(),criterion_ids:vec!["observation-value".into()],assertion:CheckAssertion::Command{program:"/usr/bin/env".into(),args:vec!["python3".into(),checker.display().to_string(),"observation".into(),"{workdir}".into(),source.into(),output.into()],verifier_files:vec![checker]}}]})
}
fn engine(
    root: &Path,
    store: &Store,
    scope: &BTreeMap<String, String>,
    corrected: bool,
    journal: Arc<NativeJournal>,
) -> Result<Engine> {
    let mut config = workflows::config();
    config.knowledge_scope = scope.clone();
    if corrected {
        config.team = vec!["b".into(), "a".into()];
    }
    let (events, _) = mpsc::unbounded_channel();
    let mut engine=Engine::new(store.clone(),config,events,CancellationToken::new())?.with_execution_backend(Arc::new(ScriptedBackend{case:if corrected{"observation-corrected"}else{"observation-original"}.into(),task_title:"Record observation".into(),task_prompt:"Compute O04 Hill 2026-W36 percentage from the declared actual input and record its row identity".into(),output:if corrected{"outputs/observation-corrected.json"}else{"outputs/observation-original.json"}.into(),writer:root.join("ymp-evals/driver/artifact_writer.py"),journal}))?;
    engine.knowledge_proposals = Arc::new(OnlyOutcome);
    Ok(engine)
}
pub async fn run(root: &Path, directory: &Path, spec: &Value) -> Result<Value> {
    let started = Instant::now();
    let work = directory.join("work");
    std::fs::create_dir_all(work.join("inputs"))?;
    for input in spec["inputs"].as_array().unwrap() {
        let path = input.as_str().unwrap();
        std::fs::copy(
            root.join("ymp-evals/fixtures/universal").join(path),
            work.join(path),
        )?;
    }
    let store = Store::open(&directory.join("metadata"))?;
    let scope: BTreeMap<String, String> = serde_json::from_value(spec["setup"]["scope"].clone())?;
    let journal = Arc::new(NativeJournal::default());
    let mut first = engine(root, &store, &scope, false, journal.clone())?;
    first.acceptance_contracts = vec![contract(root, false, None, &scope)?];
    let output = first
        .run(
            &work,
            "Record the original observation from its actual source",
            None,
        )
        .await?;
    ensure!(
        output.session.status == "completed",
        "Original source failed: {}",
        output.summary
    );
    let project = store.project(&work)?;
    let old = store
        .memory_inventory(Some(&project.id))?
        .into_iter()
        .find(|entry| entry.source_session == output.session.id && entry.kind == "outcome")
        .context("No promoted original knowledge")?;
    let later_id = new_id();
    let third_id = new_id();
    let mut boundary = Vec::new();
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    let old_source = old.provenance.as_ref().unwrap().source.as_ref().unwrap();
    let old_trace = store.trace(&output.session.id)?;
    let obs1 = old_trace
        .decisions
        .iter()
        .find(|d| d.kind == "reputation_observed")
        .context("Original accepted source has no observation")?;
    rows.push(json!({"type":"knowledge_promoted","session":"s1","knowledge":"k1","status":old.status,"value":entry_value(&store,&old)?,"source_sha256":crate::file_digest(&work.join("inputs/observations.csv"))?,"rows":["O04"],"confirmation":"c1","observation":"obs1","agent":obs1.actor}));
    sources.push(json!({"knowledge_id":old.id,"acceptance_id":old_source.acceptance_id,"observation_id":obs1.links.observation_id}));
    let found = store.resolve_memory(
        Some(&project.id),
        &old.id,
        None,
        &scope,
        KnowledgeRetrievalMode::Supported,
    )?;
    boundary.push(json!({"operation":"resolve_memory","requesting_context":later_id,"scope":scope,"result":found}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    if let Some(found) = found {
        rows.push(json!({"type":"knowledge_retrieved","session":"s2","knowledge":"k1","value":entry_value(&store,&found)?,"source_confirmation":"c1"}));
        sources.push(json!({"boundary_action":boundary.len()-1}));
    }
    let mut harbor = scope.clone();
    harbor.insert("site".into(), "Harbor".into());
    let found = store.resolve_memory(
        Some(&project.id),
        &old.id,
        None,
        &harbor,
        KnowledgeRetrievalMode::Supported,
    )?;
    let inspection = store.inspect_knowledge(Some(&project.id), &harbor)?;
    let availability = inspection
        .iter()
        .find(|row| row.id == old.id)
        .unwrap()
        .availability
        .clone();
    boundary.push(json!({"operation":"resolve_memory","requesting_context":later_id,"scope":harbor,"result":found,"availability":availability}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    rows.push(json!({"type":if found.is_none(){"knowledge_filtered"}else{"unexpected_knowledge_retrieved"},"session":"s2","knowledge":"k1","reason":availability}));
    sources.push(json!({"boundary_action":boundary.len()-1}));
    let candidate = store.retain_knowledge(
        &old_source.acceptance_id,
        &KnowledgeProposal::Candidate {
            title: "Unconfirmed correction hypothesis".into(),
            content: "Agent agreement proposes O04 value60 without confirming its source".into(),
        },
        &scope,
        &OnlyOutcome.identity(),
    )?;
    let before = store.observations()?.len();
    let mut promote = candidate.clone();
    promote.status = "active".into();
    let promoted = store.save_memory(&promote);
    boundary.push(json!({"operation":"save_memory","candidate":candidate,"requested_status":"active","error":promoted.as_ref().err().map(|e|e.to_string())}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    rows.push(json!({"type":if promoted.is_err(){"knowledge_promotion_denied"}else{"unexpected_knowledge_promoted"},"reason":"unconfirmed_agent_agreement"}));
    sources.push(json!({"boundary_action":boundary.len()-1}));
    let agreement_credit = store.observations()?.len().saturating_sub(before);
    let correction_contract = contract(root, true, Some(&old), &scope)?;
    let captured = CapturedAcceptanceContract::capture(
        correction_contract,
        &work,
        BuiltinConfirmationChecker.identity(),
    )?;
    let second = engine(root, &store, &scope, true, journal.clone())?;
    let mut session = output.session.clone();
    session.id = later_id.clone();
    session.title = "Correct O04 against the replacement source".into();
    session.status = "running".into();
    session.turns_used = 0;
    session.created_at = now();
    session.team = second
        .config
        .team
        .iter()
        .map(|id| second.config.agent(id).cloned())
        .collect::<Result<Vec<_>>>()?;
    let mut policy = store.session_policy(&output.session.id)?.unwrap();
    policy.session_id = later_id.clone();
    policy.goal = session.title.clone();
    policy.captured_team = session.team.clone();
    policy.captured_at = now();
    store.create_session_with_contracts(&session, &policy, std::slice::from_ref(&captured))?;
    let capture_record = store
        .decisions(&later_id)?
        .into_iter()
        .find(|d| d.kind == "acceptance_contract_captured")
        .unwrap();
    rows.push(json!({"type":"external_source_captured","source":"inputs/observations-corrected.csv","sha256":captured.inputs[0].sha256,"rows":["O04"]}));
    sources.push(json!({"contract_id":capture_record.id,"input_index":0}));
    let found = store.resolve_memory(
        Some(&project.id),
        &old.id,
        None,
        &scope,
        KnowledgeRetrievalMode::Supported,
    )?;
    let availability = store
        .inspect_knowledge(Some(&project.id), &scope)?
        .into_iter()
        .find(|row| row.id == old.id)
        .unwrap()
        .availability;
    boundary.push(json!({"operation":"resolve_memory","requesting_context":later_id,"scope":scope,"result":found,"availability":availability}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    rows.push(json!({"type":if found.is_none(){"knowledge_filtered"}else{"unexpected_knowledge_retrieved"},"session":"s2","knowledge":"k1","reason":availability}));
    sources.push(json!({"boundary_action":boundary.len()-1}));
    let corrected = second
        .run(
            &work,
            "Correct O04 against the replacement source",
            Some(&later_id),
        )
        .await?;
    ensure!(
        corrected.session.status == "completed",
        "Correction failed: {}",
        corrected.summary
    );
    let new = store
        .memory_inventory(Some(&project.id))?
        .into_iter()
        .find(|entry| entry.source_session == later_id && entry.kind == "outcome")
        .context("Missing corrected projection")?;
    let new_trace = store.trace(&later_id)?;
    let correction = new_trace
        .decisions
        .iter()
        .find(|d| d.kind == "knowledge_superseded")
        .context("No committed correction")?;
    let obs2 = new_trace
        .decisions
        .iter()
        .find(|d| d.kind == "reputation_observed")
        .context("Correction has no supported observation")?;
    rows.push(json!({"type":"knowledge_superseded","old":"k1","new":"k2","value":entry_value(&store,&new)?,"confirmation":"c2","observation":"obs2","agent":obs2.actor}));
    sources.push(json!({"correction_id":correction.id,"observation_id":obs2.links.observation_id}));
    let mut third = session.clone();
    third.id = third_id.clone();
    third.title = "Retrieve current O04 knowledge".into();
    third.turns_used = 0;
    third.created_at = now();
    let mut third_policy = policy;
    third_policy.session_id = third_id.clone();
    third_policy.goal = third.title.clone();
    third_policy.captured_at = now();
    store.create_session(&third, &third_policy)?;
    let found = store.resolve_memory(
        Some(&project.id),
        &new.id,
        None,
        &scope,
        KnowledgeRetrievalMode::Supported,
    )?;
    boundary.push(json!({"operation":"resolve_memory","requesting_context":third_id,"scope":scope,"result":found}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    if let Some(found) = found {
        rows.push(json!({"type":"knowledge_retrieved","session":"s3","knowledge":"k2","value":entry_value(&store,&found)?,"source_confirmation":"c2"}));
        sources.push(json!({"boundary_action":boundary.len()-1}));
    }
    let observation = store
        .observations()?
        .into_iter()
        .find(|o| Some(&o.id) == obs2.links.observation_id.as_ref())
        .unwrap();
    let acceptance = observation.evidence.clone();
    let applied = store.observe_confirmed(&observation, &acceptance)?;
    boundary.push(json!({"operation":"observe_confirmed","observation_id":observation.id,"acceptance_id":acceptance,"applied":applied}));
    store.event(
        &output.session.id,
        "eval_knowledge_boundary",
        &json!({"boundary_action":boundary.len()-1,"actual":boundary.last()}),
    )?;
    rows.push(json!({"type":if applied{"observation_applied"}else{"observation_ignored"},"observation":"obs2","reason":"already_applied"}));
    sources.push(json!({"boundary_action":boundary.len()-1}));
    let history = store.inspect_knowledge(Some(&project.id), &scope)?;
    let old_state = history.iter().find(|r| r.id == old.id).unwrap();
    let new_state = history.iter().find(|r| r.id == new.id).unwrap();
    let mut positive: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (trace, label) in [(&old_trace, "obs1"), (&new_trace, "obs2")] {
        for decision in trace
            .decisions
            .iter()
            .filter(|d| d.kind == "reputation_observed")
        {
            positive
                .entry(decision.actor.clone().unwrap_or_default())
                .or_default()
                .push(label.into());
        }
    }
    let traces = store
        .sessions(None)?
        .iter()
        .map(|session| Ok((session.id.clone(), store.trace(&session.id)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let projection = self::project(&traces, &rows, &sources, &boundary)?;
    let observed = json!({"schema_version":1,"case_id":"knowledge-correction","events":projection.events,"final_state":{"knowledge":{"k1":old_state.entry.status,"k2":new_state.entry.status},"current_value":entry_value(&store,&new)?,"positive_observations_by_agent":positive,"agreement_credit":agreement_credit}});
    write_json(
        &directory.join("projection-bindings.json"),
        &json!({"observed_values":rows,"source_bindings":sources}),
    )?;
    write_json(&directory.join("runtime.json"), &traces)?;
    journal.save(directory)?;
    write_json(&directory.join("knowledge.json"), &history)?;
    write_json(&directory.join("boundary-actions.json"), &boundary)?;
    write_json(&directory.join("observed.json"), &observed)?;
    write_json(
        &directory.join("alias-map.json"),
        &json!({"s1":output.session.id,"s2":later_id,"s3":third_id,"k1":old.id,"k2":new.id,"c1":old_source.confirmation_ids,"c2":new.provenance.as_ref().unwrap().source.as_ref().unwrap().confirmation_ids,"obs1":obs1.links.observation_id,"obs2":obs2.links.observation_id,"projection_sources":projection.sources,"context_note":"The later query-only client context preallocates its eventual session ID; the session is atomically captured when correction admission begins."}),
    )?;
    let metrics = traces
        .iter()
        .map(|(id, trace)| {
            Ok((
                id.clone(),
                export::metrics(trace, started.elapsed().as_secs_f64(), 1)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    write_json(&directory.join("metrics.json"), &metrics)?;
    let validation = protocols::validate(root, directory, "knowledge-correction").await?;
    Ok(
        json!({"case_id":"knowledge-correction","complete":validation["validator_passed"]==true,"validator":validation,"metrics":metrics}),
    )
}

/// Values above are captured from returned records. This walk determines their
/// occurrence and order from the complete durable journal, never the call script.
pub(crate) fn project(
    traces: &BTreeMap<String, SessionTrace>,
    values: &[Value],
    bindings: &[Value],
    boundary: &[Value],
) -> Result<export::Projection> {
    ensure!(
        values.len() == bindings.len(),
        "Every knowledge value needs source binding"
    );
    let mut projection = export::Projection::default();
    let mut used = vec![false; bindings.len()];
    let mut observations = BTreeMap::<String, usize>::new();
    for event in export::ordered_history(traces.values())? {
        let decision = if event.kind == "provenance" {
            match serde_json::from_value::<ProvenanceEvent>(event.data.clone())? {
                ProvenanceEvent::DecisionRecorded { decision } => Some(decision),
                _ => None,
            }
        } else {
            None
        };
        let relevant = event.kind == "eval_knowledge_boundary"
            || (event.kind == "knowledge_retained" && event.data["status"] == "active")
            || decision.as_ref().is_some_and(|d| {
                d.kind == "knowledge_superseded"
                    || (d.kind == "acceptance_contract_captured"
                        && d.links
                            .acceptance_contract
                            .as_ref()
                            .is_some_and(|c| c.contract.knowledge_correction.is_some()))
            });
        if relevant {
            let found = bindings.iter().position(|binding| {
                if event.kind == "eval_knowledge_boundary" {
                    return binding
                        .get("boundary_action")
                        .is_some_and(|index| *index == event.data["boundary_action"]);
                }
                if event.kind == "knowledge_retained" {
                    return binding
                        .get("knowledge_id")
                        .is_some_and(|id| *id == event.data["id"]);
                }
                decision.as_ref().is_some_and(|d| {
                    binding["correction_id"] == d.id || binding["contract_id"] == d.id
                })
            });
            let source = export::event_source(event);
            if let Some(index) = found {
                if event.kind == "eval_knowledge_boundary" {
                    let ordinal = event.data["boundary_action"]
                        .as_u64()
                        .context("Invalid actual boundary index")?
                        as usize;
                    ensure!(
                        boundary.get(ordinal) == Some(&event.data["actual"]),
                        "Durable boundary differs from retained actual response"
                    );
                }
                used[index] = true;
                projection.push(values[index].clone(), source);
            } else {
                projection.push(json!({"type":"unexpected_knowledge_transition","kind":event.kind,"data":event.data}),source);
            }
        }
        if let Some(decision) = decision.filter(|d| d.kind == "reputation_observed") {
            let id = decision
                .links
                .observation_id
                .as_deref()
                .context("Unbound observation")?;
            let count = observations.entry(id.into()).or_default();
            *count += 1;
            if *count > 1 || !bindings.iter().any(|b| b["observation_id"] == id) {
                projection.push(json!({"type":"observation_applied","observation_id":id,"agent":decision.actor}),export::event_source(event));
            }
        }
    }
    ensure!(
        used.iter().all(|used| *used),
        "Captured knowledge response has no durable source"
    );
    Ok(projection)
}
